use std::{
    fs::{self, File, OpenOptions},
    io::{self, Cursor, Read, Write},
    path::{Path, PathBuf},
};

use image_webp::{ColorType, WebPEncoder};
use sha1::{Digest, Sha1};
use uuid::Uuid;

pub struct ImageCache<'a> {
    png: PathBuf,
    source_bytes: usize,
    source_blob: &'a str,
}

pub struct CachedImage {
    pub bytes: Vec<u8>,
    pub mime: &'static str,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CacheState {
    Prepared,
    LegacyPng,
    Missing,
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

    pub fn read_png(&self) -> Option<Vec<u8>> {
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
        (bytes.starts_with(b"RIFF")
            && bytes.get(8..12) == Some(b"WEBP")
            && format!("{:x}", Sha1::digest(&bytes)) == encoded)
            .then_some(CachedImage {
                bytes,
                mime: "image/webp",
            })
    }

    pub fn state(&self) -> CacheState {
        if let Some(image) = self.read_prepared() {
            if image.mime == "image/webp" && self.png.is_file() {
                // A previous run may have been interrupted after committing the WebP.
                let _ = fs::remove_file(&self.png);
            }
            CacheState::Prepared
        } else if self.read_png().is_some() {
            CacheState::LegacyPng
        } else {
            CacheState::Missing
        }
    }

    pub fn read(&self) -> Option<CachedImage> {
        self.read_prepared()
            .or_else(|| {
                Some(CachedImage {
                    bytes: self.read_png()?,
                    mime: "image/png",
                })
            })
            .or_else(|| self.read_prepared())
    }

    // The caller supplies the verified upstream PNG. Keep it available until the compact file
    // and its proof have both been committed.
    pub fn prepare(&self, png: &[u8], already_saved: bool) -> io::Result<()> {
        if !already_saved {
            write_atomic(&self.png, png)?;
        }
        let compact = lossless_webp(png).filter(|bytes| bytes.len() + 81 < png.len());
        let proof = if let Some(bytes) = &compact {
            write_atomic(&self.png.with_extension("webp"), bytes)?;
            format!("{} {:x}", self.source_blob, Sha1::digest(bytes))
        } else {
            format!("{} png", self.source_blob)
        };
        write_atomic(&self.proof_path(), proof.as_bytes())?;
        if compact.is_some() {
            let _ = fs::remove_file(&self.png);
        } else {
            let _ = fs::remove_file(self.png.with_extension("webp"));
        }
        Ok(())
    }
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

fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use image_webp::WebPDecoder;

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
        assert!(cache.read().is_none());
        write_atomic(&path, &source).unwrap();
        assert!(cache.state() == CacheState::LegacyPng);
        cache.prepare(&source, true).unwrap();
        assert!(!path.exists());
        assert!(cache.state() == CacheState::Prepared);
        let image = cache.read().unwrap();
        assert_eq!(image.mime, "image/webp");
        let mut decoder = WebPDecoder::new(Cursor::new(&image.bytes)).unwrap();
        let mut decoded = vec![0; decoder.output_buffer_size().unwrap()];
        decoder.read_image(&mut decoded).unwrap();
        assert_eq!(decoded, pixels);
        let mut corrupted = image.bytes;
        *corrupted.last_mut().unwrap() ^= 1;
        write_atomic(&path.with_extension("webp"), &corrupted).unwrap();
        assert!(cache.read().is_none());
        cache.prepare(&source, false).unwrap();
        assert!(cache.state() == CacheState::Prepared);
        fs::remove_dir_all(root).unwrap();
    }
}
