use std::{
    fs::{self, File},
    io::{self, Cursor, Read},
    path::{Path, PathBuf},
    sync::Mutex,
};

use image_webp::{ColorType, WebPEncoder};
use rusqlite::{params, Connection, OptionalExtension};
use sha1::{Digest, Sha1};

pub struct ImageCache<'a> {
    png: PathBuf,
    source_bytes: usize,
    source_blob: &'a str,
}

pub struct CachedImage {
    pub bytes: Vec<u8>,
    pub mime: &'static str,
}

pub struct ImageArchive {
    path: PathBuf,
    connection: Mutex<Option<Connection>>,
}

impl ImageArchive {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            connection: Mutex::new(None),
        }
    }

    fn query<T>(
        &self,
        operation: impl FnOnce(&Connection) -> rusqlite::Result<T>,
    ) -> Result<T, String> {
        let mut connection = self.connection.lock().map_err(|error| error.to_string())?;
        if connection.is_none() {
            let folder = self.path.parent().expect("image archive directory");
            fs::create_dir_all(folder).map_err(|error| error.to_string())?;
            let opened = Connection::open(&self.path).map_err(|error| error.to_string())?;
            opened
                .execute_batch(
                    "CREATE TABLE IF NOT EXISTS images (
                    name TEXT PRIMARY KEY,
                    source_blob TEXT NOT NULL,
                    mime TEXT NOT NULL,
                    wrapped INTEGER NOT NULL,
                    digest TEXT NOT NULL,
                    payload BLOB NOT NULL
                );",
                )
                .map_err(|error| error.to_string())?;
            *connection = Some(opened);
        }
        operation(connection.as_ref().expect("opened image archive"))
            .map_err(|error| error.to_string())
    }

    pub fn read(&self, name: &str, source: &ImageCache<'_>) -> Result<Option<CachedImage>, String> {
        let row = self.query(|db| {
            db.query_row(
                "SELECT mime, wrapped, digest, payload FROM images
             WHERE name = ?1 AND source_blob = ?2 AND length(payload) <= ?3",
                params![name, source.source_blob, source.source_bytes as i64],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, bool>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Vec<u8>>(3)?,
                    ))
                },
            )
            .optional()
        })?;
        Ok(row.and_then(|(mime, wrapped, digest, payload)| {
            let bytes = if wrapped {
                zstd::bulk::decompress(&payload, source.source_bytes).ok()?
            } else {
                payload
            };
            if format!("{:x}", Sha1::digest(&bytes)) != digest {
                return None;
            }
            let mime = match mime.as_str() {
                "image/png" if source.valid_png(&bytes) => "image/png",
                "image/webp" if is_webp(&bytes) => "image/webp",
                _ => return None,
            };
            Some(CachedImage { bytes, mime })
        }))
    }

    pub fn store(
        &self,
        name: &str,
        source: &ImageCache<'_>,
        image: &CachedImage,
    ) -> Result<(), String> {
        let compressed =
            zstd::bulk::compress(&image.bytes, 5).map_err(|error| error.to_string())?;
        let wrapped = compressed.len() < image.bytes.len();
        let payload = if wrapped { &compressed } else { &image.bytes };
        let digest = format!("{:x}", Sha1::digest(&image.bytes));
        self.query(|db| {
            db.execute(
                "INSERT OR REPLACE INTO images (name, source_blob, mime, wrapped, digest, payload)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    name,
                    source.source_blob,
                    image.mime,
                    wrapped,
                    digest,
                    payload
                ],
            )
        })
        .map(|_| ())
    }
}

impl<'a> ImageCache<'a> {
    pub fn new(png: PathBuf, source_bytes: usize, source_blob: &'a str) -> Self {
        Self {
            png,
            source_bytes,
            source_blob,
        }
    }

