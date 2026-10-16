//! Images for the SE5 UI: instance thumbnails, account avatars, screenshots, icons from the web. Decoding runs on a
//! worker with the engine's own decoder (`sturdy::images`), the pixels are uploaded as engine textures on the UI
//! thread, and the least recently used textures are unloaded once the cache is full.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, mpsc};

use vertex_shell::sturdy::Engine;
use vertex_shell::sturdy::assets::{
    Asset, RendererTextureHandle, TextureColorSpace, TextureKind, TexturePixelFormat,
};
use vertex_shell::sturdy::images::{DecodeOptions, DecodedImage, PixelFormat, decode};

/// Textures kept alive at once; beyond this the least recently drawn ones are unloaded.
const MAX_TEXTURES: usize = 192;

/// Longest edge of a texture made from a picture; bigger ones are shrunk on the worker first.
const MAX_TEXTURE_EDGE: u32 = 2560;

/// Images at least this wide and tall would be uploaded block-compressed; 0 disables it (compression is synchronous).
const COMPRESS_MIN_DIMENSION: u32 = 0;

/// Longest the UI thread spends uploading decoded images in one frame; the rest wait for the next.
const UPLOAD_BUDGET: std::time::Duration = std::time::Duration::from_millis(4);

#[derive(Clone)]
pub enum ImageSource {
    File(PathBuf),
    Bytes(Arc<[u8]>),
    /// Downloaded on the worker (project icons from Modrinth/CurseForge).
    Url(String),
}

/// A texture ready to draw.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Picture {
    pub texture: RendererTextureHandle,
    pub width: u32,
    pub height: u32,
}

impl Picture {
    pub fn aspect(&self) -> f32 {
        if self.height == 0 {
            1.0
        } else {
            self.width as f32 / self.height as f32
        }
    }
}

enum Slot {
    Loading,
    Failed,
    Ready { asset: Asset, picture: Picture },
}

struct Entry {
    slot: Slot,
    last_used: u64,
}

type Decoded = (String, Result<DecodedImage, String>);

pub struct ImageCache {
    entries: HashMap<String, Entry>,
    tx: mpsc::Sender<Decoded>,
    rx: mpsc::Receiver<Decoded>,
    tick: u64,
}

impl Default for ImageCache {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            entries: HashMap::new(),
            tx,
            rx,
            tick: 0,
        }
    }
}

impl ImageCache {
    /// The picture for `key`, starting its load on first use. `max_dimension` asks the decoder for a cheap
    /// reduced-size decode when the source is much bigger (thumbnails of large screenshots).
    pub fn get(
        &mut self,
        key: &str,
        source: impl FnOnce() -> ImageSource,
        max_dimension: u32,
    ) -> Option<Picture> {
        let tick = self.tick;
        if let Some(entry) = self.entries.get_mut(key) {
            entry.last_used = tick;
            return match &entry.slot {
                Slot::Ready { picture, .. } => Some(*picture),
                Slot::Loading | Slot::Failed => None,
            };
        }
        self.entries.insert(
            key.to_owned(),
            Entry {
                slot: Slot::Loading,
                last_used: tick,
            },
        );
        let source = source();
        let tx = self.tx.clone();
        let key = key.to_owned();
        launcher_runtime::spawn_blocking_detached(move || {
            let result = load(&source, &key, max_dimension);
            let _ = tx.send((key, result));
            vertex_shell::wake();
        });
        None
    }

    /// Whether `key` failed to load (so the caller can draw a placeholder instead of waiting).
    pub fn failed(&self, key: &str) -> bool {
        self.entries
            .get(key)
            .is_some_and(|entry| matches!(entry.slot, Slot::Failed))
    }

    /// Drops `key`, so the next `get` loads it again (the file changed).
    pub fn invalidate(&mut self, engine: &mut Engine<'_>, key: &str) {
        if let Some(Entry {
            slot: Slot::Ready { asset, .. },
            ..
        }) = self.entries.remove(key)
        {
            let _ = engine.assets().unload(asset);
        }
    }

    /// Uploads freshly decoded images and unloads the least recently used ones. Call once per frame.
    pub fn pump(&mut self, engine: &mut Engine<'_>) {
        self.tick += 1;
        let started = std::time::Instant::now();
        loop {
            if started.elapsed() > UPLOAD_BUDGET {
                // Out of time: the rest stay queued; look again next frame.
                vertex_shell::wake();
                break;
            }
            let Ok((key, result)) = self.rx.try_recv() else {
                break;
            };
            let Some(entry) = self.entries.get_mut(&key) else {
                continue;
            };
            entry.slot = match result.and_then(|image| upload(engine, &key, &image)) {
                Ok((asset, picture)) => Slot::Ready { asset, picture },
                Err(err) => {
                    tracing::debug!(target: "vertexlauncher/se5/images", "Could not load image {key}: {err}");
                    Slot::Failed
                }
            };
        }
        self.trim(engine);
    }

    /// Something is still loading; the UI should look again soon.
    pub fn loading(&self) -> bool {
        self.entries
            .values()
            .any(|entry| matches!(entry.slot, Slot::Loading))
    }

