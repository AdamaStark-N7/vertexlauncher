//! The color picker shown under an account row in the accounts popup.

use super::*;
use egui::Color32;

use crate::account_colors::{self, PaletteColor};
use crate::account_text::mention;
use crate::ui::account_color::{color_preview_box, color_swatch_button};
use crate::ui::account_label::account_label;
use crate::ui::text_input_theme;

const SWATCH_SIZE: f32 = 20.0;
const CAPTION_WIDTH: f32 = 74.0;
const CUSTOM_INPUT_WIDTH: f32 = 124.0;
const INVALID_COLOR_HINT: &str = "Use #rrggbb or rgb(r, g, b)";

/// What has been typed in the custom color box, kept across frames while the editor is open.
#[derive(Clone, Default)]
struct CustomColorInput {
    text: String,
    invalid: bool,
}

impl CustomColorInput {
    fn showing(color: Option<Color32>) -> Self {
        Self {
            text: color.map(account_colors::format_hex).unwrap_or_default(),
            invalid: false,
        }
    }

    /// Applies the typed color to `account_id`, or flags the text as invalid.
    fn apply(&mut self, account_id: &str) {
        match account_colors::parse_color(&self.text) {
            Some(color) => {
                account_colors::set_color(account_id, Some(color));
                *self = Self::showing(Some(color));
            }
            None => self.invalid = true,
        }
    }
}

/// Renders the editor for the account `account_id` named `account_name`. In streamer mode the
/// title shows the name highlighted in the color being edited. Returns `true` when the user is
/// done with it.
pub(super) fn render_account_color_editor(
    ui: &mut egui::Ui,
    text_ui: &mut TextUi,
    editor_id: egui::Id,
    account_id: &str,
    account_name: &str,
    streamer_mode: bool,
    button_style: &ButtonOptions,
) -> bool {
    let current = account_colors::color_for(account_id);
    let input_key = editor_id.with(("custom_color_input", account_id));
    let mut input = ui
        .data_mut(|data| data.get_temp::<CustomColorInput>(input_key))
        .unwrap_or_else(|| CustomColorInput::showing(current));
    let mut done = false;

    egui::Frame::new()
        .fill(ui.visuals().faint_bg_color)
        .stroke(egui::Stroke::new(
            1.0,
            ui.visuals().widgets.noninteractive.bg_stroke.color,
        ))
        .corner_radius(egui::CornerRadius::same(style::CORNER_RADIUS_SM))
        .inner_margin(egui::Margin::same(style::SPACE_MD as i8))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(style::SPACE_XS, style::SPACE_SM);
            let _ = account_label(
                ui,
                text_ui,
                ("account_color_title", account_id),
                &format!("Color for {}", mention(account_id, account_name)),
                &style::stat_label(ui),
                streamer_mode,
                false,
            );

            for (caption, colors) in [
                ("Primary", &account_colors::PRIMARY_COLORS[..]),
                ("Secondary", &account_colors::SECONDARY_COLORS[..]),
                ("Tertiary", &account_colors::TERTIARY_COLORS[..]),
            ] {
                render_palette_row(
                    ui, text_ui, account_id, caption, colors, current, &mut input,
                );
            }

            render_custom_row(ui, text_ui, account_id, current, &mut input, button_style);
            if input.invalid {
                let _ = textui_egui::without_name_scrubbing(|| {
                    text_ui.label(
                        ui,
                        ("account_color_invalid", account_id),
                        INVALID_COLOR_HINT,
                        &style::muted_single_line(ui),
                    )
                });
            }

            ui.horizontal(|ui| {
                let mut action_style = button_style.clone();
                action_style.min_size = egui::vec2(96.0, style::CONTROL_HEIGHT);
                if current.is_some()
                    && textui_egui::without_name_scrubbing(|| {
                        text_ui.button(
                            ui,
                            ("account_color_clear", account_id),
                            "Clear color",
                            &action_style,
                        )
                    })
                    .clicked()
                {
                    account_colors::set_color(account_id, None);
                    input = CustomColorInput::default();
                }
                if textui_egui::without_name_scrubbing(|| {
                    text_ui.button(
                        ui,
                        ("account_color_done", account_id),
                        "Done",
                        &action_style,
                    )
                })
                .clicked()
                {
                    done = true;
                }
            });
        });

    if done {
        ui.data_mut(|data| data.remove::<CustomColorInput>(input_key));
    } else {
        ui.data_mut(|data| data.insert_temp(input_key, input));
    }
    done
}

fn render_palette_row(
    ui: &mut egui::Ui,
    text_ui: &mut TextUi,
    account_id: &str,
    caption: &str,
    colors: &[PaletteColor],
    current: Option<Color32>,
    input: &mut CustomColorInput,
) {
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(
            egui::vec2(CAPTION_WIDTH, SWATCH_SIZE),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                let _ = textui_egui::without_name_scrubbing(|| {
                    text_ui.label(
                        ui,
                        ("account_color_caption", caption, account_id),
                        caption,
                        &style::muted_single_line(ui),
                    )
                });
            },
        );
        for entry in colors {
            let response = color_swatch_button(
                ui,
                ("account_color_swatch", account_id, entry.name),
                Some(entry.color),
                SWATCH_SIZE,
                current == Some(entry.color),
            );
            let response = style::hover_tip(ui, response, entry.name);
            if response.clicked() {
                account_colors::set_color(account_id, Some(entry.color));
                *input = CustomColorInput::showing(Some(entry.color));
            }
        }
    });
}

fn render_custom_row(
    ui: &mut egui::Ui,
    text_ui: &mut TextUi,
    account_id: &str,
    current: Option<Color32>,
    input: &mut CustomColorInput,
    button_style: &ButtonOptions,
) {
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(
            egui::vec2(CAPTION_WIDTH, SWATCH_SIZE),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                let _ = textui_egui::without_name_scrubbing(|| {
                    text_ui.label(
                        ui,
                        ("account_color_caption_custom", account_id),
                        "Custom",
                        &style::muted_single_line(ui),
                    )
                });
            },
        );
        // Shows the current color, with the dot, when it is not one of the palette colors.
        let custom_color = current.filter(|color| !account_colors::is_palette_color(*color));
        color_preview_box(ui, custom_color, SWATCH_SIZE, custom_color.is_some());

        let mut input_options = text_input_theme::themed_text_input_options(ui, false);
        input_options.desired_rows = 1;
        input_options.desired_width = Some(CUSTOM_INPUT_WIDTH);
        input_options.min_width = CUSTOM_INPUT_WIDTH;
        input_options.padding = egui::vec2(6.0, 4.0);
        let field = text_ui.singleline_input(
            ui,
            ("account_color_custom_input", account_id),
            &mut input.text,
            &input_options,
        );
        if field.changed() {
            input.invalid = false;
        }
        let enter_pressed = field.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));

        let mut set_style = button_style.clone();
        set_style.min_size = egui::vec2(48.0, style::CONTROL_HEIGHT);
        let set_clicked = textui_egui::without_name_scrubbing(|| {
            text_ui.button(ui, ("account_color_set", account_id), "Set", &set_style)
        })
        .clicked();
        if enter_pressed || set_clicked {
            input.apply(account_id);
        }
    });
}