    pub fn valid_png(&self, bytes: &[u8]) -> bool {
        if bytes.len() != self.source_bytes || !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            return false;
        }
        let mut hash = Sha1::new();
        hash.update(format!("blob {}\0", self.source_bytes));
        hash.update(bytes);
        format!("{:x}", hash.finalize()) == self.source_blob
    }

    fn read_png(&self) -> Option<Vec<u8>> {
        let bytes = read_at_most(&self.png, self.source_bytes)?;
        self.valid_png(&bytes).then_some(bytes)
    }

    fn proof_path(&self) -> PathBuf {
        self.png.with_extension("cache")
    }

    fn read_prepared(&self) -> Option<CachedImage> {
        let proof = read_at_most(&self.proof_path(), 96)?;
        let (source_blob, encoded) = std::str::from_utf8(&proof).ok()?.split_once(' ')?;
        if source_blob != self.source_blob {
            return None;
        }
        if encoded == "png" {
            return Some(CachedImage {
                bytes: self.read_png()?,
                mime: "image/png",
            });
        }
        let bytes = read_at_most(&self.png.with_extension("webp"), self.source_bytes)?;
        (is_webp(&bytes) && format!("{:x}", Sha1::digest(&bytes)) == encoded).then_some(
            CachedImage {
                bytes,
                mime: "image/webp",
            },
        )
    }

    pub fn read_legacy(&self) -> Option<CachedImage> {
        self.read_prepared().or_else(|| {
            Some(CachedImage {
                bytes: self.read_png()?,
                mime: "image/png",
            })
        })
    }

    pub fn clear_legacy(&self) -> io::Result<()> {
        for path in [
            &self.png,
            &self.png.with_extension("webp"),
            &self.proof_path(),
        ] {
            match fs::remove_file(path) {
                Ok(()) => (),
                Err(error) if error.kind() == io::ErrorKind::NotFound => (),
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }
}

impl CachedImage {
    pub fn from_png(png: Vec<u8>) -> Self {
        if let Some(bytes) = lossless_webp(&png).filter(|bytes| bytes.len() < png.len()) {
            Self {
                bytes,
                mime: "image/webp",
            }
        } else {
            Self {
                bytes: png,
                mime: "image/png",
            }
        }
    }
}

fn is_webp(bytes: &[u8]) -> bool {
    bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP")
}

fn read_at_most(path: &Path, limit: usize) -> Option<Vec<u8>> {
    let file = File::open(path).ok()?;
    let size = file.metadata().ok()?.len();
    if size > limit as u64 {
        return None;
    }
    let mut bytes = Vec::with_capacity(size as usize);
    file.take(limit as u64 + 1).read_to_end(&mut bytes).ok()?;
    (bytes.len() <= limit).then_some(bytes)
}

fn lossless_webp(input: &[u8]) -> Option<Vec<u8>> {
    let mut decoder = png::Decoder::new(Cursor::new(input));
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder.read_info().ok()?;
    let info = reader.info();
    // Retain source PNGs when WebP cannot preserve the animation, precision or color space.
    if info.animation_control.is_some()
        || info.coding_independent_code_points.is_some()
        || (info.srgb.is_none()
            && info.icc_profile.is_none()
            && (info.gama_chunk.is_some() || info.chrm_chunk.is_some()))
    {
        return None;
    }
    let profile = info.icc_profile.as_ref().map(|data| data.to_vec());
    let mut pixels = vec![0; reader.output_buffer_size()?];
    let frame = reader.next_frame(&mut pixels).ok()?;
    let color = match (frame.bit_depth, frame.color_type) {
        (png::BitDepth::Eight, png::ColorType::Grayscale) => ColorType::L8,
        (png::BitDepth::Eight, png::ColorType::GrayscaleAlpha) => ColorType::La8,
        (png::BitDepth::Eight, png::ColorType::Rgb) => ColorType::Rgb8,
        (png::BitDepth::Eight, png::ColorType::Rgba) => ColorType::Rgba8,
        _ => return None,
    };
    let mut output = Vec::new();
    let mut encoder = WebPEncoder::new(&mut output);
    if let Some(profile) = profile {
        encoder.set_icc_profile(profile);
    }
    encoder
        .encode(
            &pixels[..frame.buffer_size()],
            frame.width,
            frame.height,
            color,
        )
        .ok()?;
    Some(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image_webp::WebPDecoder;
    use uuid::Uuid;

    #[test]
    fn migrates_verified_png_without_changing_pixels_and_detects_corruption() {
        let root = std::env::temp_dir().join(format!("hyacimicon-images-{}", Uuid::new_v4()));
        let path = root.join("avatars/head-1.png");
        let pixels = [13, 37, 53, 0, 12, 34, 56, 127, 77, 89, 91, 255].repeat(128 * 128 / 4);
        let mut source = Vec::new();
        let mut encoder = png::Encoder::new(&mut source, 96, 128);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_compression(png::Compression::NoCompression);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&pixels)
            .unwrap();
        let mut hash = Sha1::new();
        hash.update(format!("blob {}\0", source.len()));
        hash.update(&source);
        let blob = format!("{:x}", hash.finalize());
        let cache = ImageCache::new(path.clone(), source.len(), &blob);
        let archive = ImageArchive::new(root.join("StarRailRes.pack"));
        let name = "avatars/head-1.png";
        assert!(archive.read(name, &cache).unwrap().is_none());
        archive
            .store(
                "png-backup",
                &cache,
                &CachedImage {
                    bytes: source.clone(),
                    mime: "image/png",
                },
            )
            .unwrap();
        let wrapped: bool = archive
            .query(|db| db.query_row("SELECT wrapped FROM images", [], |row| row.get(0)))
            .unwrap();
        assert!(wrapped);
        assert_eq!(
            archive.read("png-backup", &cache).unwrap().unwrap().bytes,
            source
        );
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, &source).unwrap();
        let legacy = cache.read_legacy().unwrap();
        archive
            .store(name, &cache, &CachedImage::from_png(legacy.bytes))
            .unwrap();
        cache.clear_legacy().unwrap();
        assert!(!path.exists());
        let image = archive.read(name, &cache).unwrap().unwrap();
        assert_eq!(image.mime, "image/webp");
        let mut decoder = WebPDecoder::new(Cursor::new(&image.bytes)).unwrap();
        let mut decoded = vec![0; decoder.output_buffer_size().unwrap()];
        decoder.read_image(&mut decoded).unwrap();
        assert_eq!(decoded, pixels);
        archive
            .query(|db| {
                db.execute(
                    "UPDATE images SET digest = 'corrupted' WHERE name = ?1",
                    [name],
                )
            })
            .unwrap();
        assert!(archive.read(name, &cache).unwrap().is_none());
        archive
            .store(name, &cache, &CachedImage::from_png(source))
            .unwrap();
        drop(archive);
        let reopened = ImageArchive::new(root.join("StarRailRes.pack"));
        assert!(reopened.read(name, &cache).unwrap().is_some());
        drop(reopened);
        fs::remove_dir_all(root).unwrap();
    }
}
