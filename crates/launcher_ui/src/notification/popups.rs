use super::*;

use egui::{self, Color32, CornerRadius, Frame, Layout, Margin, Stroke};
use textui::TextUi;
use textui_egui::interaction;
use textui_egui::prelude::*;

use crate::ui::style;

pub fn render_popups(
    ctx: &egui::Context,
    text_ui: &mut TextUi,
    settings: &NotificationDisplaySettings,
    suppressed_progress_source: Option<&str>,
) {
    drain_notifications(settings.fade_out);

    let entries = {
        let Ok(store) = center().store.lock() else {
            return;
        };
        if store.entries.is_empty() {
            return;
        }
        store
            .entries
            .iter()
            .filter(|entry| {
                suppressed_progress_source.is_none_or(|source| {
                    let suppressible = entry.progress.is_some() || entry.spinner;
                    !suppressible
                        || (entry.source != source && entry.replace_key.as_deref() != Some(source))
                })
            })
            .cloned()
            .collect::<Vec<_>>()
    };
    if entries.is_empty() {
        return;
    }

    egui::Area::new(egui::Id::new("notification_stack_area"))
        .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-14.0, 48.0))
        .interactable(true)
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            ui.set_width(360.0);
            ui.spacing_mut().item_spacing = egui::vec2(0.0, 8.0);
            let now = Instant::now();
            let mut dismissed_ids = Vec::new();
            let mut triggered_actions: Vec<Arc<dyn Fn(&egui::Context) + Send + Sync>> = Vec::new();

            for (index, entry) in entries.iter().enumerate() {
                let base = ui.visuals().window_fill;
                let frame = Frame::new()
                    .fill(Color32::from_rgba_premultiplied(
                        base.r(),
                        base.g(),
                        base.b(),
                        255,
                    ))
                    .stroke(Stroke::new(1.0, ui.visuals().window_stroke.color))
                    .corner_radius(CornerRadius::same(10))
                    .inner_margin(Margin::same(10));

                let previous_opacity = ui.opacity();
                ui.set_opacity(previous_opacity * entry_fade_opacity(entry, now, settings));
                frame.show(ui, |ui| {
                    if entry.progress.is_none() && !entry.spinner {
                        let elapsed = now.saturating_duration_since(entry.last_seen);
                        let ttl = entry_ttl(entry);
                        let remaining = ttl.saturating_sub(elapsed);
                        let expiry_progress =
                            (remaining.as_secs_f32() / ttl.as_secs_f32()).clamp(0.0, 1.0);
                        let expiry_fill = severity_accent_fill(ui, entry.severity);
                        let bar_width = ui.available_width().max(1.0);
                        let (bar_rect, _) = ui.allocate_exact_size(
                            egui::vec2(bar_width, NOTIFICATION_EXPIRY_BAR_HEIGHT),
                            egui::Sense::hover(),
                        );
                        ui.painter().rect_filled(
                            bar_rect,
                            CornerRadius::same(2),
                            ui.visuals().widgets.inactive.bg_fill,
                        );
                        if expiry_progress > 0.0 {
                            let fill_width = bar_rect.width() * expiry_progress;
                            let filled_rect = if settings.expiry_bars_empty_left {
                                egui::Rect::from_min_max(
                                    egui::pos2(bar_rect.max.x - fill_width, bar_rect.min.y),
                                    bar_rect.max,
                                )
                            } else {
                                egui::Rect::from_min_max(
                                    bar_rect.min,
                                    egui::pos2(bar_rect.min.x + fill_width, bar_rect.max.y),
                                )
                            };
                            ui.painter().rect_filled(
                                filled_rect,
                                CornerRadius::same(2),
                                expiry_fill,
                            );
                        }
                        ui.add_space(8.0);
                    }

                    ui.with_layout(Layout::right_to_left(egui::Align::TOP), |ui| {
                        let close_button = notification_icon_button(
                            ui,
                            &format!("notif-close-{}", entry.id),
                            assets::X_SVG,
                            "Dismiss notification",
                        );
                        if close_button.clicked() {
                            dismissed_ids.push(entry.id);
                        }
                        ui.add_space(6.0);
                        ui.allocate_ui_with_layout(
                            egui::vec2(ui.available_width().max(0.0), 0.0),
                            Layout::left_to_right(egui::Align::TOP),
                            |ui| {
                                let icon_size = 16.0;
                                let icon = crate::ui::svg_tint::themed_svg_image(
                                    "notification",
                                    &format!(
                                        "notif-icon-{}-{}",
                                        index,
                                        severity_name(entry.severity)
                                    ),
                                    entry
                                        .icon
                                        .unwrap_or_else(|| icon_for_severity(entry.severity)),
                                    ui.visuals().text_color(),
                                    icon_size,
                                );
                                let icon_rect = egui::Rect::from_min_size(
                                    ui.cursor().min + egui::vec2(0.0, 2.0),
                                    egui::vec2(icon_size, icon_size),
                                );
                                ui.put(icon_rect, icon);
                                ui.add_space(6.0);

                                if entry.count > 1 {
                                    let count_style = style::caption(ui);
                                    let _ = text_ui.label(
                                        ui,
                                        ("notif-count", index),
                                        &format!("x{}", entry.count),
                                        &count_style,
                                    );
                                    ui.add_space(6.0);
                                }

                                let mut source_style = style::stat_label(ui);
                                source_style.wrap = true;
                                let _ = text_ui.label(
                                    ui,
                                    ("notif-source", index),
                                    &format!(
                                        "{} · {}",
                                        severity_name(entry.severity),
                                        entry.source
                                    ),
                                    &source_style,
                                );
                            },
                        );
                    });

                    ui.add_space(4.0);
                    let message_style = LabelOptions {
                        color: ui.visuals().text_color(),
                        wrap: true,
                        ..style::caption(ui)
                    };
                    if let Some(progress) = entry.progress {
                        ui.add_space(6.0);
                        let overlay = format!("{}  {:.0}%", entry.message, progress * 100.0);
                        ui.add(
                            egui::ProgressBar::new(progress).desired_width(ui.available_width()),
                        );
                        let _ = text_ui.label(
                            ui,
                            ("notif-progress-message", index),
                            overlay.as_str(),
                            &message_style,
                        );
                    } else if entry.spinner {
                        ui.horizontal_wrapped(|ui| {
                            ui.spinner();
                            ui.add_space(6.0);
                            let _ = text_ui.label(
                                ui,
                                ("notif-spinner-message", index),
                                entry.message.as_str(),
                                &message_style,
                            );
                        });
                    } else {
                        let _ = text_ui.label(
                            ui,
                            ("notif-message", index),
                            entry.message.as_str(),
                            &message_style,
                        );
                    }

                    if !entry.actions.is_empty() {
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            for (action_index, action) in entry.actions.iter().enumerate() {
                                let clicked = text_ui
                                    .button(
                                        ui,
                                        ("notif-action", entry.id, action_index),
                                        action.label.as_str(),
                                        &ui_foundation::secondary_button(
                                            ui,
                                            egui::vec2(72.0, style::CONTROL_HEIGHT),
                                        ),
                                    )
                                    .clicked();
                                if clicked {
                                    triggered_actions.push(action.callback.clone());
                                    dismissed_ids.push(entry.id);
                                }
                            }
                        });
                    }
                });
                ui.set_opacity(previous_opacity);
            }

            for callback in triggered_actions {
                callback(ctx);
            }
            if !dismissed_ids.is_empty()
                && let Ok(mut store) = center().store.lock()
            {
                store
                    .entries
                    .retain(|entry| !dismissed_ids.contains(&entry.id));
            }
        });

    ctx.request_repaint();
}

