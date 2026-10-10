use super::*;

pub fn toggle_row(
    text_ui: &mut TextUi,
    ui: &mut Ui,
    label: &str,
    info_tooltip: Option<&str>,
    value: &mut bool,
) -> Response {
    let metrics = control_metrics(ui);
    let label_options = row_label_options(ui);
    let toggle_id = ui.make_persistent_id(("toggle_switch", label));
    let activate_target = gamepad_activate_target(ui.ctx());

    ui.horizontal(|ui| {
        let mut label_response =
            text_ui.clickable_label(ui, ("toggle_label", label), label, &label_options);
        register_gamepad_custom_activate_id(ui.ctx(), label_response.id);
        register_gamepad_custom_activate_id(ui.ctx(), toggle_id);
        if label_response.clicked() {
            *value = !*value;
            label_response.mark_changed();
        }

        if info_tooltip.is_some() {
            ui.add_space(6.0);
            info_hint(text_ui, ui, ("toggle_info", label), info_tooltip);
        }

        let switch_response = ui
            .with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.add_space(metrics.right_padding);
                switch(ui, value, metrics, toggle_id)
            })
            .inner;
        maybe_request_default_focus(ui.ctx(), &switch_response);
        apply_gamepad_scroll_if_focused(ui, &switch_response);

        if activate_target == Some(toggle_id)
            || activate_target.is_some_and(|id| id == label_response.id)
        {
            *value = !*value;
            label_response.mark_changed();
        }

        if label_response.has_focus() || switch_response.has_focus() {
            ui.painter().rect_stroke(
                ui.min_rect(),
                egui::CornerRadius::same(8),
                ui.visuals().selection.stroke,
                egui::StrokeKind::Inside,
            );
        }

        switch_response.union(label_response)
    })
    .inner
}

fn switch(ui: &mut Ui, value: &mut bool, metrics: ControlMetrics, id: egui::Id) -> Response {
    let desired_size = egui::vec2(metrics.switch_width, metrics.control_height);
    let rect = ui.allocate_exact_size(desired_size, Sense::hover()).0;
    let mut response = ui.interact(rect, id, Sense::click());

    if response.clicked() {
        *value = !*value;
        response.mark_changed();
    }

    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Checkbox, ui.is_enabled(), *value, "")
    });

    if ui.is_rect_visible(rect) {
        // Animate the knob with cubic ease-out + OKLab colour interpolation,
        // matching the style of the mod-enable toggle in the instance menu.
        let anim_id = egui::Id::new(("settings_switch_anim", id));
        let target = if *value { 1.0f32 } else { 0.0f32 };
        let dt = ui.ctx().input(|i| i.stable_dt).min(0.1);

        let mut anim: ToggleAnimState = ui
            .ctx()
            .data_mut(|d| d.get_temp::<ToggleAnimState>(anim_id))
            .unwrap_or_else(|| ToggleAnimState::settled(target));

        anim.redirect(target);
        anim.advance(dt);
        let pos = anim.visual_pos();

        if !anim.is_done() {
            ui.ctx().request_repaint();
        }
        ui.ctx().data_mut(|d| d.insert_temp(anim_id, anim));

        // Track
        let visuals = ui.visuals();
        let off_fill = visuals.widgets.inactive.bg_fill;
        let on_fill = visuals.selection.bg_fill;
        let switch_state = interaction::InteractionState::of(ui.ctx(), &response, ui.is_enabled());
        let fill = lerp_color32_oklab(off_fill, on_fill, pos)
            .lerp_to_gamma(visuals.text_color(), 0.14 * switch_state.hover);

        let bg_stroke = if response.has_focus() {
            visuals.selection.stroke
        } else {
            let off_color = visuals.widgets.inactive.bg_stroke.color;
            let on_color = visuals.selection.stroke.color;
            let stroke_color = lerp_color32_oklab(off_color, on_color, pos)
                .lerp_to_gamma(visuals.widgets.hovered.bg_stroke.color, switch_state.hover);
            let stroke_w = visuals.widgets.inactive.bg_stroke.width
                + (visuals.selection.stroke.width - visuals.widgets.inactive.bg_stroke.width) * pos;
            egui::Stroke::new(stroke_w, stroke_color)
        };

        let corner_radius = rect.height() / 2.0;
        ui.painter().rect(
            rect,
            corner_radius,
            fill,
            bg_stroke,
            egui::StrokeKind::Inside,
        );

        // Knob — same proportions as before, position driven by eased pos.
        let knob_margin = (metrics.control_height * 0.10).clamp(2.0, 4.0);
        let knob_radius = (rect.height() - (knob_margin * 2.0)) / 2.0;
        let knob_x = rect.left()
            + knob_margin
            + knob_radius
            + (rect.width() - (knob_margin + knob_radius) * 2.0) * pos;
        let knob_center = egui::pos2(knob_x, rect.center().y);
        let knob_off = visuals.widgets.noninteractive.fg_stroke.color;
        let knob_on = visuals.widgets.active.fg_stroke.color;
        let knob_fill = lerp_color32_oklab(knob_off, knob_on, pos);
        ui.painter().circle(
            knob_center,
            knob_radius,
            knob_fill,
            egui::Stroke::new(1.0, bg_stroke.color),
        );
    }

    response
}
