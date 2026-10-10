use super::*;

impl TextUi {
    #[doc(hidden)]
    pub fn egui_singleline_input(
        &mut self,
        ui: &mut Ui,
        id_source: impl Hash,
        text: &mut String,
        options: &InputOptions,
    ) -> Response {
        self.input_widget(ui, id_source, text, options, false)
    }

    #[doc(hidden)]
    pub fn egui_multiline_input(
        &mut self,
        ui: &mut Ui,
        id_source: impl Hash,
        text: &mut String,
        options: &InputOptions,
    ) -> Response {
        self.input_widget(ui, id_source, text, options, true)
    }

    #[doc(hidden)]
    pub fn egui_multiline_rich_viewer(
        &mut self,
        ui: &mut Ui,
        id_source: impl Hash,
        spans: &[RichTextSpan],
        options: &InputOptions,
        stick_to_bottom: bool,
        wrap: bool,
    ) -> Response {
        let id = ui.make_persistent_id(id_source).with("textui_rich_viewer");
        let width = options
            .desired_width
            .unwrap_or_else(|| ui.available_width())
            .max(options.min_width);
        let min_height = options.line_height + (options.padding.y * 2.0);
        let height = (options.line_height * options.desired_rows.max(1) as f32
            + options.padding.y * 2.0)
            .max(min_height);

        let desired_size = egui::vec2(width, height);
        let rect = ui.allocate_space(desired_size).1;
        let mut response = ui.interact(rect, id, Sense::click_and_drag());

        let has_focus = response.has_focus();
        if has_focus {
            lock_input_focus_filter(ui, id, true);
        }
        let scale = ui.ctx().pixels_per_point();
        let content_rect = rect.shrink2(options.padding);
        let content_width_px = (content_rect.width() * scale).max(1.0);
        let content_height_px = (content_rect.height() * scale).max(1.0);
        let text = spans
            .iter()
            .map(|span| span.text.as_str())
            .collect::<String>();
        let attrs_fingerprint = self.rich_viewer_attrs_fingerprint(spans, options, scale, wrap);

        let mut state = self
            .input_states
            .remove(&id)
            .unwrap_or_else(|| Self::new_input_state(&mut self.font_system, &text, true));

        let needs_text_sync =
            state.last_text != text || state.attrs_fingerprint != attrs_fingerprint;
        if needs_text_sync {
            state.scroll_metrics = self.replace_editor_rich_text(
                &mut state.editor,
                spans,
                options,
                content_width_px,
                content_height_px,
                scale,
                wrap,
            );
            state.last_text = text.clone();
            state.attrs_fingerprint = attrs_fingerprint;
            state.preferred_cursor_x_px = None;
            if stick_to_bottom && !has_focus && !response.hovered() {
                scroll_editor_to_buffer_end(&mut self.font_system, &mut state.editor);
            }
        } else {
            state.scroll_metrics = self.configure_viewer(
                &mut state.editor,
                options,
                content_width_px,
                content_height_px,
                scale,
                wrap,
            );
        }

        let pointer_pos = response.interact_pointer_pos();
        let scrollbar_tracks = viewer_scrollbar_track_rects(
            ui.style().spacing.scroll,
            response.hovered(),
            response.is_pointer_button_down_on(),
            content_rect,
            state.scroll_metrics,
        );
        let pointer_over_scrollbar = pointer_pos.is_some_and(|pos| scrollbar_tracks.contains(pos));
        let pointer_over_text = pointer_pos.is_some_and(|pos| {
            viewer_visible_text_rect(content_rect, state.scroll_metrics)
                .is_some_and(|text_rect| text_rect.contains(pos))
        }) && !pointer_over_scrollbar;
        let pointer_pressed_on_widget =
            ui.ctx().input(|i| i.pointer.primary_pressed()) && response.is_pointer_button_down_on();

        if (response.clicked() || pointer_pressed_on_widget) && !pointer_over_scrollbar {
            response.request_focus();
        }

        if pointer_over_text {
            ui.output_mut(|o| {
                o.cursor_icon = egui::CursorIcon::Text;
                o.mutable_text_under_cursor = true;
            });
        }

        let pointer_interacted = !pointer_over_scrollbar
            && (pointer_pressed_on_widget
                || response.clicked()
                || response.double_clicked()
                || response.triple_clicked()
                || response.drag_started()
                || response.dragged());

        let mut state_changed = if has_focus || response.hovered() || pointer_interacted {
            self.handle_viewer_events(
                ui,
                &response,
                &mut state.editor,
                content_rect,
                scale,
                &mut state.preferred_cursor_x_px,
                &options.fundamentals,
                has_focus,
                pointer_over_scrollbar,
                &mut state.scroll_metrics,
            )
        } else {
            false
        };

        paint_input_frame(ui, rect, options, has_focus, response.hovered());

        {
            let painter = ui
                .painter()
                .with_clip_rect(content_rect.intersect(ui.clip_rect()));
            self.paint_editor_gpu(
                &painter,
                content_rect,
                &state.editor,
                options,
                scale,
                false,
                true,
            );
        }

        state_changed |= self.sync_viewer_scrollbars(
            ui,
            id,
            &mut state.editor,
            content_rect,
            scale,
            &options.fundamentals,
            &mut state.scroll_metrics,
        );

        self.input_states.insert(id, state);
        if state_changed {
            response.mark_changed();
        }
        apply_gamepad_scroll_if_focused(ui, &response);

        response
    }

