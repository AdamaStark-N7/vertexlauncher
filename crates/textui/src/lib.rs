use std::{
    cmp::Ordering,
    collections::{BTreeSet, VecDeque},
    hash::{Hash, Hasher},
    mem,
    sync::{Arc, Mutex, mpsc},
};

const TEXT_WGPU_INSTANCED_SHADER: &str = include_str!("shaders/text_instanced.wgsl");

use bytemuck::{Pod, Zeroable};
use cosmic_text::{
    Action, Affinity, Attrs, AttrsOwned, BorrowedWithFontSystem, Buffer, CacheKey, Color, Cursor,
    Edit, Editor, Family, FontFeatures, FontSystem, LayoutGlyph, LayoutRun, Metrics, Motion,
    Selection, Shaping, Style as FontStyle, SubpixelBin, SwashContent, SwashImage, Weight, Wrap,
    fontdb,
};
use egui::{
    self, Color32, ColorImage, Context, CornerRadius, Id, Key, Pos2, Rect, Response, Sense,
    TextureHandle, TextureId, TextureOptions, Ui, Vec2,
};
use egui_wgpu::RenderState as EguiWgpuRenderState;
use etagere::{AllocId, Allocation, AtlasAllocator, size2};
use launcher_runtime as tokio_runtime;
use rustc_hash::{FxHashMap, FxHashSet, FxHasher};
use shared_lru::ThreadSafeLru;
use skrifa::raw::{FontRef as SkrifaFontRef, TableProvider as _};
use swash::scale::{Render, ScaleContext, Source, StrikeWith};
use swash::zeno::{
    Angle as SwashAngle, Format as SwashFormat, Transform as SwashTransform, Vector as SwashVector,
};
use swash::{Setting as SwashSetting, Tag as SwashTag};
use syntect::easy::HighlightLines;
use syntect::highlighting::{FontStyle as SyntectFontStyle, Theme, ThemeSet};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;
use tracing::warn;
use unicode_segmentation::UnicodeSegmentation;
use wgpu::util::DeviceExt as _;

mod advanced_text;
#[path = "text_ui_async_raster.rs"]
mod async_raster;
mod atlas;
mod button_options;
mod clipboard;
mod code_block_options;
mod conversions;
mod cursor_layout;
mod editor;
mod font_features;
mod geometry;
mod gpu;
mod input_options;
#[path = "text_ui_input_runtime.rs"]
mod input_runtime;
#[path = "text_ui_input_widget.rs"]
mod input_widget;
mod label_options;
#[path = "text_ui_label_scene.rs"]
mod label_scene;
mod markdown_options;
mod markdown_parser;
mod path_layout;
#[path = "text_ui_path_text.rs"]
mod path_text;
mod prepared_layout;
#[path = "text_ui_scene_builder.rs"]
mod scene_builder;
mod script_fonts;
mod text_helpers;
#[path = "text_ui.rs"]
mod text_ui;
#[path = "text_ui_async_widgets.rs"]
mod text_ui_async_widgets;
#[path = "text_ui_code_markdown.rs"]
mod text_ui_code_markdown;
#[path = "text_ui_frame.rs"]
mod text_ui_frame;
#[path = "text_ui_gpu_scene_assembly.rs"]
mod text_ui_gpu_scene_assembly;
#[path = "text_ui_gpu_scenes.rs"]
mod text_ui_gpu_scenes;
#[path = "text_ui_input_attrs.rs"]
mod text_ui_input_attrs;
#[path = "text_ui_input_chrome.rs"]
mod text_ui_input_chrome;
#[path = "text_ui_input_editor_state.rs"]
mod text_ui_input_editor_state;
#[path = "text_ui_input_ime.rs"]
mod text_ui_input_ime;
#[path = "text_ui_input_paint.rs"]
mod text_ui_input_paint;
#[path = "text_ui_input_undo.rs"]
mod text_ui_input_undo;
#[path = "text_ui_input_viewer.rs"]
mod text_ui_input_viewer;
#[path = "text_ui_label_button.rs"]
mod text_ui_label_button;
#[path = "text_ui_typography.rs"]
mod text_ui_typography;
mod tooltip_options;

