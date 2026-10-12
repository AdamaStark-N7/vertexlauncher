mod button_options;
mod button_tooltip_widgets;
mod code_block_options;
mod code_markdown_widgets;
mod ext_impl;
#[path = "gamepad_scroll.rs"]
mod gamepad_scroll;
mod gpu_paint;
#[path = "input_events.rs"]
mod input_events;
mod input_options;
pub mod interaction;
mod label_options;
mod label_widget;
mod markdown_options;
mod pixel_snap;
mod scene_cache;
mod scene_hash;
mod text_helpers;
mod text_scrubber;
mod tooltip_options;

use egui::{
    Color32, Context, CornerRadius, Id, Painter, Rect, Response, Sense, TextureHandle, TextureId,
    TextureOptions, Ui, Vec2,
};
use egui_wgpu::RenderState;
use std::{
    collections::HashMap,
    hash::DefaultHasher,
    hash::{Hash, Hasher},
    sync::{Arc, Mutex},
};
use textui::{
    TextAtlasPageData, TextAtlasSampling, TextFrameInfo, TextFrameOutput, TextGpuScene,
    TextInputEvent, TextKey, TextMarkdownBlock, TextMarkdownHeadingLevel, TextModifiers, TextPath,
    TextPathError, TextPathLayout, TextPathOptions, TextPointerButton, TextRenderScene, TextUi,
};

pub use button_options::{ButtonOptions, ButtonTypography, set_button_typography};
use button_tooltip_widgets::*;
pub use button_tooltip_widgets::{TrailingIcon, button_with_trailing_icon};
pub use code_block_options::CodeBlockOptions;
use code_markdown_widgets::*;
pub use gamepad_scroll::*;
use gpu_paint::*;
use input_events::translate_input_events;
pub use input_options::InputOptions;
pub use label_options::LabelOptions;
use label_widget::*;
pub use markdown_options::MarkdownOptions;
use pixel_snap::*;
pub use scene_cache::release_untouched;
use scene_cache::*;
use scene_hash::*;
pub use text_helpers::{
    TruncatedText, normalize_inline_whitespace, truncate_single_line_text_with_ellipsis,
    truncate_single_line_text_with_ellipsis_detailed,
    truncate_single_line_text_with_ellipsis_preserving_whitespace,
    truncate_single_line_text_with_ellipsis_preserving_whitespace_detailed,
};
pub use text_scrubber::{
    TextScrubber, name_scrubbing_exempt, scrubbed, set_text_scrubber, without_name_scrubbing,
};
pub use textui::{RichTextSpan, RichTextStyle, TextColor};
pub use tooltip_options::TooltipOptions;

#[derive(Clone)]
pub struct TextTextureHandle {
    scene: Arc<TextGpuScene>,
    pub size_points: Vec2,
}

impl TextTextureHandle {
    pub fn scene(&self) -> &TextGpuScene {
        &self.scene
    }

    pub fn into_scene(self) -> Arc<TextGpuScene> {
        self.scene
    }

    pub fn paint(&self, text_ui: &mut TextUi, ui: &Ui, rect: Rect) {
        let painter = ui.painter().with_clip_rect(ui.clip_rect());
        paint_gpu_scene_in_rect(text_ui, &painter, rect, &self.scene, Color32::WHITE);
    }

    pub fn paint_tinted(&self, text_ui: &mut TextUi, ui: &Ui, rect: Rect, tint: egui::Color32) {
        let painter = ui.painter().with_clip_rect(ui.clip_rect());
        paint_gpu_scene_in_rect(text_ui, &painter, rect, &self.scene, tint);
    }

    pub fn paint_on(
        &self,
        text_ui: &mut TextUi,
        painter: &egui::Painter,
        rect: Rect,
        tint: egui::Color32,
    ) {
        paint_gpu_scene_in_rect(text_ui, painter, rect, &self.scene, tint);
    }
}