    fn trim(&mut self, engine: &mut Engine<'_>) {
        let ready = self
            .entries
            .values()
            .filter(|entry| matches!(entry.slot, Slot::Ready { .. }))
            .count();
        if ready <= MAX_TEXTURES {
            return;
        }
        let mut by_age: Vec<(u64, String)> = self
            .entries
            .iter()
            .filter(|(_, entry)| matches!(entry.slot, Slot::Ready { .. }))
            .map(|(key, entry)| (entry.last_used, key.clone()))
            .collect();
        by_age.sort();
        for (_, key) in by_age.into_iter().take(ready - MAX_TEXTURES) {
            if let Some(Entry {
                slot: Slot::Ready { asset, .. },
                ..
            }) = self.entries.remove(&key)
            {
                let _ = engine.assets().unload(asset);
            }
        }
    }
}

fn load(source: &ImageSource, key: &str, max_dimension: u32) -> Result<DecodedImage, String> {
    let bytes: std::borrow::Cow<'_, [u8]> = match source {
        ImageSource::File(path) => std::borrow::Cow::Owned(
            match vertex_shell::sturdy::images::read_file_accelerated(path) {
                Some(bytes) => bytes,
                None => std::fs::read(path).map_err(|err| err.to_string())?,
            },
        ),
        ImageSource::Bytes(bytes) => std::borrow::Cow::Borrowed(bytes),
        ImageSource::Url(url) => std::borrow::Cow::Owned(download(url)?),
    };
    let options = DecodeOptions {
        preview_min_dimension: max_dimension,
        ..DecodeOptions::default()
    };
    let image = decode(&bytes, options, key).map_err(|err| err.to_string())?;
    if image.format != PixelFormat::Rgba8 {
        return Err("unsupported pixel format".to_owned());
    }
    Ok(fit_texture(image))
}

/// Shrinks pictures bigger than any screen shows them (4K+ screenshots) on the worker, so the texture the UI thread
/// has to create and mip is a sane size.
fn fit_texture(mut image: DecodedImage) -> DecodedImage {
    let longest = image.width.max(image.height);
    if longest <= MAX_TEXTURE_EDGE {
        return image;
    }
    let scale = MAX_TEXTURE_EDGE as f32 / longest as f32;
    let (width, height) = (
        ((image.width as f32 * scale).round() as u32).max(1),
        ((image.height as f32 * scale).round() as u32).max(1),
    );
    let Some(source) =
        image::RgbaImage::from_raw(image.width, image.height, image.frames[0].pixels.clone())
    else {
        return image;
    };
    let resized = image::imageops::resize(
        &source,
        width,
        height,
        image::imageops::FilterType::Triangle,
    );
    image.frames.truncate(1);
    image.frames[0].pixels = resized.into_raw();
    image.width = width;
    image.height = height;
    image
}

fn upload(
    engine: &mut Engine<'_>,
    key: &str,
    image: &DecodedImage,
) -> Result<(Asset, Picture), String> {
    let mut assets = engine.assets();
    let asset = assets
        .create_texture(
            image.pixels(),
            image.width,
            image.height,
            TexturePixelFormat::Rgba8,
            TextureColorSpace::Srgb,
            TextureKind::ColorAlpha,
            key,
            // The engine compresses on the calling thread, which stalled the UI with a screenshot grid; opt-in only.
            COMPRESS_MIN_DIMENSION != 0 && image.width.min(image.height) >= COMPRESS_MIN_DIMENSION,
            true,
        )
        .map_err(|err| format!("{err:?}"))?;
    let texture = assets
        .texture_handle(asset)
        .map_err(|err| format!("{err:?}"))?;
    Ok((
        asset,
        Picture {
            texture,
            width: image.width,
            height: image.height,
        },
    ))
}

/// Largest remote image accepted; project icons and banners are far below this.
const MAX_DOWNLOAD_BYTES: u64 = 16 * 1024 * 1024;

fn download(url: &str) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let mut response = ureq::get(url)
        .call()
        .map_err(|err| format!("failed to download {url}: {err}"))?;
    let mut bytes = Vec::new();
    response
        .body_mut()
        .as_reader()
        .take(MAX_DOWNLOAD_BYTES)
        .read_to_end(&mut bytes)
        .map_err(|err| format!("failed to read {url}: {err}"))?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_large_screenshot_decodes_at_full_size() {
        let dir = std::env::temp_dir().join("vertex-image-probe");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("big.png");
        image::RgbImage::from_fn(3840, 2160, |x, y| {
            image::Rgb([(x % 255) as u8, (y % 255) as u8, 90])
        })
        .save(&path)
        .unwrap();
        let started = std::time::Instant::now();
        let decoded = load(&ImageSource::File(path.clone()), "probe", 0).expect("decodes");
        println!("full decode took {:?}", started.elapsed());
        assert_eq!((decoded.width, decoded.height), (2560, 1440));
        let _ = std::fs::remove_file(path);
    }
}
