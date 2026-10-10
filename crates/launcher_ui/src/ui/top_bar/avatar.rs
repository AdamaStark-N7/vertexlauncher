use super::*;
use std::sync::{Arc, OnceLock};

use image::{ColorType, ImageEncoder, codecs::png::PngEncoder};
use shared_lru::ThreadSafeLru;

pub(super) fn rounded_profile_avatar_png(
    cache_key: u64,
    avatar_png: &[u8],
    radius: u8,
) -> Arc<[u8]> {
    static CACHE: OnceLock<ThreadSafeLru<u64, Arc<[u8]>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| ThreadSafeLru::new(usize::MAX));

    if let Some(bytes) = cache.write(|state| {
        state
            .touch(&cache_key)
            .map(|entry| Arc::clone(&entry.value))
    }) {
        return bytes;
    }

    let rounded = Arc::<[u8]>::from(
        round_avatar_png_bytes(avatar_png, radius).unwrap_or_else(|| avatar_png.to_vec()),
    );

    cache.write(|state| {
        state.insert_without_eviction(cache_key, Arc::clone(&rounded), rounded.len());
        while state.len() > ROUNDED_AVATAR_CACHE_MAX_ENTRIES {
            if state.pop_lru().is_none() {
                break;
            }
        }
    });

    rounded
}

pub(super) fn round_avatar_png_bytes(avatar_png: &[u8], radius: u8) -> Option<Vec<u8>> {
    let mut image = image::load_from_memory(avatar_png).ok()?.to_rgba8();
    let width = image.width();
    let height = image.height();
    let radius = radius as i32;
    let width_i32 = i32::try_from(width).ok()?;
    let height_i32 = i32::try_from(height).ok()?;

    for y in 0..height {
        for x in 0..width {
            let x_i32 = i32::try_from(x).ok()?;
            let y_i32 = i32::try_from(y).ok()?;
            let dx = if x_i32 < radius {
                radius - 1 - x_i32
            } else if x_i32 >= width_i32 - radius {
                x_i32 - (width_i32 - radius)
            } else {
                0
            };
            let dy = if y_i32 < radius {
                radius - 1 - y_i32
            } else if y_i32 >= height_i32 - radius {
                y_i32 - (height_i32 - radius)
            } else {
                0
            };

            if dx > 0 || dy > 0 {
                let distance_sq = dx * dx + dy * dy;
                if distance_sq >= radius * radius {
                    image.get_pixel_mut(x, y).0[3] = 0;
                }
            }
        }
    }

    let mut bytes = Vec::new();
    PngEncoder::new(&mut bytes)
        .write_image(image.as_raw(), width, height, ColorType::Rgba8.into())
        .ok()?;
    Some(bytes)
}