fn severity_accent_fill(ui: &egui::Ui, severity: Severity) -> Color32 {
    match severity {
        Severity::Log | Severity::Info => ui.visuals().selection.bg_fill,
        Severity::Warn => ui.visuals().warn_fg_color,
        Severity::Error => ui.visuals().error_fg_color,
    }
}

fn notification_icon_button(
    ui: &mut egui::Ui,
    icon_id: &str,
    svg_bytes: &[u8],
    tooltip: &str,
) -> egui::Response {
    let button_size = egui::vec2(22.0, 22.0);
    let icon_size = 12.0;
    let (rect, response) = ui.allocate_exact_size(button_size, egui::Sense::click());
    let state = interaction::InteractionState::of(ui.ctx(), &response, true);
    let fill = interaction::FillPalette::from_visuals(ui.visuals()).fill(state, false);
    let stroke = interaction::hover_stroke(
        state,
        ui.visuals().widgets.noninteractive.bg_stroke,
        ui.visuals().widgets.hovered.bg_stroke,
    );
    ui.painter().rect_filled(rect, CornerRadius::same(7), fill);
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(7),
        stroke,
        egui::StrokeKind::Inside,
    );
    interaction::paint_focus_ring(ui.painter(), ui.visuals(), state, rect, 7);

    let image = crate::ui::svg_tint::themed_svg_image(
        "notification",
        icon_id,
        svg_bytes,
        ui.visuals().text_color(),
        icon_size,
    );
    let icon_rect = egui::Rect::from_center_size(rect.center(), egui::vec2(icon_size, icon_size));
    let _ = ui.put(icon_rect, image);

    crate::ui::style::hover_tip(ui, response, tooltip)
}