    pub(crate) fn input_widget(
        &mut self,
        ui: &mut Ui,
        id_source: impl Hash,
        text: &mut String,
        options: &InputOptions,
        multiline: bool,
    ) -> Response {
        let id = ui.make_persistent_id(id_source).with("textui_input");
        let width = options
            .desired_width
            .unwrap_or_else(|| ui.available_width())
            .max(options.min_width);

        let min_height = options.line_height + (options.padding.y * 2.0);
        let height = if multiline {
            (options.line_height * options.desired_rows.max(2) as f32 + options.padding.y * 2.0)
                .max(min_height)
        } else {
            min_height
        };

        let desired_size = egui::vec2(width, height);
        let rect = ui.allocate_space(desired_size).1;
        let mut response = ui.interact(rect, id, Sense::click_and_drag());

        if response.hovered() {
            ui.output_mut(|o| {
                o.cursor_icon = egui::CursorIcon::Text;
                o.mutable_text_under_cursor = true;
            });
        }

        if response.clicked() {
            response.request_focus();
        }

        let has_focus = response.has_focus();
        if has_focus {
            lock_input_focus_filter(ui, id, multiline);
        }
        let scale = ui.ctx().pixels_per_point();
        let content_rect = rect.shrink2(options.padding);
        let content_width_px = (content_rect.width() * scale).max(1.0);
        let content_height_px = (content_rect.height() * scale).max(1.0);
        let attrs_fingerprint = self.input_attrs_fingerprint(options, scale);

        let mut state = self
            .input_states
            .remove(&id)
            .unwrap_or_else(|| Self::new_input_state(&mut self.font_system, text, multiline));

        if state.multiline != multiline {
            state = Self::new_input_state(&mut self.font_system, text, multiline);
        }

        let needs_text_sync = !has_focus && state.last_text != *text;
        let needs_attrs_sync = state.attrs_fingerprint != attrs_fingerprint;
        if needs_text_sync || needs_attrs_sync {
            state.scroll_metrics = self.replace_editor_text(
                &mut state.editor,
                text,
                options,
                multiline,
                content_width_px,
                content_height_px,
                scale,
            );
            state.last_text.clone_from(text);
            state.attrs_fingerprint = attrs_fingerprint;
            state.preferred_cursor_x_px = None;
        }

        state.scroll_metrics = self.configure_editor(
            &mut state.editor,
            options,
            multiline,
            content_width_px,
            content_height_px,
            scale,
        );

        let pointer_interacted = response.clicked()
            || response.double_clicked()
            || response.triple_clicked()
            || response.dragged();

        let mut changed = false;
        if has_focus || pointer_interacted {
            let (undo_pressed, redo_pressed) = if has_focus {
                ui.input(|i| {
                    let undo = i.key_pressed(Key::Z) && i.modifiers.command && !i.modifiers.shift;
                    let redo = (i.key_pressed(Key::Y) && i.modifiers.command)
                        || (i.key_pressed(Key::Z) && i.modifiers.command && i.modifiers.shift);
                    (undo, redo)
                })
            } else {
                (false, false)
            };

            if undo_pressed {
                changed |= self.apply_undo_step(
                    &mut state,
                    UndoDirection::Undo,
                    options,
                    multiline,
                    [content_width_px, content_height_px],
                    scale,
                );
            } else if redo_pressed {
                changed |= self.apply_undo_step(
                    &mut state,
                    UndoDirection::Redo,
                    options,
                    multiline,
                    [content_width_px, content_height_px],
                    scale,
                );
            } else {
                if has_focus {
                    let pending_op = pending_modify_op(&self.frame_events);
                    if pending_op != UndoOpKind::None {
                        let should_push = matches!(pending_op, UndoOpKind::Paste | UndoOpKind::Cut)
                            || state.last_undo_op != pending_op;
                        if should_push {
                            push_undo(
                                &mut state.undo_stack,
                                UndoEntry {
                                    text: editor_to_string(&state.editor),
                                    cursor: state.editor.cursor(),
                                    selection: state.editor.selection(),
                                },
                            );
                            state.redo_stack.clear();
                        }
                        state.last_undo_op = pending_op;
                    } else if self.frame_events.iter().any(is_navigation_event) {
                        state.last_undo_op = UndoOpKind::None;
                    }
                }

                changed |= self.handle_input_events(
                    ui,
                    &response,
                    &mut state.editor,
                    multiline,
                    content_rect,
                    scale,
                    &mut state.preferred_cursor_x_px,
                    &options.fundamentals,
                    has_focus,
                    &mut state.scroll_metrics,
                );
            }

            if !multiline && ui.input(|i| i.key_pressed(Key::Enter)) {
                response.surrender_focus();
            }
        }

        let context_actions = self.input_context_menu(&response, &state.editor);
        changed |= apply_input_context_actions(ui.ctx(), &mut state, context_actions, multiline);

        let latest_text = editor_to_string(&state.editor);
        if latest_text != *text {
            *text = latest_text.clone();
            state.last_text = latest_text;
            state.preferred_cursor_x_px = None;
            changed = true;
        }

        if changed {
            response.mark_changed();
        }

        state.last_used_frame = self.current_frame;

        paint_input_frame(ui, rect, options, has_focus, response.hovered());

        {
            let painter = ui
                .painter()
                .with_clip_rect(content_rect.intersect(ui.clip_rect()));
            self.paint_editor_gpu(
                &painter,
                content_rect,
                &state.editor,
                options,
                scale,
                has_focus,
                false,
            );
        }
        self.input_states.insert(id, state);
        if !has_focus
            && text.is_empty()
            && let Some(placeholder_text) = options
                .placeholder_text
                .as_deref()
                .filter(|placeholder| !placeholder.is_empty())
        {
            self.paint_input_placeholder(
                ui,
                id,
                placeholder_text,
                options,
                multiline,
                content_rect,
            );
        }

        apply_gamepad_scroll_if_focused(ui, &response);

        response
    }
}
