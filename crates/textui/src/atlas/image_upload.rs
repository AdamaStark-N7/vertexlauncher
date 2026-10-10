use super::*;

pub(super) fn swash_image_to_color_image(image: &cosmic_text::SwashImage) -> Option<ColorImage> {
    let width = image.placement.width as usize;
    let height = image.placement.height as usize;
    if width == 0 || height == 0 {
        return None;
    }

    let pixels = match image.content {
        SwashContent::Mask => image
            .data
            .iter()
            .map(|alpha| Color32::from_white_alpha(*alpha))
            .collect::<Vec<_>>(),
        SwashContent::Color | SwashContent::SubpixelMask => image
            .data
            .chunks_exact(4)
            .map(|rgba| Color32::from_rgba_unmultiplied(rgba[0], rgba[1], rgba[2], rgba[3]))
            .collect::<Vec<_>>(),
    };

    Some(ColorImage::new([width, height], pixels))
}

pub(super) fn build_atlas_upload_image(glyph: &ColorImage, padding: usize) -> ColorImage {
    let mut upload = ColorImage::filled(
        [
            glyph.size[0].saturating_add(padding * 2),
            glyph.size[1].saturating_add(padding * 2),
        ],
        Color32::TRANSPARENT,
    );
    blit_color_image(&mut upload, glyph, padding, padding);
    upload
}

pub(crate) fn blit_color_image(
    dest: &mut ColorImage,
    src: &ColorImage,
    dest_x: usize,
    dest_y: usize,
) {
    let dest_width = dest.size[0];
    let copy_width = src.size[0].min(dest_width.saturating_sub(dest_x));
    if copy_width == 0 {
        return;
    }
    for y in 0..src.size[1] {
        let target_y = dest_y + y;
        if target_y >= dest.size[1] {
            break;
        }
        let src_start = y * src.size[0];
        let dest_start = target_y * dest_width + dest_x;
        dest.pixels[dest_start..dest_start + copy_width]
            .copy_from_slice(&src.pixels[src_start..src_start + copy_width]);
    }
}

pub(super) fn color_image_sub_image(src: &ColorImage, rect: DirtyAtlasRect) -> ColorImage {
    let size = rect.size();
    let mut image = ColorImage::filled(size, Color32::TRANSPARENT);
    let src_width = src.size[0];
    let dst_width = size[0];
    for y in 0..size[1] {
        let src_y = rect.min[1] + y;
        let src_start = src_y * src_width + rect.min[0];
        let dst_start = y * dst_width;
        image.pixels[dst_start..dst_start + dst_width]
            .copy_from_slice(&src.pixels[src_start..src_start + dst_width]);
    }
    image
}

pub(super) fn write_color_image_to_wgpu_texture(
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    pos: [usize; 2],
    image: &ColorImage,
) {
    if image.size[0] == 0 || image.size[1] == 0 {
        return;
    }
    let bytes = color_image_to_rgba16f_bytes(image);
    let size = wgpu::Extent3d {
        width: image.size[0] as u32,
        height: image.size[1] as u32,
        depth_or_array_layers: 1,
    };
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d {
                x: pos[0] as u32,
                y: pos[1] as u32,
                z: 0,
            },
            aspect: wgpu::TextureAspect::All,
        },
        &bytes,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(8 * image.size[0] as u32),
            rows_per_image: Some(image.size[1] as u32),
        },
        size,
    );
}

pub(super) fn color_image_to_rgba16f_bytes(image: &ColorImage) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(image.pixels.len().saturating_mul(8));
    for pixel in &image.pixels {
        for channel in pixel.to_array() {
            let half = half::f16::from_f32(f32::from(channel) / 255.0);
            bytes.extend_from_slice(&half.to_bits().to_le_bytes());
        }
    }
    bytes
}