pub trait TextUiEguiExt {
    fn label_async<H: Hash>(
        &mut self,
        ui: &mut Ui,
        id_source: H,
        text: &str,
        options: &LabelOptions,
    ) -> Response;
    fn code_block_async<H: Hash>(
        &mut self,
        ui: &mut Ui,
        id_source: H,
        code: &str,
        options: &CodeBlockOptions,
    ) -> Response;
    fn label<H: Hash>(
        &mut self,
        ui: &mut Ui,
        id_source: H,
        text: &str,
        options: &LabelOptions,
    ) -> Response;
    fn clickable_label<H: Hash>(
        &mut self,
        ui: &mut Ui,
        id_source: H,
        text: &str,
        options: &LabelOptions,
    ) -> Response;
    fn measure_text_size(&mut self, ui: &Ui, text: &str, options: &LabelOptions) -> Vec2;
    /// Like [`Self::measure_text_size`] for code that only has a [`Context`].
    fn measure_text_size_ctx(&mut self, ctx: &Context, text: &str, options: &LabelOptions) -> Vec2;
    fn prepare_label_texture<H: Hash>(
        &mut self,
        ctx: &Context,
        id_source: H,
        text: &str,
        options: &LabelOptions,
        width_points_opt: Option<f32>,
    ) -> TextTextureHandle;
    fn prepare_rich_text_texture<H: Hash>(
        &mut self,
        ctx: &Context,
        id_source: H,
        spans: &[RichTextSpan],
        options: &LabelOptions,
        width_points_opt: Option<f32>,
    ) -> TextTextureHandle;
    fn paint_label_on_path<H: Hash>(
        &mut self,
        painter: &Painter,
        id_source: H,
        text: &str,
        options: &LabelOptions,
        width_points_opt: Option<f32>,
        path: &TextPath,
        path_options: &TextPathOptions,
    ) -> Result<TextPathLayout, TextPathError>;
    fn paint_rich_text_on_path<H: Hash>(
        &mut self,
        painter: &Painter,
        id_source: H,
        spans: &[RichTextSpan],
        options: &LabelOptions,
        width_points_opt: Option<f32>,
        path: &TextPath,
        path_options: &TextPathOptions,
    ) -> Result<TextPathLayout, TextPathError>;
    fn paint_scene_in_rect(&mut self, painter: &Painter, rect: Rect, scene: &TextRenderScene);
    fn paint_scene_in_rect_tinted(
        &mut self,
        painter: &Painter,
        rect: Rect,
        scene: &TextRenderScene,
        tint: egui::Color32,
    );
    fn button<H: Hash>(
        &mut self,
        ui: &mut Ui,
        id_source: H,
        text: &str,
        options: &ButtonOptions,
    ) -> Response;
    fn selectable_button<H: Hash>(
        &mut self,
        ui: &mut Ui,
        id_source: H,
        text: &str,
        selected: bool,
        options: &ButtonOptions,
    ) -> Response;
    fn tooltip_for_response<H: Hash>(
        &mut self,
        ui: &Ui,
        id_source: H,
        response: &Response,
        text: &str,
        options: &TooltipOptions,
    );
    /// Paints a single unwrapped line of styled spans with its top-left at `pos`, without
    /// allocating layout space. Returns the painted size. Scenes are cached per `id_source`.
    fn paint_rich_line<H: Hash>(
        &mut self,
        ui: &Ui,
        painter: &Painter,
        id_source: H,
        pos: egui::Pos2,
        spans: &[RichTextSpan],
        options: &LabelOptions,
    ) -> Vec2;
    /// Draws a tooltip at `pointer` without needing a widget response.
    fn tooltip_at<H: Hash>(
        &mut self,
        ctx: &Context,
        id_source: H,
        pointer: egui::Pos2,
        text: &str,
        options: &TooltipOptions,
    );
    fn code_block<H: Hash>(
        &mut self,
        ui: &mut Ui,
        id_source: H,
        code: &str,
        options: &CodeBlockOptions,
    ) -> Response;
    fn markdown<H: Hash>(
        &mut self,
        ui: &mut Ui,
        id_source: H,
        markdown: &str,
        options: &MarkdownOptions,
    );
    fn singleline_input<H: Hash>(
        &mut self,
        ui: &mut Ui,
        id_source: H,
        text: &mut String,
        options: &InputOptions,
    ) -> Response;
    fn multiline_input<H: Hash>(
        &mut self,
        ui: &mut Ui,
        id_source: H,
        text: &mut String,
        options: &InputOptions,
    ) -> Response;
    fn multiline_rich_viewer<H: Hash>(
        &mut self,
        ui: &mut Ui,
        id_source: H,
        spans: &[RichTextSpan],
        options: &InputOptions,
        stick_to_bottom: bool,
        wrap: bool,
    ) -> Response;
}

pub mod prelude {
    pub use super::{
        ButtonOptions, CodeBlockOptions, InputOptions, LabelOptions, MarkdownOptions, RichTextSpan,
        RichTextStyle, TextColor, TextTextureHandle, TextUiEguiExt, TooltipOptions, TruncatedText,
        normalize_inline_whitespace, truncate_single_line_text_with_ellipsis,
        truncate_single_line_text_with_ellipsis_detailed,
        truncate_single_line_text_with_ellipsis_preserving_whitespace,
        truncate_single_line_text_with_ellipsis_preserving_whitespace_detailed,
    };
}