pub use self::text_ui::TextUi;
use crate::async_raster::{AsyncRasterState, AsyncRasterWorkerMessage, new_async_raster_state};
pub(crate) use crate::atlas::{
    GlyphAtlas, GlyphContentMode, GlyphRasterKey, PaintTextQuad, PreparedAtlasGlyph,
    adjusted_glyph_right_px, adjusted_glyph_x_px, collect_glyph_spacing_prefixes_px,
    collect_prepared_glyphs_from_buffer, cursor_stops_for_glyphs, glyph_logical_font_size_points,
    hash_text_fundamentals, hit_buffer_with_fundamentals, rasterize_atlas_glyph,
    render_swash_outline_commands, shared_variation_settings,
};
pub(crate) use crate::conversions::{
    core_label_options, cosmic_to_egui_color, egui_key_from_text, egui_modifiers_from_text,
    glyph_content_mode_from_rasterization, multiply_color32, texture_options_for_sampling,
    to_cosmic_color, to_cosmic_text_color, wgpu_filter_mode_for_sampling,
};
use crate::cursor_layout::{editor_cursor_x_in_run, editor_sel_rect};
use crate::editor::{
    EditorScrollMetrics, InputState, UndoEntry, UndoOpKind, clamp_borrowed_buffer_scroll,
    clamp_cursor_to_editor, clamp_selection_to_editor, click_editor_to_pointer,
    double_click_editor_to_pointer, drag_editor_selection_to_pointer, editor_horizontal_scroll,
    editor_to_string, extend_selection_to_pointer, handle_editor_key_event,
    handle_read_only_editor_key_event, is_navigation_event, measure_borrowed_buffer_scroll_metrics,
    measure_buffer_pixels, pending_modify_op, push_undo, scroll_editor_to_buffer_end, select_all,
    triple_click_editor_to_pointer, viewer_scrollbar_track_rects, viewer_visible_text_rect,
};
pub(crate) use crate::font_features::{
    build_font_features, compose_font_features, configure_text_font_defaults, opsz_for_font_size,
    parse_feature_tag_list, resolved_hinting_enabled, resolved_stem_darkening_strength,
};
use crate::geometry::{
    egui_point_from_text, egui_rect_from_text, egui_vec_from_text, snap_rect_to_pixel_grid,
    snap_width_to_bin,
};
use crate::gpu::{
    CpuSceneAtlasPage, ResolvedTextGraphicsConfig, ResolvedTextRendererBackend, TextWgpuInstance,
    TextWgpuPreparedScene, TextWgpuSceneBatchSource, TextWgpuSceneCallback,
    allocate_cpu_scene_page_slot, blit_to_page, color_image_to_page_data, cpu_page_to_page_data,
    default_gpu_scene_page_side, fit_scene_page_side, gpu_scene_approx_bytes,
    gpu_scene_page_batches_approx_bytes, map_scene_quads_to_rect, paint_text_quads_fallback,
    quad_positions_from_min_size, rect_from_points, rotated_quad_positions, uv_quad_points,
};
use crate::input_runtime::apply_gamepad_scroll_if_focused;
use crate::path_layout::{
    build_path_layout_from_prepared_layout, export_prepared_layout_as_shapes,
};
pub(crate) use crate::prepared_layout::{
    PreparedGlyph, PreparedTextCacheEntry, PreparedTextLayout,
};
use crate::script_fonts::ScriptFonts;

/// Font file bytes shared between the main text engine and its worker threads without copying.
pub(crate) type SharedFontBytes = Arc<dyn AsRef<[u8]> + Send + Sync>;
use crate::text_ui_gpu_scene_assembly::GpuSceneAssembly;
use crate::text_ui_input_chrome::{
    apply_input_context_actions, lock_input_focus_filter, paint_input_frame,
};
use crate::text_ui_input_ime::{
    PreeditRange, clear_preedit, commit_preedit, cursor_without_preedit, set_preedit,
    text_without_preedit,
};
use crate::text_ui_input_undo::UndoDirection;
use crate::{
    button_options::ButtonOptions, clipboard::copy_sanitized, code_block_options::CodeBlockOptions,
    input_options::InputOptions, label_options::LabelOptions, markdown_options::MarkdownOptions,
    markdown_parser::parse_markdown_blocks, tooltip_options::TooltipOptions,
};

