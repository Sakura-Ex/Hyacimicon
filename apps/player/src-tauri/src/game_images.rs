use std::{collections::BTreeMap, path::PathBuf, sync::OnceLock, time::Duration};

use crate::game_image_cache::{CachedImage, ImageArchive, ImageCache};
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

fn archive(directory: &std::path::Path) -> &'static ImageArchive {
    static ARCHIVE: OnceLock<ImageArchive> = OnceLock::new();
    ARCHIVE.get_or_init(|| ImageArchive::new(directory.with_extension("pack")))
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
        archive: &'static ImageArchive,
        local: String,
        path: PathBuf,
        legacy: bool,
    ) -> Result<(), String> {
        let cache = self.cache(path);
        let downloaded = if legacy {
            None
        } else {
            Some(self.fetch(&client).await?)
        };
        tauri::async_runtime::spawn_blocking(move || {
            let image = match downloaded {
                Some(bytes) if cache.valid_png(&bytes) => CachedImage::from_png(bytes),
                Some(_) => return Err(format!("{} 校验失败", self.remote)),
                None => {
                    let image = cache
                        .read_legacy()
                        .ok_or_else(|| format!("{} 的旧缓存已变更", self.remote))?;
                    if image.mime == "image/png" {
                        CachedImage::from_png(image.bytes)
                    } else {
                        image
                    }
                }
            };
            archive
                .store(&local, &cache, &image)
                .map_err(|error| format!("归档 {} 失败：{error}", self.remote))?;
            cache
                .clear_legacy()
                .map_err(|error| format!("清理 {} 的旧缓存失败：{error}", self.remote))
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
    let archive = archive(&directory);
    let mut available = Vec::new();
    let mut pending = Vec::new();
    for (&local, image) in images() {
        let cache = image.cache(directory.join(local));
        if archive
            .read(local, &cache)
            .map_err(|error| format!("校验图像容器失败：{error}"))?
            .is_some()
        {
            cache
                .clear_legacy()
                .map_err(|error| format!("清理 {local} 的旧缓存失败：{error}"))?;
            available.push(local.to_owned());
        } else {
            let legacy = cache.read_legacy().is_some();
            if legacy {
                available.push(local.to_owned());
            }
            pending.push((local.to_owned(), *image, legacy));
        }
    }
    pending.sort_by_key(|(_, _, legacy)| !legacy);
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
        .map(|(local, image, legacy): (String, Image, bool)| {
            let path = directory.join(&local);
            let client = client.clone();
            async move {
                let result = image
                    .prepare(client, archive, local.clone(), path, legacy)
                    .await;
                (local, legacy, result)
            }
        })
        .buffer_unordered(PARALLEL_IMAGES);
    let mut consecutive_failures = 0;
    let mut processed = 0;
    while let Some((local, legacy, result)) = updates.next().await {
        match result {
            Ok(()) => {
                if !legacy {
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
                if !legacy && consecutive_failures >= PARALLEL_IMAGES {
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
            let directory = cache_directory().ok()?;
            let archive = archive(&directory);
            let cache = image.cache(directory.join(local));
            let packed = || archive.read(local, &cache).ok().flatten();
            packed().or_else(|| cache.read_legacy()).or_else(packed)
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
