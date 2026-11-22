use std::{collections::BTreeMap, path::PathBuf, sync::OnceLock, time::Duration};

use crate::game_image_cache::{CacheState, ImageCache};
use futures_util::{stream, StreamExt};
use serde::Serialize;
use tauri::{
    async_runtime::Mutex,
    http::{Method, Request, Response},
    ipc::Channel,
    State,
};

const SOURCE: &str = "https://raw.githubusercontent.com/Mar-7th/StarRailRes/03907e798b46f9eb109a1f882055cdf49ce3d374";
const PARALLEL_IMAGES: usize = 4;

pub type SyncLock = Mutex<()>;

#[derive(Clone, Copy)]
struct Image {
    remote: &'static str,
    bytes: usize,
    blob: &'static str,
}

fn images() -> &'static BTreeMap<&'static str, Image> {
    static IMAGES: OnceLock<BTreeMap<&str, Image>> = OnceLock::new();
    IMAGES.get_or_init(|| {
        include_str!("../star-rail-images.tsv")
            .lines()
            .map(|line| {
                let mut fields = line.split('\t');
                let local = fields.next().expect("local image path");
                let remote = fields.next().expect("upstream image path");
                let bytes = fields
                    .next()
                    .expect("image size")
                    .parse()
                    .expect("valid size");
                let blob = fields.next().expect("Git blob hash");
                (
                    local,
                    Image {
                        remote,
                        bytes,
                        blob,
                    },
                )
            })
            .collect()
    })
}

fn cache_directory() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|error| format!("无法定位程序文件：{error}"))?;
    let directory = exe.parent().ok_or("无法定位程序目录")?;
    Ok(directory.join("images").join("StarRailRes"))
}

impl Image {
    fn cache(&self, path: PathBuf) -> ImageCache<'static> {
        ImageCache::new(path, self.bytes, self.blob)
    }

    async fn fetch(&self, client: &reqwest::Client) -> Result<Vec<u8>, String> {
        let mut response = client
            .get(format!("{SOURCE}/{}", self.remote))
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map_err(|error| error.to_string())?;
        let mut bytes = Vec::with_capacity(self.bytes);
        while let Some(chunk) = response.chunk().await.map_err(|error| error.to_string())? {
            if chunk.len() > self.bytes - bytes.len() {
                return Err(format!("{} 长度超出清单", self.remote));
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok(bytes)
    }

    async fn prepare(
        self,
        client: reqwest::Client,
        path: PathBuf,
        state: CacheState,
    ) -> Result<(), String> {
        let cache = self.cache(path);
        let downloaded = if state == CacheState::LegacyPng {
            None
        } else {
            Some(self.fetch(&client).await?)
        };
        tauri::async_runtime::spawn_blocking(move || {
            let (png, already_saved) = match downloaded {
                Some(bytes) if cache.valid_png(&bytes) => (bytes, false),
                Some(_) => return Err(format!("{} 校验失败", self.remote)),
                None => (
                    cache
                        .read_png()
                        .ok_or_else(|| format!("{} 的本地 PNG 已变更", self.remote))?,
                    true,
                ),
            };
            cache
                .prepare(&png, already_saved)
                .map_err(|error| format!("保存 {} 失败：{error}", self.remote))
        })
        .await
        .map_err(|error| error.to_string())?
    }
}

#[derive(Clone, Serialize)]
pub struct SyncReport {
    available: Vec<String>,
    missing: usize,
    error: Option<String>,
}

#[tauri::command]
pub async fn player_sync_game_images(
    lock: State<'_, SyncLock>,
    on_cached: Channel<SyncReport>,
) -> Result<SyncReport, String> {
    let _guard = lock.try_lock().map_err(|_| "正在校验游戏图像")?;
    let directory = cache_directory()?;
    let mut available = Vec::new();
    let mut pending = Vec::new();
    for (&local, image) in images() {
        let state = image.cache(directory.join(local)).state();
        if state != CacheState::Missing {
            available.push(local.to_owned());
        }
        if state != CacheState::Prepared {
            pending.push((local.to_owned(), *image, state));
        }
    }
    pending.sort_by_key(|(_, _, state)| *state == CacheState::Missing);
    let mut report = SyncReport {
        available,
        missing: pending.len(),
        error: None,
    };
    on_cached
        .send(report.clone())
        .map_err(|error| error.to_string())?;
    if pending.is_empty() {
        return Ok(report);
    }

    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(5))
        .read_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|error| format!("无法连接 StarRailRes：{error}"))?;
    let mut updates = stream::iter(pending)
        .map(|(local, image, state): (String, Image, CacheState)| {
            let path = directory.join(&local);
            let client = client.clone();
            async move { (local, state, image.prepare(client, path, state).await) }
        })
        .buffer_unordered(PARALLEL_IMAGES);
    let mut consecutive_failures = 0;
    let mut processed = 0;
    while let Some((local, state, result)) = updates.next().await {
        match result {
            Ok(()) => {
                if state == CacheState::Missing {
                    report.available.push(local);
                }
                report.missing -= 1;
                processed += 1;
                consecutive_failures = 0;
                if processed % 24 == 0 && report.missing > 0 {
                    on_cached
                        .send(report.clone())
                        .map_err(|error| error.to_string())?;
                }
            }
            Err(error) => {
                report.error = Some(error);
                consecutive_failures += 1;
                if state == CacheState::Missing && consecutive_failures >= PARALLEL_IMAGES {
                    break;
                }
            }
        }
    }
    Ok(report)
}

pub fn serve(request: Request<Vec<u8>>) -> Response<Vec<u8>> {
    let bytes = (request.method() == Method::GET)
        .then(|| request.uri().path().strip_prefix('/'))
        .flatten()
        .and_then(|local| {
            let image = images().get(local)?;
            image.cache(cache_directory().ok()?.join(local)).read()
        });
    match bytes {
        Some(image) => Response::builder()
            .header("Content-Type", image.mime)
            .header("Cache-Control", "no-store")
            .header("X-Content-Type-Options", "nosniff")
            .body(image.bytes),
        None => Response::builder()
            .status(404)
            .header("Cache-Control", "no-store")
            .body(Vec::new()),
    }
    .expect("valid image response")
}
