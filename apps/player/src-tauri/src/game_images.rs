use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::OnceLock,
    time::Duration,
};

use futures_util::{stream, StreamExt};
use serde::Serialize;
use sha1::{Digest, Sha1};
use tauri::{
    async_runtime::Mutex,
    http::{Method, Request, Response},
    ipc::Channel,
    State,
};
use uuid::Uuid;

const SOURCE: &str = "https://raw.githubusercontent.com/Mar-7th/StarRailRes/03907e798b46f9eb109a1f882055cdf49ce3d374";
const PARALLEL_DOWNLOADS: usize = 8;
const PNG_HEADER: &[u8] = b"\x89PNG\r\n\x1a\n";

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
    fn valid(&self, bytes: &[u8]) -> bool {
        if bytes.len() != self.bytes || !bytes.starts_with(PNG_HEADER) {
            return false;
        }
        let mut hash = Sha1::new();
        hash.update(format!("blob {}\0", self.bytes));
        hash.update(bytes);
        format!("{:x}", hash.finalize()) == self.blob
    }

    fn read(&self, path: &Path) -> Option<Vec<u8>> {
        let file = File::open(path).ok()?;
        if file.metadata().ok()?.len() != self.bytes as u64 {
            return None;
        }
        let mut bytes = Vec::with_capacity(self.bytes);
        file.take(self.bytes as u64 + 1)
            .read_to_end(&mut bytes)
            .ok()?;
        self.valid(&bytes).then_some(bytes)
    }

    async fn fetch(&self, client: &reqwest::Client, path: &Path) -> Result<(), String> {
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
        if !self.valid(&bytes) {
            return Err(format!("{} 校验失败", self.remote));
        }
        write_image(path, &bytes).map_err(|error| format!("无法写入 {}：{error}", path.display()))
    }
}

fn write_image(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let directory = path.parent().expect("cache subdirectory");
    fs::create_dir_all(directory)?;
    let temporary = directory.join(format!(".{}.tmp", Uuid::new_v4()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    let result = (|| {
        file.write_all(bytes)?;
        drop(file);
        fs::rename(&temporary, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
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
    let (available, missing): (Vec<_>, Vec<_>) = images()
        .iter()
        .map(|(&local, image)| (local.to_owned(), *image))
        .partition(|(local, image)| image.read(&directory.join(local)).is_some());
    let mut report = SyncReport {
        available: available.into_iter().map(|(path, _)| path).collect(),
        missing: missing.len(),
        error: None,
    };
    on_cached
        .send(report.clone())
        .map_err(|error| error.to_string())?;
    if missing.is_empty() {
        return Ok(report);
    }

    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|error| format!("无法连接 StarRailRes：{error}"))?;
    let mut downloads = stream::iter(missing)
        .map(|(local, image): (String, Image)| {
            let path = directory.join(&local);
            let client = client.clone();
            async move { (local, image.fetch(&client, &path).await) }
        })
        .buffer_unordered(PARALLEL_DOWNLOADS);
    let mut consecutive_failures = 0;
    while let Some((local, result)) = downloads.next().await {
        match result {
            Ok(()) => {
                report.available.push(local);
                report.missing -= 1;
                consecutive_failures = 0;
                if report.available.len() % 24 == 0 && report.missing > 0 {
                    on_cached
                        .send(report.clone())
                        .map_err(|error| error.to_string())?;
                }
            }
            Err(error) => {
                report.error = Some(error);
                consecutive_failures += 1;
                if consecutive_failures == PARALLEL_DOWNLOADS {
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
            image.read(&cache_directory().ok()?.join(local))
        });
    match bytes {
        Some(bytes) => Response::builder()
            .header("Content-Type", "image/png")
            .header("Cache-Control", "no-store")
            .header("X-Content-Type-Options", "nosniff")
            .body(bytes),
        None => Response::builder()
            .status(404)
            .header("Cache-Control", "no-store")
            .body(Vec::new()),
    }
    .expect("valid image response")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_cached_file_and_replaces_same_size_corruption() {
        let root = std::env::temp_dir().join(format!("hyacimicon-images-{}", Uuid::new_v4()));
        let path = root.join("avatars/head-1.png");
        let bytes = b"\x89PNG\r\n\x1a\nfixture";
        let image = Image {
            remote: "icon/character/1.png",
            bytes: bytes.len(),
            blob: "aa62e8baaeee255d1bbcd31e48ddb34fd7042e93",
        };
        assert!(image.read(&path).is_none());
        write_image(&path, b"\x89PNG\r\n\x1a\ncorrupt").unwrap();
        assert!(image.read(&path).is_none());
        write_image(&path, bytes).unwrap();
        assert_eq!(image.read(&path).as_deref(), Some(bytes.as_slice()));
        fs::remove_dir_all(root).unwrap();
    }
}
