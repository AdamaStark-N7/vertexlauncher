use super::*;
use textui_egui::interaction;
use textui_egui::prelude::*;

use crate::assets;

pub(super) fn render_device_code_section(
    ui: &mut egui::Ui,
    text_ui: &mut TextUi,
    prompt: &auth::DeviceCodePrompt,
    output: &mut TopBarOutput,
    full_action_width: f32,
    button_style: &ButtonOptions,
) {
    let qr_url = prompt.verification_url();

    let instruction_text = "Scan this QR code to continue with device code on another device, or use browser sign-in on this device to avoid typing the code manually.";

    // Instructional text — wraps naturally, normal size
    let instruction_style = style::body(ui);
    let _ = text_ui.label(
        ui,
        "device_code_instruction",
        instruction_text,
        &instruction_style,
    );

    // QR code: pre-allocate exact rect so nothing can expand outward
    let qr_size = (full_action_width * 0.7).clamp(120.0, 220.0);
    ui.allocate_ui_with_layout(
        egui::vec2(full_action_width, qr_size),
        Layout::top_down(Align::Center),
        |ui| {
            render_qr_code(ui, &qr_url, qr_size);
        },
    );

    // Code row: build style then measure actual rendered size to drive layout
    let code_label_style = style::subtitle(ui);
    let code_text_size =
        text_ui.measure_text_size(ui, prompt.user_code.as_str(), &code_label_style);
    let code_row_height = code_text_size.y + style::SPACE_MD * 2.0;
    let code_box_width = code_text_size.x + style::SPACE_MD * 2.0;
    let copy_btn_size = code_row_height; // square button matching row height
    let total_row_width = code_box_width + copy_btn_size;

    let cr = style::CORNER_RADIUS_SM;
    let box_radius = egui::CornerRadius {
        nw: cr,
        sw: cr,
        ne: 0,
        se: 0,
    };
    let btn_radius = egui::CornerRadius {
        nw: 0,
        sw: 0,
        ne: cr,
        se: cr,
    };

    // Outer container centers the joined control horizontally
    ui.allocate_ui_with_layout(
        egui::vec2(full_action_width, code_row_height),
        Layout::top_down(Align::Center),
        |ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(total_row_width, code_row_height),
                Layout::right_to_left(Align::Center),
                |ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;

                    render_copy_code_button(
                        ui,
                        &prompt.user_code,
                        copy_btn_size,
                        code_row_height,
                        btn_radius,
                    );

                    // Code display box — sized to fit text, left-rounded right-flat
                    let (box_rect, _) = ui.allocate_exact_size(
                        egui::vec2(code_box_width, code_row_height),
                        egui::Sense::hover(),
                    );
                    ui.painter().rect(
                        box_rect,
                        box_radius,
                        ui.visuals().widgets.noninteractive.bg_fill,
                        egui::Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color),
                        egui::StrokeKind::Inside,
                    );
                    let inner = box_rect.shrink2(egui::vec2(style::SPACE_MD, 0.0));
                    ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| {
                        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                            let _ = text_ui.label(
                                ui,
                                "device_code_user_code",
                                prompt.user_code.as_str(),
                                &code_label_style,
                            );
                        });
                    });
                },
            );
        },
    );

    // Bottom row: Cancel (left half) + Open Browser (right half)
    let half = (full_action_width - style::SPACE_MD) / 2.0;
    ui.allocate_ui_with_layout(
        egui::vec2(full_action_width, style::CONTROL_HEIGHT),
        Layout::left_to_right(Align::Center),
        |ui| {
            ui.spacing_mut().item_spacing.x = style::SPACE_MD;
            let mut half_style = button_style.clone();
            half_style.min_size = egui::vec2(half, style::CONTROL_HEIGHT);
            if text_ui
                .button(ui, "device_code_cancel", "Cancel", &half_style)
                .clicked()
            {
                output.cancel_device_code_sign_in = true;
            }
            let open_browser_label = "Use Browser Sign-in";
            if text_ui
                .button(
                    ui,
                    "device_code_open_browser",
                    open_browser_label,
                    &half_style,
                )
                .clicked()
            {
                output.open_device_code_browser = true;
            }
        },
    );
}

/// Square, right-rounded copy button that puts the device code on the clipboard.
fn render_copy_code_button(
    ui: &mut egui::Ui,
    user_code: &str,
    copy_btn_size: f32,
    code_row_height: f32,
    btn_radius: egui::CornerRadius,
) {
    let text_color = ui.visuals().text_color();
    let icon_size = (copy_btn_size - style::SPACE_MD * 2.0).clamp(12.0, 28.0);
    let icon = crate::ui::svg_tint::themed_svg_image(
        "topbar",
        "device-code-copy",
        assets::COPY_SVG,
        text_color,
        icon_size,
    );
    let (btn_rect, btn_response) = ui.allocate_exact_size(
        egui::vec2(copy_btn_size, code_row_height),
        egui::Sense::click(),
    );
    let state = interaction::InteractionState::of(ui.ctx(), &btn_response, true);
    let btn_fill = interaction::FillPalette::new(
        ui.visuals().widgets.noninteractive.bg_fill,
        ui.visuals().widgets.hovered.bg_fill,
        ui.visuals().widgets.active.bg_fill,
        ui.visuals().selection.bg_fill,
        ui.visuals().text_color(),
    )
    .fill(state, false);
    let stroke = interaction::hover_stroke(
        state,
        egui::Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color),
        ui.visuals().widgets.hovered.bg_stroke,
    );
    ui.painter().rect(
        btn_rect,
        btn_radius,
        btn_fill,
        stroke,
        egui::StrokeKind::Inside,
    );
    // Paint icon centered in the button rect — paint_at bypasses the cursor
    let icon_rect =
        egui::Rect::from_center_size(btn_rect.center(), egui::vec2(icon_size, icon_size));
    icon.paint_at(ui, icon_rect);
    if style::hover_tip(ui, btn_response, "Copy code").clicked() {
        ui.ctx().copy_text(user_code.to_owned());
    }
}
