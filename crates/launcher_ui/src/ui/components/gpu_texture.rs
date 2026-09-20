//! GPU-resident BC7 textures registered with egui's wgpu renderer.
//!
//! Available only when the device was created with `TEXTURE_COMPRESSION_BC` (see
//! `native_options.rs`); otherwise [`is_available`] is false and callers fall back to a
//! downscaled RGBA texture. Dropping a [`GpuBc7Texture`] frees the egui registration and the
//! GPU memory, so cache eviction is just dropping the `Arc`.

use std::sync::{Arc, OnceLock};

use egui_wgpu::RenderState;

use super::bc7::{Bc7Image, Bc7Tile};

struct GpuContext {
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: Arc<egui::mutex::RwLock<egui_wgpu::Renderer>>,
}

static GPU: OnceLock<Option<GpuContext>> = OnceLock::new();

/// Records the render state once at startup. Safe to call every frame.
pub fn install(render_state: &RenderState) {
    GPU.get_or_init(|| {
        let supported = render_state
            .device
            .features()
            .contains(wgpu::Features::TEXTURE_COMPRESSION_BC);
        tracing::info!(
            target: "vertexlauncher/gpu_texture",
            bc_compression = supported,
            "GPU BC texture compression availability"
        );
        supported.then(|| GpuContext {
            device: render_state.device.clone(),
            queue: render_state.queue.clone(),
            renderer: Arc::clone(&render_state.renderer),
        })
    });
}

pub fn is_available() -> bool {
    matches!(GPU.get(), Some(Some(_)))
}

struct GpuBc7Texture {
    id: egui::TextureId,
    /// Kept alive for as long as egui's bind group references its view. Moved to the deferred
    /// free list on drop.
    texture: Option<wgpu::Texture>,
    renderer: Arc<egui::mutex::RwLock<egui_wgpu::Renderer>>,
    size: egui::Vec2,
    uv_scale: egui::Vec2,
    gpu_bytes: usize,
}

impl GpuBc7Texture {
    /// Uploads every mip of `image`. Returns `None` if GPU compression is unavailable.
    fn upload(label: &str, image: &Bc7Image) -> Option<Self> {
        let gpu = GPU.get()?.as_ref()?;
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: image.padded_width,
                height: image.padded_height,
                depth_or_array_layers: 1,
            },
            mip_level_count: image.mips.len() as u32,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            // Not the sRGB variant: egui's shader does its own gamma handling and expects
            // gamma-encoded values straight from the sampler (its own textures are Rgba8Unorm).
            format: wgpu::TextureFormat::Bc7RgbaUnorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        for (level, mip) in image.mips.iter().enumerate() {
            gpu.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: level as u32,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &mip.data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(mip.blocks_x * 16),
                    rows_per_image: Some(mip.blocks_y),
                },
                // Compressed copies cover whole blocks, even for sub-block mips.
                wgpu::Extent3d {
                    width: mip.blocks_x * 4,
                    height: mip.blocks_y * 4,
                    depth_or_array_layers: 1,
                },
            );
        }
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let id = gpu
            .renderer
            .write()
            .register_native_texture_with_sampler_options(
                &gpu.device,
                &view,
                wgpu::SamplerDescriptor {
                    label: Some(label),
                    mag_filter: wgpu::FilterMode::Linear,
                    min_filter: wgpu::FilterMode::Linear,
                    mipmap_filter: wgpu::MipmapFilterMode::Linear,
                    ..Default::default()
                },
            );
        let [u, v] = image.uv_scale();
        Some(Self {
            id,
            texture: Some(texture),
            renderer: Arc::clone(&gpu.renderer),
            size: egui::vec2(image.width as f32, image.height as f32),
            uv_scale: egui::vec2(u, v),
            gpu_bytes: image.byte_size(),
        })
    }

    fn id(&self) -> egui::TextureId {
        self.id
    }

    /// Logical (unpadded) pixel size.
    fn size(&self) -> egui::Vec2 {
        self.size
    }

    /// Multiply source UVs by this to stay inside the un-padded image area.
    fn uv_scale(&self) -> egui::Vec2 {
        self.uv_scale
    }

    fn gpu_bytes(&self) -> usize {
        self.gpu_bytes
    }
}

/// Textures released this frame. They may still be referenced by shapes egui has already queued
/// for painting, so they are freed at the start of the next frame instead of immediately.
static DEFERRED_FREES: std::sync::Mutex<Vec<DeferredFree>> = std::sync::Mutex::new(Vec::new());

struct DeferredFree {
    renderer: Arc<egui::mutex::RwLock<egui_wgpu::Renderer>>,
    id: egui::TextureId,
    texture: Option<wgpu::Texture>,
}

/// Frees textures dropped during the previous frame. Call once at the start of every frame.
pub fn flush_deferred_frees() {
    let pending = match DEFERRED_FREES.lock() {
        Ok(mut list) => std::mem::take(&mut *list),
        Err(_) => return,
    };
    for free in pending {
        free.renderer.write().free_texture(&free.id);
        drop(free.texture);
    }
}

