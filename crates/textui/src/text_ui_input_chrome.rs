use super::*;

#[path = "text_ui_input_context_actions.rs"]
mod context_actions;

pub(super) use self::context_actions::InputContextActions;

fn input_frame_fill(options: &InputOptions, has_focus: bool, hovered: bool) -> Color32 {
    if has_focus {
        options
            .background_color_focused
            .or(options.background_color_hovered)
            .unwrap_or(options.background_color)
    } else if hovered {
        options
            .background_color_hovered
            .unwrap_or(options.background_color)
    } else {
        options.background_color
    }
}

fn input_frame_stroke(options: &InputOptions, has_focus: bool, hovered: bool) -> egui::Stroke {
    if has_focus {
        options
            .stroke_focused
            .or(options.stroke_hovered)
            .unwrap_or(options.stroke)
    } else if hovered {
        options.stroke_hovered.unwrap_or(options.stroke)
    } else {
        options.stroke
    }
}

/// Paints the background fill and border of an input/viewer widget.
pub(super) fn paint_input_frame(
    ui: &Ui,
    rect: Rect,
    options: &InputOptions,
    has_focus: bool,
    hovered: bool,
) {
    let frame_fill = input_frame_fill(options, has_focus, hovered);
    let frame_stroke = input_frame_stroke(options, has_focus, hovered);
    let corner_radius = CornerRadius::same(options.corner_radius);

    ui.painter().rect_filled(rect, corner_radius, frame_fill);
    ui.painter()
        .rect_stroke(rect, corner_radius, frame_stroke, egui::StrokeKind::Inside);
}

/// Keeps arrow keys (and optionally tab) routed to the focused input widget.
pub(super) fn lock_input_focus_filter(ui: &mut Ui, id: Id, tab: bool) {
    ui.memory_mut(|m| {
        m.set_focus_lock_filter(
            id,
            egui::EventFilter {
                horizontal_arrows: true,
                vertical_arrows: true,
                tab,
                escape: false,
            },
        );
    });
}

/// Applies context-menu actions to the input state; returns true when the text changed.
pub(super) fn apply_input_context_actions(
    ctx: &Context,
    state: &mut InputState,
    actions: InputContextActions,
    multiline: bool,
) -> bool {
    let mut changed = false;
    if actions.cut {
        if let Some(sel) = state.editor.copy_selection() {
            push_undo(&mut state.undo_stack, UndoEntry::capture(&state.editor));
            state.redo_stack.clear();
            state.last_undo_op = UndoOpKind::None;
            copy_sanitized(ctx, sel);
            state.editor.delete_selection();
            state.preferred_cursor_x_px = None;
            changed = true;
        }
    }
    if actions.copy {
        if let Some(sel) = state.editor.copy_selection() {
            copy_sanitized(ctx, sel);
        }
    }
    if actions.paste {
        if let Ok(mut cb) = arboard::Clipboard::new() {
            if let Ok(paste_text) = cb.get_text() {
                let paste_text = if multiline {
                    paste_text
                } else {
                    paste_text.replace(['\n', '\r'], " ")
                };
                if !paste_text.is_empty() {
                    push_undo(&mut state.undo_stack, UndoEntry::capture(&state.editor));
                    state.redo_stack.clear();
                    state.last_undo_op = UndoOpKind::None;
                    state.editor.insert_string(&paste_text, None);
                    state.preferred_cursor_x_px = None;
                    changed = true;
                }
            }
        }
    }
    if actions.select_all {
        state.preferred_cursor_x_px = None;
        changed |= select_all(&mut state.editor);
    }
    changed
}

impl TextUi {
    /// Shows the right-click menu for an input and reports which entries were chosen.
    pub(super) fn input_context_menu(
        &mut self,
        response: &Response,
        editor: &Editor<'static>,
    ) -> InputContextActions {
        let mut actions = InputContextActions::default();
        response.context_menu(|menu| {
            let has_selection = editor.selection() != Selection::None;
            let button_options = ButtonOptions {
                font_size: 14.0,
                line_height: 18.0,
                text_color: menu.visuals().text_color(),
                fill: Color32::TRANSPARENT,
                fill_hovered: menu.visuals().widgets.hovered.bg_fill,
                fill_active: menu.visuals().widgets.active.bg_fill,
                fill_selected: Color32::TRANSPARENT,
                stroke: egui::Stroke::NONE,
                corner_radius: 4,
                padding: egui::vec2(8.0, 4.0),
                min_size: egui::vec2(menu.available_width().max(96.0), 26.0),
            };
            if menu
                .add_enabled_ui(has_selection, |menu| {
                    self.button(menu, "textui_input_context_cut", "Cut", &button_options)
                })
                .inner
                .clicked()
            {
                actions.cut = true;
                menu.close();
            }
            if menu
                .add_enabled_ui(has_selection, |menu| {
                    self.button(menu, "textui_input_context_copy", "Copy", &button_options)
                })
                .inner
                .clicked()
            {
                actions.copy = true;
                menu.close();
            }
            if self
                .button(menu, "textui_input_context_paste", "Paste", &button_options)
                .clicked()
            {
                actions.paste = true;
                menu.close();
            }
            menu.separator();
            if self
                .button(
                    menu,
                    "textui_input_context_select_all",
                    "Select All",
                    &button_options,
                )
                .clicked()
            {
                actions.select_all = true;
                menu.close();
            }
        });
        actions
    }

    /// Paints the placeholder label of an empty, unfocused input.
    pub(super) fn paint_input_placeholder(
        &mut self,
        ui: &Ui,
        id: Id,
        placeholder_text: &str,
        options: &InputOptions,
        multiline: bool,
        content_rect: Rect,
    ) {
        let placeholder_style = LabelOptions {
            font_size: options.font_size,
            line_height: options.line_height,
            color: options
                .placeholder_color
                .unwrap_or_else(|| options.text_color.gamma_multiply(0.5)),
            wrap: multiline,
            monospace: options.monospace,
            weight: options.weight,
            fundamentals: options.fundamentals.clone(),
            ..LabelOptions::default()
        };
        let placeholder_scene = self.prepare_label_scene(
            ui.ctx(),
            id.with("placeholder"),
            placeholder_text,
            &placeholder_style,
            multiline.then_some(content_rect.width()),
        );
        let placeholder_size = egui_vec_from_text(placeholder_scene.size_points);
        let y_offset = if multiline {
            0.0
        } else {
            ((content_rect.height() - placeholder_size.y) * 0.5).max(0.0)
        };
        let placeholder_rect = Rect::from_min_size(
            Pos2::new(content_rect.min.x, content_rect.min.y + y_offset),
            placeholder_size.min(content_rect.size()),
        );
        let painter = ui
            .painter()
            .with_clip_rect(content_rect.intersect(ui.clip_rect()));
        self.paint_scene_in_rect(&painter, placeholder_rect, &placeholder_scene);
    }
}