pub use advanced_text::DEFAULT_ELLIPSIS;
pub use advanced_text::{
    RichTextSpan, RichTextStyle, TextAtlasPageData, TextAtlasPageSnapshot, TextAtlasQuad,
    TextAtlasSampling, TextColor, TextFeatureSetting, TextFrameInfo, TextFrameOutput,
    TextFundamentals, TextGlyphRasterMode, TextGpuPowerPreference, TextGpuQuad, TextGpuScene,
    TextGpuSceneDrawOptions, TextGpuScenePageBatch, TextGraphicsApi, TextGraphicsConfig,
    TextHintingMode, TextInputEvent, TextKerning, TextKey, TextLabelOptions, TextMarkdownBlock,
    TextMarkdownHeadingLevel, TextModifiers, TextOpticalSizingMode, TextPath, TextPathError,
    TextPathGlyph, TextPathLayout, TextPathOptions, TextPoint, TextPointerButton,
    TextRasterizationConfig, TextRect, TextRenderScene, TextRendererBackend, TextRenderingPolicy,
    TextStemDarkeningMode, TextVariationSetting, TextVector, VectorGlyphShape, VectorPathCommand,
    VectorTextShape,
};
pub use clipboard::{apply_smart_quotes, sanitize_for_clipboard};
pub use conversions::{
    wgpu_backends_for_text_graphics_api, wgpu_power_preference_for_text_gpu_preference,
};
#[doc(hidden)]
pub use input_options::InputOptions as EguiInputOptions;

/// Default OpenType feature tags applied when no explicit feature string is
/// provided to [`TextUi::apply_open_type_features`].
pub const DEFAULT_OPEN_TYPE_FEATURE_TAGS: &str = "kern, liga, calt, onum, pnum";
const PREPARED_TEXT_CACHE_MAX_BYTES: usize = 16 * 1024 * 1024;
const ASYNC_RASTER_CACHE_MAX_BYTES: usize = 24 * 1024 * 1024;
const GPU_SCENE_CACHE_MAX_BYTES: usize = 32 * 1024 * 1024;
const GPU_SCENE_PAGE_BATCH_CACHE_MAX_BYTES: usize = 24 * 1024 * 1024;
const GPU_SCENE_DRAW_BATCH_CACHE_MAX_BYTES: usize = 32 * 1024 * 1024;
const GPU_SCENE_GLYPH_CACHE_MAX_BYTES: usize = 24 * 1024 * 1024;
const GLYPH_ATLAS_MAX_BYTES: usize = 64 * 1024 * 1024;
const GLYPH_ATLAS_STALE_FRAMES: u64 = 900;
const GLYPH_ATLAS_PAGE_TARGET_PX: usize = 1024;
const GLYPH_ATLAS_PADDING_PX: i32 = 1;
const GLYPH_ATLAS_FETCH_MAX_PER_FRAME: usize = 128;
const GLYPH_ATLAS_UPLOAD_MAX_GLYPHS_PER_FRAME: usize = 64;
const GLYPH_ATLAS_UPLOAD_MAX_BYTES_PER_FRAME: usize = 512 * 1024;
const AUTO_MSDF_MIN_LOGICAL_FONT_SIZE_PT: f32 = 28.0;
const FIELD_GLYPH_MEAN_ALPHA_ERROR_LIMIT: f32 = 0.045;
const FIELD_GLYPH_MAX_ALPHA_ERROR_LIMIT: f32 = 0.25;
const FIELD_GLYPH_LARGE_ERROR_PIXEL_RATIO_LIMIT: f32 = 0.02;
const TEXT_WGPU_PASS_DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const TEXT_WGPU_PASS_MSAA_SAMPLES: u32 = 4;
const INPUT_STATE_STALE_FRAMES: u64 = 900;
const UNDO_STACK_MAX: usize = 200;

// Width-bin size in device pixels.  Labels whose available width differs by
// less than this will share the same cached texture, preventing mass cache
// busts from sub-pixel layout jitter (scrollbars, fractional DPI, etc.).
const WIDTH_BIN_PX: f32 = 16.0;

/// Snap a point-space width to the nearest WIDTH_BIN_PX device-pixel boundary.
#[inline]
fn color_image_byte_size(image: &ColorImage) -> usize {
    color_image_byte_size_from_size(image.size)
}

fn color_image_byte_size_from_size(size: [usize; 2]) -> usize {
    size[0]
        .saturating_mul(size[1])
        .saturating_mul(mem::size_of::<Color32>())
}

#[inline]
fn new_fingerprint_hasher() -> FxHasher {
    FxHasher::default()
}

type SpanStyle = RichTextStyle;
type RichSpan = RichTextSpan;