impl Drop for GpuBc7Texture {
    fn drop(&mut self) {
        if let Ok(mut list) = DEFERRED_FREES.lock() {
            list.push(DeferredFree {
                renderer: Arc::clone(&self.renderer),
                id: self.id,
                texture: self.texture.take(),
            });
        }
    }
}

struct GpuTile {
    x: u32,
    y: u32,
    texture: GpuBc7Texture,
}

/// A BC7 image split into tiles that each fit the adapter's texture size limit. Most images are a
/// single tile.
pub struct TiledBc7Texture {
    width: u32,
    height: u32,
    tiles: Vec<GpuTile>,
    gpu_bytes: usize,
}

impl TiledBc7Texture {
    pub fn upload(label: &str, tiles: &[Bc7Tile]) -> Option<Self> {
        let mut out = Vec::with_capacity(tiles.len());
        let (mut width, mut height) = (0, 0);
        for tile in tiles {
            width = width.max(tile.x + tile.image.width);
            height = height.max(tile.y + tile.image.height);
            out.push(GpuTile {
                x: tile.x,
                y: tile.y,
                texture: GpuBc7Texture::upload(label, &tile.image)?,
            });
        }
        let gpu_bytes = out.iter().map(|t| t.texture.gpu_bytes()).sum();
        Some(Self {
            width,
            height,
            tiles: out,
            gpu_bytes,
        })
    }

    pub fn gpu_bytes(&self) -> usize {
        self.gpu_bytes
    }

    /// For an image that fit in one texture: its id, pixel size and UV extent.
    pub fn single(&self) -> Option<(egui::TextureId, egui::Vec2, egui::Vec2)> {
        match self.tiles.as_slice() {
            [tile] => Some((
                tile.texture.id(),
                tile.texture.size(),
                tile.texture.uv_scale(),
            )),
            _ => None,
        }
    }

    pub fn paint(
        &self,
        ui: &egui::Ui,
        rect: egui::Rect,
        uv: egui::Rect,
        corner_radius: egui::CornerRadius,
    ) {
        let tiles: Vec<TileDraw> = self
            .tiles
            .iter()
            .map(|tile| TileDraw {
                x: tile.x,
                y: tile.y,
                size: tile.texture.size(),
                id: tile.texture.id(),
                uv_scale: tile.texture.uv_scale(),
            })
            .collect();
        paint_tiles(
            ui,
            rect,
            uv,
            corner_radius,
            [self.width, self.height],
            &tiles,
        );
    }
}

/// One texture of a tiled image, as needed for drawing.
pub struct TileDraw {
    pub x: u32,
    pub y: u32,
    /// Un-padded pixel size of the tile.
    pub size: egui::Vec2,
    pub id: egui::TextureId,
    /// Fraction of the texture holding real pixels per axis (1.0 unless block-padded).
    pub uv_scale: egui::Vec2,
}

/// Paints `uv` (a sub-rect of the whole image, 0..1) into `rect`, drawing only the tiles it
/// touches. Shared by compressed and uncompressed tiled textures so they behave identically.
pub fn paint_tiles(
    ui: &egui::Ui,
    rect: egui::Rect,
    uv: egui::Rect,
    corner_radius: egui::CornerRadius,
    image_size: [u32; 2],
    tiles: &[TileDraw],
) {
    let (w, h) = (image_size[0] as f32, image_size[1] as f32);
    let uv_size = uv.size().max(egui::vec2(f32::EPSILON, f32::EPSILON));
    let single = tiles.len() == 1;
    for tile in tiles {
        let tile_min = egui::pos2(tile.x as f32 / w, tile.y as f32 / h);
        let tile_max = egui::pos2(
            (tile.x as f32 + tile.size.x) / w,
            (tile.y as f32 + tile.size.y) / h,
        );
        let visible = egui::Rect::from_min_max(
            egui::pos2(uv.min.x.max(tile_min.x), uv.min.y.max(tile_min.y)),
            egui::pos2(uv.max.x.min(tile_max.x), uv.max.y.min(tile_max.y)),
        );
        if visible.width() <= 0.0 || visible.height() <= 0.0 {
            continue;
        }
        let to_screen = |p: egui::Pos2| {
            egui::pos2(
                rect.min.x + (p.x - uv.min.x) / uv_size.x * rect.width(),
                rect.min.y + (p.y - uv.min.y) / uv_size.y * rect.height(),
            )
        };
        let dest = egui::Rect::from_min_max(to_screen(visible.min), to_screen(visible.max));
        let local = |p: egui::Pos2| {
            egui::pos2(
                (p.x - tile_min.x) / (tile_max.x - tile_min.x) * tile.uv_scale.x,
                (p.y - tile_min.y) / (tile_max.y - tile_min.y) * tile.uv_scale.y,
            )
        };
        egui::Image::new(egui::load::SizedTexture::new(tile.id, tile.size))
            .fit_to_exact_size(dest.size())
            .maintain_aspect_ratio(false)
            .uv(egui::Rect::from_min_max(
                local(visible.min),
                local(visible.max),
            ))
            // Rounded corners only make sense when one texture covers the whole rect.
            .corner_radius(if single {
                corner_radius
            } else {
                egui::CornerRadius::ZERO
            })
            .paint_at(ui, dest);
    }
}
