use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use auth::{CachedAccount, MinecraftProfileState, MinecraftSkinVariant};
use egui::{Color32, CornerRadius, Pos2, Rect, Sense, Stroke, TextureHandle, TextureOptions, Ui};
use image::RgbaImage;
use textui::{ButtonOptions, TextUi};

use super::LaunchAuthContext;
use crate::ui::style;

const PREVIEW_ORBIT_SECONDS: f64 = 45.0;
const PREVIEW_TARGET_FPS: f32 = 60.0;
const PREVIEW_HEIGHT: f32 = 460.0;
const CAMERA_DRAG_SENSITIVITY_RAD_PER_POINT: f32 = 0.0046;
const CAMERA_INERTIA_VELOCITY_BLEND: f32 = 0.24;
const CAMERA_INERTIA_MAX_RAD_PER_SEC: f32 = 2.2;
const CAMERA_INERTIA_FRICTION_PER_SEC: f32 = 0.85;
const CAMERA_INERTIA_STOP_THRESHOLD_RAD_PER_SEC: f32 = 0.015;
const UV_EDGE_INSET_TEXELS: f32 = 0.01;
const CAPE_TILE_WIDTH_MIN: f32 = 132.0;
const CAPE_TILE_HEIGHT: f32 = 186.0;

pub fn render(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    _selected_instance_id: Option<&str>,
    active_launch_auth: Option<&LaunchAuthContext>,
) {
    let state_id = ui.make_persistent_id("skins_screen_state");
    let mut state = ui
        .ctx()
        .data_mut(|data| data.get_temp::<SkinManagerState>(state_id))
        .unwrap_or_default();

    state.sync_active_account(active_launch_auth);
    state.poll_worker(ui.ctx());
    state.ensure_skin_texture(ui.ctx());
    state.ensure_cape_texture(ui.ctx());
    ui.ctx()
        .request_repaint_after(Duration::from_secs_f32(1.0 / PREVIEW_TARGET_FPS));

    egui::ScrollArea::vertical()
        .id_salt("skins_screen_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            render_contents(ui, text_ui, &mut state);
        });

    ui.ctx().data_mut(|data| data.insert_temp(state_id, state));
}

fn render_contents(ui: &mut Ui, text_ui: &mut TextUi, state: &mut SkinManagerState) {
    let heading = style::page_heading(ui);
    let body = style::body(ui);
    let muted = style::muted(ui);

    let _ = text_ui.label(ui, "skins_heading", "Skin Manager", &heading);
    ui.add_space(style::SPACE_SM);

    if let Some(name) = state.active_player_name.as_deref() {
        let _ = text_ui.label(
            ui,
            "skins_active_user",
            &format!("Active account: {name}"),
            &body,
        );
    } else {
        let _ = text_ui.label(
            ui,
            "skins_no_active_user",
            "Sign in with a Minecraft account to manage skins and capes.",
            &muted,
        );
        return;
    }

    ui.add_space(style::SPACE_MD);
    if render_preview(ui, text_ui, state) {
        state.show_elytra = !state.show_elytra;
    }
    ui.add_space(style::SPACE_MD);

    let button_style = neutral_button_style(ui);

    ui.horizontal(|ui| {
        if text_ui
            .button(
                ui,
                "skins_refresh_profile",
                "Refresh profile",
                &button_style,
            )
            .clicked()
        {
            state.start_refresh();
        }
    });

    ui.add_space(style::SPACE_MD);
    let _ = text_ui.label(
        ui,
        "skins_picker_heading",
        "Skin Image",
        &style::section_heading(ui),
    );

    if text_ui
        .button(ui, "skins_pick_file", "Choose skin image", &button_style)
        .clicked()
    {
        state.pick_skin_file();
    }

    if let Some(path) = state.pending_skin_path.as_deref() {
        ui.add_space(style::SPACE_XS);
        let _ = text_ui.label(ui, "skins_selected_path", path, &muted);
    }

    ui.add_space(style::SPACE_SM);
    let mut model_button_style = button_style.clone();
    model_button_style.min_size = egui::vec2(120.0, style::CONTROL_HEIGHT);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(style::SPACE_XS, style::SPACE_XS);
        let _ = text_ui.label(ui, "skins_model_label", "Model:", &body);
        if text_ui
            .selectable_button(
                ui,
                "skins_model_classic",
                "Classic",
                state.pending_variant == MinecraftSkinVariant::Classic,
                &model_button_style,
            )
            .clicked()
        {
            state.pending_variant = MinecraftSkinVariant::Classic;
        }
        if text_ui
            .selectable_button(
                ui,
                "skins_model_slim",
                "Slim (Alex)",
                state.pending_variant == MinecraftSkinVariant::Slim,
                &model_button_style,
            )
            .clicked()
        {
            state.pending_variant = MinecraftSkinVariant::Slim;
        }
    });

    ui.add_space(style::SPACE_MD);
    let _ = text_ui.label(
        ui,
        "skins_cape_heading",
        "Cape",
        &style::section_heading(ui),
    );
    ui.add_space(style::SPACE_XS);

    render_cape_grid(ui, text_ui, state);

    ui.add_space(style::SPACE_MD);
    if let Some(status) = state.status_message.as_deref() {
        let _ = text_ui.label(ui, "skins_status", status, &body);
    }
    if state.save_in_progress {
        let _ = text_ui.label(ui, "skins_saving", "Saving changes...", &muted);
    }

    ui.add_space(style::SPACE_MD);
    let mut save_style = button_style.clone();
    let viewport_width = ui.clip_rect().width().max(1.0);
    let save_width = ui.available_width().min(viewport_width).max(1.0);
    save_style.min_size = egui::vec2(save_width, style::CONTROL_HEIGHT_LG);
    save_style.fill = ui.visuals().selection.bg_fill;
    save_style.fill_hovered = ui.visuals().selection.bg_fill.gamma_multiply(1.15);
    save_style.fill_active = ui.visuals().selection.bg_fill.gamma_multiply(0.92);
    save_style.text_color = ui.visuals().strong_text_color();

    let can_save = state.can_save();
    let response = ui.add_enabled_ui(can_save && !state.save_in_progress, |ui| {
        text_ui.button(ui, "skins_save", "Save", &save_style)
    });
    if response.inner.clicked() {
        state.start_save();
    }
}

fn render_preview(ui: &mut Ui, text_ui: &mut TextUi, state: &mut SkinManagerState) -> bool {
    let viewport_width = ui.clip_rect().width().max(1.0);
    let desired = egui::vec2(
        ui.available_width().min(viewport_width).max(1.0),
        PREVIEW_HEIGHT.min(ui.available_height().max(280.0)),
    );
    let (rect, response) = ui.allocate_exact_size(desired, Sense::click_and_drag());
    let painter = ui.painter_at(rect);

    paint_preview_background(&painter, rect);

    let now = ui.input(|i| i.time);
    let dt = state.consume_frame_dt(now);

    if response.drag_started() {
        state.begin_manual_camera_control(now);
        state.camera_drag_active = true;
        state.camera_inertial_velocity = 0.0;
    }
    if state.camera_drag_active && response.dragged() {
        let drag_step_x = ui.input(|i| i.pointer.delta().x);
        let yaw_step = drag_step_x * CAMERA_DRAG_SENSITIVITY_RAD_PER_POINT;
        state.camera_yaw_offset += yaw_step;
        if dt > 0.000_1 && yaw_step.abs() > 0.0 {
            let instant_velocity = (yaw_step / dt).clamp(
                -CAMERA_INERTIA_MAX_RAD_PER_SEC,
                CAMERA_INERTIA_MAX_RAD_PER_SEC,
            );
            state.camera_inertial_velocity = state.camera_inertial_velocity
                * (1.0 - CAMERA_INERTIA_VELOCITY_BLEND)
                + instant_velocity * CAMERA_INERTIA_VELOCITY_BLEND;
        }
    }
    if state.camera_drag_active && response.drag_stopped() {
        state.camera_drag_active = false;
        if state.camera_inertial_velocity.abs() < CAMERA_INERTIA_STOP_THRESHOLD_RAD_PER_SEC {
            state.camera_inertial_velocity = 0.0;
            state.finish_manual_camera_control(now);
        }
    }

    if !state.camera_drag_active && state.orbit_pause_started_at.is_some() {
        state.camera_yaw_offset += state.camera_inertial_velocity * dt;
        if dt > 0.0 {
            let friction = (-CAMERA_INERTIA_FRICTION_PER_SEC * dt).exp();
            state.camera_inertial_velocity *= friction;
        }
        if state.camera_inertial_velocity.abs() < CAMERA_INERTIA_STOP_THRESHOLD_RAD_PER_SEC {
            state.camera_inertial_velocity = 0.0;
            state.finish_manual_camera_control(now);
        }
    }

    let orbit_time = state.effective_orbit_time(now);
    let yaw = ((orbit_time / PREVIEW_ORBIT_SECONDS) as f32) * std::f32::consts::TAU
        + state.camera_yaw_offset;
    let walk = (now as f32 * 3.3).sin();

    let skin_image = state.skin_sample.as_ref();
    let cape_image = state.cape_sample.as_ref();
    let cape_uv = state.cape_uv;
    let variant = state.pending_variant;
    let show_elytra = state.show_elytra;
    let preview_texture = &mut state.preview_texture;

    if let Some(skin_image) = skin_image {
        draw_character(
            ui.ctx(),
            &painter,
            rect,
            preview_texture,
            skin_image,
            cape_image,
            cape_uv,
            yaw,
            walk,
            variant,
            show_elytra,
        );
    } else {
        ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
            ui.with_layout(
                egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
                |ui| {
                    let mut muted = style::muted(ui);
                    muted.wrap = false;
                    let _ = text_ui.label(ui, "skins_preview_no_skin", "No skin loaded", &muted);
                },
            );
        });
    }

    let toggle_rect = Rect::from_min_size(
        egui::pos2(rect.left() + 14.0, rect.bottom() - 46.0),
        egui::vec2(154.0, 32.0),
    );
    let toggle_text = if state.show_elytra {
        "Elytra: On"
    } else {
        "Elytra: Off"
    };
    let mut button_clicked = false;
    ui.scope_builder(egui::UiBuilder::new().max_rect(toggle_rect), |ui| {
        let mut toggle_style = neutral_button_style(ui);
        toggle_style.min_size = toggle_rect.size();
        let response = text_ui.button(
            ui,
            "skins_toggle_elytra_overlay",
            toggle_text,
            &toggle_style,
        );
        button_clicked = response.clicked();
    });
    button_clicked
}

fn paint_preview_background(painter: &egui::Painter, rect: Rect) {
    painter.rect_filled(
        rect,
        CornerRadius::same(8),
        Color32::from_rgba_premultiplied(23, 26, 32, 186),
    );
    painter.rect_stroke(
        rect,
        CornerRadius::same(8),
        Stroke::new(1.0, Color32::from_rgba_premultiplied(165, 173, 184, 42)),
        egui::StrokeKind::Outside,
    );
}

fn draw_character(
    ctx: &egui::Context,
    painter: &egui::Painter,
    rect: Rect,
    preview_texture: &mut Option<TextureHandle>,
    skin_image: &RgbaImage,
    cape_image: Option<&RgbaImage>,
    cape_uv: FaceUvs,
    yaw: f32,
    walk_phase: f32,
    variant: MinecraftSkinVariant,
    show_elytra: bool,
) {
    let arm_width = if variant == MinecraftSkinVariant::Slim {
        3.0
    } else {
        4.0
    };
    let bob = walk_phase.abs() * 0.55;
    let leg_swing = walk_phase * 0.62;
    let arm_swing = -walk_phase * 0.74;

    let target = Vec3::new(0.0, 19.5 + bob, 0.0);
    let camera_radius = 56.0;
    let camera_pos = Vec3::new(
        target.x + yaw.cos() * camera_radius,
        target.y + 25.0,
        target.z + yaw.sin() * camera_radius,
    );
    let camera = Camera::look_at(camera_pos, target, Vec3::new(0.0, 1.0, 0.0));
    let projection = Projection {
        fov_y_radians: 36.0_f32.to_radians(),
        near: 1.5,
    };

    let mut base_tris = Vec::with_capacity(180);
    let mut overlay_tris = Vec::with_capacity(140);
    let model_offset = Vec3::new(0.0, bob, 0.0);
    let light_dir = Vec3::new(0.35, 1.0, 0.2).normalized();

    let torso_uv = FaceUvs {
        top: uv_rect(20, 16, 8, 4),
        bottom: uv_rect(28, 16, 8, 4),
        left: uv_rect(28, 20, 4, 12),
        right: uv_rect(16, 20, 4, 12),
        front: uv_rect(20, 20, 8, 12),
        back: uv_rect(32, 20, 8, 12),
    };
    let torso_overlay_uv = FaceUvs {
        top: uv_rect(20, 32, 8, 4),
        bottom: uv_rect(28, 32, 8, 4),
        left: uv_rect(28, 36, 4, 12),
        right: uv_rect(16, 36, 4, 12),
        front: uv_rect(20, 36, 8, 12),
        back: uv_rect(32, 36, 8, 12),
    };

    let head_uv = FaceUvs {
        top: uv_rect(8, 0, 8, 8),
        bottom: uv_rect(16, 0, 8, 8),
        left: uv_rect(16, 8, 8, 8),
        right: uv_rect(0, 8, 8, 8),
        front: uv_rect(8, 8, 8, 8),
        back: uv_rect(24, 8, 8, 8),
    };
    let head_overlay_uv = FaceUvs {
        top: uv_rect(40, 0, 8, 8),
        bottom: uv_rect(48, 0, 8, 8),
        left: uv_rect(48, 8, 8, 8),
        right: uv_rect(32, 8, 8, 8),
        front: uv_rect(40, 8, 8, 8),
        back: uv_rect(56, 8, 8, 8),
    };

    let (right_arm_uv, left_arm_uv, right_arm_overlay_uv, left_arm_overlay_uv) =
        if variant == MinecraftSkinVariant::Slim {
            (
                FaceUvs {
                    top: uv_rect(44, 16, 3, 4),
                    bottom: uv_rect(47, 16, 3, 4),
                    left: uv_rect(47, 20, 3, 12),
                    right: uv_rect(40, 20, 3, 12),
                    front: uv_rect(44, 20, 3, 12),
                    back: uv_rect(51, 20, 3, 12),
                },
                FaceUvs {
                    top: uv_rect(36, 48, 3, 4),
                    bottom: uv_rect(39, 48, 3, 4),
                    left: uv_rect(39, 52, 3, 12),
                    right: uv_rect(32, 52, 3, 12),
                    front: uv_rect(36, 52, 3, 12),
                    back: uv_rect(43, 52, 3, 12),
                },
                FaceUvs {
                    top: uv_rect(44, 32, 3, 4),
                    bottom: uv_rect(47, 32, 3, 4),
                    left: uv_rect(47, 36, 3, 12),
                    right: uv_rect(40, 36, 3, 12),
                    front: uv_rect(44, 36, 3, 12),
                    back: uv_rect(51, 36, 3, 12),
                },
                FaceUvs {
                    top: uv_rect(52, 48, 3, 4),
                    bottom: uv_rect(55, 48, 3, 4),
                    left: uv_rect(55, 52, 3, 12),
                    right: uv_rect(48, 52, 3, 12),
                    front: uv_rect(52, 52, 3, 12),
                    back: uv_rect(59, 52, 3, 12),
                },
            )
        } else {
            (
                FaceUvs {
                    top: uv_rect(44, 16, 4, 4),
                    bottom: uv_rect(48, 16, 4, 4),
                    left: uv_rect(48, 20, 4, 12),
                    right: uv_rect(40, 20, 4, 12),
                    front: uv_rect(44, 20, 4, 12),
                    back: uv_rect(52, 20, 4, 12),
                },
                FaceUvs {
                    top: uv_rect(36, 48, 4, 4),
                    bottom: uv_rect(40, 48, 4, 4),
                    left: uv_rect(40, 52, 4, 12),
                    right: uv_rect(32, 52, 4, 12),
                    front: uv_rect(36, 52, 4, 12),
                    back: uv_rect(44, 52, 4, 12),
                },
                FaceUvs {
                    top: uv_rect(44, 32, 4, 4),
                    bottom: uv_rect(48, 32, 4, 4),
                    left: uv_rect(48, 36, 4, 12),
                    right: uv_rect(40, 36, 4, 12),
                    front: uv_rect(44, 36, 4, 12),
                    back: uv_rect(52, 36, 4, 12),
                },
                FaceUvs {
                    top: uv_rect(52, 48, 4, 4),
                    bottom: uv_rect(56, 48, 4, 4),
                    left: uv_rect(56, 52, 4, 12),
                    right: uv_rect(48, 52, 4, 12),
                    front: uv_rect(52, 52, 4, 12),
                    back: uv_rect(60, 52, 4, 12),
                },
            )
        };

    let right_leg_uv = FaceUvs {
        top: uv_rect(4, 16, 4, 4),
        bottom: uv_rect(8, 16, 4, 4),
        left: uv_rect(8, 20, 4, 12),
        right: uv_rect(0, 20, 4, 12),
        front: uv_rect(4, 20, 4, 12),
        back: uv_rect(12, 20, 4, 12),
    };
    let left_leg_uv = FaceUvs {
        top: uv_rect(20, 48, 4, 4),
        bottom: uv_rect(24, 48, 4, 4),
        left: uv_rect(24, 52, 4, 12),
        right: uv_rect(16, 52, 4, 12),
        front: uv_rect(20, 52, 4, 12),
        back: uv_rect(28, 52, 4, 12),
    };
    let leg_overlay_uv = FaceUvs {
        top: uv_rect(4, 48, 4, 4),
        bottom: uv_rect(8, 48, 4, 4),
        left: uv_rect(8, 52, 4, 12),
        right: uv_rect(0, 52, 4, 12),
        front: uv_rect(4, 52, 4, 12),
        back: uv_rect(12, 52, 4, 12),
    };

    add_cuboid_triangles(
        &mut base_tris,
        TriangleTexture::Skin,
        CuboidSpec {
            size: Vec3::new(8.0, 12.0, 4.0),
            pivot_top_center: Vec3::new(0.0, 24.0, 0.0) + model_offset,
            rotate_x: 0.0,
            uv: torso_uv,
        },
        &camera,
        projection,
        rect,
        light_dir,
    );
    add_cuboid_triangles(
        &mut overlay_tris,
        TriangleTexture::Skin,
        CuboidSpec {
            size: Vec3::new(8.6, 12.6, 4.6),
            pivot_top_center: Vec3::new(0.0, 24.2, 0.0) + model_offset,
            rotate_x: 0.0,
            uv: torso_overlay_uv,
        },
        &camera,
        projection,
        rect,
        light_dir,
    );

    add_cuboid_triangles(
        &mut base_tris,
        TriangleTexture::Skin,
        CuboidSpec {
            size: Vec3::new(8.0, 8.0, 8.0),
            pivot_top_center: Vec3::new(0.0, 32.0, 0.0) + model_offset,
            rotate_x: 0.0,
            uv: head_uv,
        },
        &camera,
        projection,
        rect,
        light_dir,
    );
    add_cuboid_triangles(
        &mut overlay_tris,
        TriangleTexture::Skin,
        CuboidSpec {
            size: Vec3::new(8.8, 8.8, 8.8),
            pivot_top_center: Vec3::new(0.0, 32.4, 0.0) + model_offset,
            rotate_x: 0.0,
            uv: head_overlay_uv,
        },
        &camera,
        projection,
        rect,
        light_dir,
    );

    let shoulder_x = 4.0 + arm_width * 0.5;
    add_cuboid_triangles(
        &mut base_tris,
        TriangleTexture::Skin,
        CuboidSpec {
            size: Vec3::new(arm_width, 12.0, 4.0),
            pivot_top_center: Vec3::new(-shoulder_x, 24.0, 0.0) + model_offset,
            rotate_x: arm_swing,
            uv: left_arm_uv,
        },
        &camera,
        projection,
        rect,
        light_dir,
    );
    add_cuboid_triangles(
        &mut overlay_tris,
        TriangleTexture::Skin,
        CuboidSpec {
            size: Vec3::new(arm_width + 0.55, 12.55, 4.55),
            pivot_top_center: Vec3::new(-shoulder_x, 24.15, 0.0) + model_offset,
            rotate_x: arm_swing,
            uv: left_arm_overlay_uv,
        },
        &camera,
        projection,
        rect,
        light_dir,
    );
    add_cuboid_triangles(
        &mut base_tris,
        TriangleTexture::Skin,
        CuboidSpec {
            size: Vec3::new(arm_width, 12.0, 4.0),
            pivot_top_center: Vec3::new(shoulder_x, 24.0, 0.0) + model_offset,
            rotate_x: -arm_swing,
            uv: right_arm_uv,
        },
        &camera,
        projection,
        rect,
        light_dir,
    );
    add_cuboid_triangles(
        &mut overlay_tris,
        TriangleTexture::Skin,
        CuboidSpec {
            size: Vec3::new(arm_width + 0.55, 12.55, 4.55),
            pivot_top_center: Vec3::new(shoulder_x, 24.15, 0.0) + model_offset,
            rotate_x: -arm_swing,
            uv: right_arm_overlay_uv,
        },
        &camera,
        projection,
        rect,
        light_dir,
    );

    add_cuboid_triangles(
        &mut base_tris,
        TriangleTexture::Skin,
        CuboidSpec {
            size: Vec3::new(4.0, 12.0, 4.0),
            pivot_top_center: Vec3::new(-2.0, 12.0, 0.0) + model_offset,
            rotate_x: leg_swing,
            uv: left_leg_uv,
        },
        &camera,
        projection,
        rect,
        light_dir,
    );
    add_cuboid_triangles(
        &mut overlay_tris,
        TriangleTexture::Skin,
        CuboidSpec {
            size: Vec3::new(4.55, 12.55, 4.55),
            pivot_top_center: Vec3::new(-2.0, 12.15, 0.0) + model_offset,
            rotate_x: leg_swing,
            uv: leg_overlay_uv,
        },
        &camera,
        projection,
        rect,
        light_dir,
    );

    add_cuboid_triangles(
        &mut base_tris,
        TriangleTexture::Skin,
        CuboidSpec {
            size: Vec3::new(4.0, 12.0, 4.0),
            pivot_top_center: Vec3::new(2.0, 12.0, 0.0) + model_offset,
            rotate_x: -leg_swing,
            uv: right_leg_uv,
        },
        &camera,
        projection,
        rect,
        light_dir,
    );
    add_cuboid_triangles(
        &mut overlay_tris,
        TriangleTexture::Skin,
        CuboidSpec {
            size: Vec3::new(4.55, 12.55, 4.55),
            pivot_top_center: Vec3::new(2.0, 12.15, 0.0) + model_offset,
            rotate_x: -leg_swing,
            uv: leg_overlay_uv,
        },
        &camera,
        projection,
        rect,
        light_dir,
    );

    let mut scene_tris = base_tris;
    scene_tris.extend(overlay_tris);
    if cape_image.is_some() {
        add_cape_triangles(
            &mut scene_tris,
            TriangleTexture::Cape,
            &camera,
            projection,
            rect,
            model_offset,
            walk_phase,
            cape_uv,
            light_dir,
        );
    }
    render_depth_buffered_scene(
        ctx,
        painter,
        rect,
        preview_texture,
        &scene_tris,
        skin_image,
        cape_image,
    );

    if show_elytra {
        draw_elytra_preview(painter, &camera, projection, rect, model_offset);
    }
}

fn add_cape_triangles(
    out: &mut Vec<RenderTriangle>,
    texture: TriangleTexture,
    camera: &Camera,
    projection: Projection,
    rect: Rect,
    model_offset: Vec3,
    walk_phase: f32,
    cape_uv: FaceUvs,
    light_dir: Vec3,
) {
    let pivot = Vec3::new(0.0, 24.0, -2.55) + model_offset;
    let cape_tilt = 0.12 + walk_phase.abs() * 0.10;
    add_cuboid_triangles(
        out,
        texture,
        CuboidSpec {
            size: Vec3::new(10.0, 16.0, 1.0),
            pivot_top_center: pivot,
            rotate_x: cape_tilt,
            uv: cape_uv,
        },
        camera,
        projection,
        rect,
        light_dir,
    );
}

fn render_depth_buffered_scene(
    ctx: &egui::Context,
    painter: &egui::Painter,
    rect: Rect,
    preview_texture: &mut Option<TextureHandle>,
    triangles: &[RenderTriangle],
    skin_image: &RgbaImage,
    cape_image: Option<&RgbaImage>,
) {
    let width = rect.width().round().max(1.0) as usize;
    let height = rect.height().round().max(1.0) as usize;
    let mut color = vec![0u8; width * height * 4];
    let mut depth = vec![f32::INFINITY; width * height];

    for tri in triangles {
        let texture = match tri.texture {
            TriangleTexture::Skin => skin_image,
            TriangleTexture::Cape => match cape_image {
                Some(image) => image,
                None => continue,
            },
        };
        rasterize_triangle_depth_tested(&mut color, &mut depth, width, height, rect, tri, texture);
    }

    let color_image = egui::ColorImage::from_rgba_unmultiplied([width, height], &color);
    if let Some(texture) = preview_texture.as_mut() {
        texture.set(color_image, TextureOptions::LINEAR);
    } else {
        *preview_texture = Some(ctx.load_texture(
            "skins/preview/rasterized-frame",
            color_image,
            TextureOptions::LINEAR,
        ));
    }

    if let Some(texture) = preview_texture.as_ref() {
        painter.image(texture.id(), rect, full_uv_rect(), Color32::WHITE);
    }
}

fn rasterize_triangle_depth_tested(
    color_buffer: &mut [u8],
    depth_buffer: &mut [f32],
    width: usize,
    height: usize,
    rect: Rect,
    tri: &RenderTriangle,
    texture: &RgbaImage,
) {
    let p0 = Pos2::new(tri.pos[0].x - rect.left(), tri.pos[0].y - rect.top());
    let p1 = Pos2::new(tri.pos[1].x - rect.left(), tri.pos[1].y - rect.top());
    let p2 = Pos2::new(tri.pos[2].x - rect.left(), tri.pos[2].y - rect.top());
    let area = edge_function(p0, p1, p2);
    if area.abs() <= 0.000_01 {
        return;
    }

    let min_x = p0.x.min(p1.x).min(p2.x).floor().max(0.0) as i32;
    let min_y = p0.y.min(p1.y).min(p2.y).floor().max(0.0) as i32;
    let max_x = p0.x.max(p1.x).max(p2.x).ceil().min(width as f32 - 1.0) as i32;
    let max_y = p0.y.max(p1.y).max(p2.y).ceil().min(height as f32 - 1.0) as i32;
    if min_x > max_x || min_y > max_y {
        return;
    }

    let inv_area = 1.0 / area;
    let inv_z0 = 1.0 / tri.depth[0].max(0.000_1);
    let inv_z1 = 1.0 / tri.depth[1].max(0.000_1);
    let inv_z2 = 1.0 / tri.depth[2].max(0.000_1);

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let sample = Pos2::new(x as f32 + 0.5, y as f32 + 0.5);
            let w0 = edge_function(p1, p2, sample) * inv_area;
            let w1 = edge_function(p2, p0, sample) * inv_area;
            let w2 = 1.0 - w0 - w1;
            if w0 < -0.000_1 || w1 < -0.000_1 || w2 < -0.000_1 {
                continue;
            }

            let pixel_index = (y as usize) * width + (x as usize);
            let depth = w0 * tri.depth[0] + w1 * tri.depth[1] + w2 * tri.depth[2];
            if depth >= depth_buffer[pixel_index] {
                continue;
            }

            let inv_z = w0 * inv_z0 + w1 * inv_z1 + w2 * inv_z2;
            if inv_z <= 0.0 {
                continue;
            }
            let u =
                (w0 * tri.uv[0].x * inv_z0 + w1 * tri.uv[1].x * inv_z1 + w2 * tri.uv[2].x * inv_z2)
                    / inv_z;
            let v =
                (w0 * tri.uv[0].y * inv_z0 + w1 * tri.uv[1].y * inv_z1 + w2 * tri.uv[2].y * inv_z2)
                    / inv_z;
            let texel = sample_texture_nearest(texture, u, v);
            let tinted = tint_rgba(texel, tri.color);
            if tinted[3] == 0 {
                continue;
            }

            blend_rgba_over(color_buffer, pixel_index * 4, tinted);
            depth_buffer[pixel_index] = depth;
        }
    }
}

fn sample_texture_nearest(texture: &RgbaImage, u: f32, v: f32) -> [u8; 4] {
    let width = texture.width() as usize;
    let height = texture.height() as usize;
    if width == 0 || height == 0 {
        return [0, 0, 0, 0];
    }
    let x = (u.clamp(0.0, 1.0) * width as f32).floor() as usize;
    let y = (v.clamp(0.0, 1.0) * height as f32).floor() as usize;
    let x = x.min(width - 1);
    let y = y.min(height - 1);
    let idx = (y * width + x) * 4;
    let raw = texture.as_raw();
    [raw[idx], raw[idx + 1], raw[idx + 2], raw[idx + 3]]
}

fn tint_rgba(color: [u8; 4], tint: Color32) -> [u8; 4] {
    [
        ((color[0] as u16 * tint.r() as u16) / 255) as u8,
        ((color[1] as u16 * tint.g() as u16) / 255) as u8,
        ((color[2] as u16 * tint.b() as u16) / 255) as u8,
        ((color[3] as u16 * tint.a() as u16) / 255) as u8,
    ]
}

fn blend_rgba_over(buffer: &mut [u8], base: usize, src: [u8; 4]) {
    let src_a = src[3] as f32 / 255.0;
    if src_a <= 0.0 {
        return;
    }
    let dst_r = buffer[base] as f32 / 255.0;
    let dst_g = buffer[base + 1] as f32 / 255.0;
    let dst_b = buffer[base + 2] as f32 / 255.0;
    let dst_a = buffer[base + 3] as f32 / 255.0;

    let src_r = src[0] as f32 / 255.0;
    let src_g = src[1] as f32 / 255.0;
    let src_b = src[2] as f32 / 255.0;

    let out_a = src_a + dst_a * (1.0 - src_a);
    if out_a <= 0.0 {
        return;
    }
    let out_r = (src_r * src_a + dst_r * dst_a * (1.0 - src_a)) / out_a;
    let out_g = (src_g * src_a + dst_g * dst_a * (1.0 - src_a)) / out_a;
    let out_b = (src_b * src_a + dst_b * dst_a * (1.0 - src_a)) / out_a;

    buffer[base] = (out_r * 255.0).round() as u8;
    buffer[base + 1] = (out_g * 255.0).round() as u8;
    buffer[base + 2] = (out_b * 255.0).round() as u8;
    buffer[base + 3] = (out_a * 255.0).round() as u8;
}

fn edge_function(a: Pos2, b: Pos2, p: Pos2) -> f32 {
    (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x)
}

#[derive(Clone, Copy)]
struct Vec3 {
    x: f32,
    y: f32,
    z: f32,
}

impl Vec3 {
    fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    fn dot(self, rhs: Self) -> f32 {
        self.x * rhs.x + self.y * rhs.y + self.z * rhs.z
    }

    fn cross(self, rhs: Self) -> Self {
        Self::new(
            self.y * rhs.z - self.z * rhs.y,
            self.z * rhs.x - self.x * rhs.z,
            self.x * rhs.y - self.y * rhs.x,
        )
    }

    fn length(self) -> f32 {
        self.dot(self).sqrt()
    }

    fn normalized(self) -> Self {
        let len = self.length();
        if len <= 0.000_1 {
            Self::new(0.0, 0.0, 0.0)
        } else {
            self * (1.0 / len)
        }
    }
}

impl std::ops::Add for Vec3 {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.x + rhs.x, self.y + rhs.y, self.z + rhs.z)
    }
}

impl std::ops::Sub for Vec3 {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(self.x - rhs.x, self.y - rhs.y, self.z - rhs.z)
    }
}

impl std::ops::Mul<f32> for Vec3 {
    type Output = Self;

    fn mul(self, rhs: f32) -> Self::Output {
        Self::new(self.x * rhs, self.y * rhs, self.z * rhs)
    }
}

#[derive(Clone, Copy)]
struct Camera {
    position: Vec3,
    right: Vec3,
    up: Vec3,
    forward: Vec3,
}

impl Camera {
    fn look_at(position: Vec3, target: Vec3, world_up: Vec3) -> Self {
        let forward = (target - position).normalized();
        let right = forward.cross(world_up).normalized();
        let up = right.cross(forward).normalized();
        Self {
            position,
            right,
            up,
            forward,
        }
    }

    fn world_to_camera(self, world: Vec3) -> Vec3 {
        let rel = world - self.position;
        Vec3::new(rel.dot(self.right), rel.dot(self.up), rel.dot(self.forward))
    }
}

#[derive(Clone, Copy)]
struct Projection {
    fov_y_radians: f32,
    near: f32,
}

#[derive(Clone, Copy)]
struct FaceUvs {
    top: Rect,
    bottom: Rect,
    left: Rect,
    right: Rect,
    front: Rect,
    back: Rect,
}

#[derive(Clone, Copy)]
struct CuboidSpec {
    size: Vec3,
    pivot_top_center: Vec3,
    rotate_x: f32,
    uv: FaceUvs,
}

#[derive(Clone, Copy)]
enum TriangleTexture {
    Skin,
    Cape,
}

struct RenderTriangle {
    texture: TriangleTexture,
    pos: [Pos2; 3],
    uv: [Pos2; 3],
    depth: [f32; 3],
    color: Color32,
}

fn rotate_x(point: Vec3, radians: f32) -> Vec3 {
    let (sin, cos) = radians.sin_cos();
    Vec3::new(
        point.x,
        point.y * cos - point.z * sin,
        point.y * sin + point.z * cos,
    )
}

fn project_point(camera_space: Vec3, projection: Projection, rect: Rect) -> Option<Pos2> {
    if camera_space.z <= projection.near {
        return None;
    }

    let aspect = (rect.width() / rect.height().max(1.0)).max(0.01);
    let tan_half_fov = (projection.fov_y_radians * 0.5).tan().max(0.01);
    let x_ndc = camera_space.x / (camera_space.z * tan_half_fov * aspect);
    let y_ndc = camera_space.y / (camera_space.z * tan_half_fov);
    let x = rect.center().x + x_ndc * (rect.width() * 0.5);
    let y = rect.center().y - y_ndc * (rect.height() * 0.5);
    Some(Pos2::new(x, y))
}

fn color_with_brightness(base: Color32, brightness: f32) -> Color32 {
    let b = brightness.clamp(0.0, 1.0);
    Color32::from_rgba_premultiplied(
        ((base.r() as f32) * b).round() as u8,
        ((base.g() as f32) * b).round() as u8,
        ((base.b() as f32) * b).round() as u8,
        base.a(),
    )
}

fn add_cuboid_triangles(
    out: &mut Vec<RenderTriangle>,
    texture: TriangleTexture,
    spec: CuboidSpec,
    camera: &Camera,
    projection: Projection,
    rect: Rect,
    light_dir: Vec3,
) {
    let w = spec.size.x;
    let h = spec.size.y;
    let d = spec.size.z;
    let x0 = -w * 0.5;
    let x1 = w * 0.5;
    let y0 = 0.0;
    let y1 = -h;
    let z0 = -d * 0.5;
    let z1 = d * 0.5;

    let faces = [
        (
            [
                Vec3::new(x0, y0, z1),
                Vec3::new(x1, y0, z1),
                Vec3::new(x1, y1, z1),
                Vec3::new(x0, y1, z1),
            ],
            spec.uv.front,
            Vec3::new(0.0, 0.0, 1.0),
        ),
        (
            [
                Vec3::new(x1, y0, z0),
                Vec3::new(x0, y0, z0),
                Vec3::new(x0, y1, z0),
                Vec3::new(x1, y1, z0),
            ],
            spec.uv.back,
            Vec3::new(0.0, 0.0, -1.0),
        ),
        (
            [
                Vec3::new(x0, y0, z0),
                Vec3::new(x0, y0, z1),
                Vec3::new(x0, y1, z1),
                Vec3::new(x0, y1, z0),
            ],
            spec.uv.left,
            Vec3::new(-1.0, 0.0, 0.0),
        ),
        (
            [
                Vec3::new(x1, y0, z1),
                Vec3::new(x1, y0, z0),
                Vec3::new(x1, y1, z0),
                Vec3::new(x1, y1, z1),
            ],
            spec.uv.right,
            Vec3::new(1.0, 0.0, 0.0),
        ),
        (
            [
                Vec3::new(x0, y0, z0),
                Vec3::new(x1, y0, z0),
                Vec3::new(x1, y0, z1),
                Vec3::new(x0, y0, z1),
            ],
            spec.uv.top,
            Vec3::new(0.0, 1.0, 0.0),
        ),
        (
            [
                Vec3::new(x0, y1, z1),
                Vec3::new(x1, y1, z1),
                Vec3::new(x1, y1, z0),
                Vec3::new(x0, y1, z0),
            ],
            spec.uv.bottom,
            Vec3::new(0.0, -1.0, 0.0),
        ),
    ];

    for (quad, uv_rect, normal) in faces {
        let world_normal = rotate_x(normal, spec.rotate_x).normalized();
        let brightness = 0.58 + world_normal.dot(light_dir).max(0.0) * 0.42;
        let tint = color_with_brightness(Color32::WHITE, brightness);

        let transformed =
            quad.map(|vertex| rotate_x(vertex, spec.rotate_x) + spec.pivot_top_center);
        let camera_vertices = transformed.map(|v| camera.world_to_camera(v));
        if camera_vertices.iter().any(|v| v.z <= projection.near) {
            continue;
        }
        // Cull faces that point away from the camera to reduce overdraw artifacts.
        let normal_camera = Vec3::new(
            world_normal.dot(camera.right),
            world_normal.dot(camera.up),
            world_normal.dot(camera.forward),
        );
        let center_camera =
            (camera_vertices[0] + camera_vertices[1] + camera_vertices[2] + camera_vertices[3])
                * 0.25;
        let to_camera = (Vec3::new(0.0, 0.0, 0.0) - center_camera).normalized();
        if normal_camera.dot(to_camera) <= 0.0 {
            continue;
        }
        let projected = camera_vertices.map(|v| project_point(v, projection, rect));
        if projected.iter().any(Option::is_none) {
            continue;
        }
        let projected = projected.map(Option::unwrap);

        let uv0 = uv_rect.left_top();
        let uv1 = uv_rect.right_top();
        let uv2 = uv_rect.right_bottom();
        let uv3 = uv_rect.left_bottom();
        out.push(RenderTriangle {
            texture,
            pos: [projected[0], projected[1], projected[2]],
            uv: [uv0, uv1, uv2],
            depth: [
                camera_vertices[0].z,
                camera_vertices[1].z,
                camera_vertices[2].z,
            ],
            color: tint,
        });
        out.push(RenderTriangle {
            texture,
            pos: [projected[0], projected[2], projected[3]],
            uv: [uv0, uv2, uv3],
            depth: [
                camera_vertices[0].z,
                camera_vertices[2].z,
                camera_vertices[3].z,
            ],
            color: tint,
        });
    }
}

fn draw_elytra_preview(
    painter: &egui::Painter,
    camera: &Camera,
    projection: Projection,
    rect: Rect,
    model_offset: Vec3,
) {
    let wing_color = Color32::from_rgba_premultiplied(145, 70, 70, 92);
    let left = [
        Vec3::new(-2.0, 24.0, -2.2),
        Vec3::new(-13.0, 23.0, -3.3),
        Vec3::new(-13.0, 10.0, -2.8),
        Vec3::new(-2.0, 12.0, -1.8),
    ]
    .map(|v| v + model_offset);
    let right = [
        Vec3::new(2.0, 24.0, -2.2),
        Vec3::new(13.0, 23.0, -3.3),
        Vec3::new(13.0, 10.0, -2.8),
        Vec3::new(2.0, 12.0, -1.8),
    ]
    .map(|v| v + model_offset);
    for wing in [left, right] {
        let camera_vertices = wing.map(|v| camera.world_to_camera(v));
        if camera_vertices.iter().any(|v| v.z <= projection.near) {
            continue;
        }
        let projected = camera_vertices.map(|v| project_point(v, projection, rect));
        if projected.iter().any(Option::is_none) {
            continue;
        }
        let projected = projected.map(Option::unwrap);
        painter.add(egui::Shape::convex_polygon(
            vec![projected[0], projected[1], projected[2], projected[3]],
            wing_color,
            Stroke::new(1.0, Color32::from_rgba_premultiplied(210, 120, 120, 120)),
        ));
    }
}

fn uv_rect(x: u32, y: u32, w: u32, h: u32) -> Rect {
    uv_rect_with_inset([64, 64], x, y, w, h)
}

fn uv_rect_with_inset(texture_size: [u32; 2], x: u32, y: u32, w: u32, h: u32) -> Rect {
    let tex_w = texture_size[0].max(1) as f32;
    let tex_h = texture_size[1].max(1) as f32;
    let max_inset_x = ((w as f32) * 0.49) / tex_w;
    let max_inset_y = ((h as f32) * 0.49) / tex_h;
    let inset_x = (UV_EDGE_INSET_TEXELS / tex_w).min(max_inset_x);
    let inset_y = (UV_EDGE_INSET_TEXELS / tex_h).min(max_inset_y);
    let min_x = (x as f32 / tex_w) + inset_x;
    let min_y = (y as f32 / tex_h) + inset_y;
    let max_x = ((x + w) as f32 / tex_w) - inset_x;
    let max_y = ((y + h) as f32 / tex_h) - inset_y;
    Rect::from_min_max(egui::pos2(min_x, min_y), egui::pos2(max_x, max_y))
}

fn render_cape_grid(ui: &mut Ui, text_ui: &mut TextUi, state: &mut SkinManagerState) {
    let label_font = egui::TextStyle::Body.resolve(ui.style());
    let label_color = ui.visuals().text_color();
    let mut max_label_width = ui
        .painter()
        .layout_no_wrap("No Cape".to_owned(), label_font.clone(), label_color)
        .size()
        .x;
    for cape in &state.available_capes {
        let width = ui
            .painter()
            .layout_no_wrap(cape.label.clone(), label_font.clone(), label_color)
            .size()
            .x;
        max_label_width = max_label_width.max(width);
    }

    let available_rect = ui.available_rect_before_wrap().intersect(ui.clip_rect());
    let available_width = available_rect.width().max(1.0);
    let tile_width = (max_label_width + 24.0)
        .max(CAPE_TILE_WIDTH_MIN)
        .min(available_width);

    let spacing_x = style::SPACE_XS;
    let spacing_y = style::SPACE_XS;

    let total_items = state.available_capes.len() + 1;
    let row_width = |item_count: usize| {
        (item_count as f32 * tile_width) + ((item_count.saturating_sub(1)) as f32 * spacing_x)
    };
    let mut columns = 1usize;
    for candidate in 1..=total_items.max(1) {
        if row_width(candidate) <= available_width + f32::EPSILON {
            columns = candidate;
        } else {
            break;
        }
    }

    let mut pending_selection = None;
    let mut row_start = 0usize;
    while row_start < total_items {
        let row_end = (row_start + columns).min(total_items);
        let row_items = row_end - row_start;
        let row_leading_space = ((available_width - row_width(row_items)) * 0.5).max(0.0);
        ui.allocate_ui_with_layout(
            egui::vec2(available_width, CAPE_TILE_HEIGHT),
            egui::Layout::left_to_right(egui::Align::Min),
            |ui| {
                if row_leading_space > 0.0 {
                    ui.add_space(row_leading_space);
                }
                let mut is_first_in_row = true;
                for item_index in row_start..row_end {
                    if !is_first_in_row {
                        ui.add_space(spacing_x);
                    }
                    is_first_in_row = false;

                    if item_index == 0 {
                        let no_cape_selected = state.pending_cape_id.is_none();
                        if draw_cape_tile(
                            ui,
                            text_ui,
                            tile_width,
                            "No Cape",
                            no_cape_selected,
                            true,
                            None,
                            None,
                        ) {
                            pending_selection = Some(None);
                        }
                        continue;
                    }

                    let cape = &state.available_capes[item_index - 1];
                    let selected = state.pending_cape_id.as_deref() == Some(cape.id.as_str());
                    let preview = cape.texture_bytes.as_deref();
                    if draw_cape_tile(
                        ui,
                        text_ui,
                        tile_width,
                        cape.label.as_str(),
                        selected,
                        false,
                        preview,
                        cape.texture_size,
                    ) {
                        pending_selection = Some(Some(cape.id.clone()));
                    }
                }
            },
        );
        if row_start + columns < total_items {
            ui.add_space(spacing_y);
        }
        row_start = row_end;
    }

    if let Some(selection) = pending_selection {
        state.pending_cape_id = selection;
    }
}

fn draw_cape_tile(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    tile_width: f32,
    label: &str,
    selected: bool,
    is_no_cape: bool,
    preview_png: Option<&[u8]>,
    preview_texture_size: Option<[u32; 2]>,
) -> bool {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(tile_width, CAPE_TILE_HEIGHT), Sense::click());

    let fill = if selected {
        ui.visuals().selection.bg_fill.gamma_multiply(0.3)
    } else {
        ui.visuals().widgets.inactive.bg_fill
    };
    let stroke = if selected {
        ui.visuals().selection.stroke
    } else {
        ui.visuals().widgets.inactive.bg_stroke
    };

    ui.painter().rect(
        rect,
        CornerRadius::same(10),
        fill,
        stroke,
        egui::StrokeKind::Middle,
    );

    let preview_rect = Rect::from_min_size(
        egui::pos2(rect.left() + 12.0, rect.top() + 12.0),
        egui::vec2((tile_width - 24.0).max(0.0), 112.0),
    );

    if is_no_cape {
        ui.painter().rect_stroke(
            preview_rect,
            CornerRadius::same(6),
            Stroke::new(1.5, ui.visuals().weak_text_color()),
            egui::StrokeKind::Middle,
        );
        let dotted_step = 8.0;
        let mut x = preview_rect.left();
        while x <= preview_rect.right() {
            ui.painter().line_segment(
                [
                    egui::pos2(x, preview_rect.top()),
                    egui::pos2((x + 3.0).min(preview_rect.right()), preview_rect.top()),
                ],
                Stroke::new(1.0, ui.visuals().weak_text_color()),
            );
            ui.painter().line_segment(
                [
                    egui::pos2(x, preview_rect.bottom()),
                    egui::pos2((x + 3.0).min(preview_rect.right()), preview_rect.bottom()),
                ],
                Stroke::new(1.0, ui.visuals().weak_text_color()),
            );
            x += dotted_step;
        }
        let mut y = preview_rect.top();
        while y <= preview_rect.bottom() {
            ui.painter().line_segment(
                [
                    egui::pos2(preview_rect.left(), y),
                    egui::pos2(preview_rect.left(), (y + 3.0).min(preview_rect.bottom())),
                ],
                Stroke::new(1.0, ui.visuals().weak_text_color()),
            );
            ui.painter().line_segment(
                [
                    egui::pos2(preview_rect.right(), y),
                    egui::pos2(preview_rect.right(), (y + 3.0).min(preview_rect.bottom())),
                ],
                Stroke::new(1.0, ui.visuals().weak_text_color()),
            );
            y += dotted_step;
        }
    } else if let Some(bytes) = preview_png {
        let mut hasher = DefaultHasher::new();
        bytes.hash(&mut hasher);
        let uri = format!("bytes://skins/cape/{:016x}.png", hasher.finish());

        if let Some(back_uv) = preview_texture_size.and_then(cape_outer_face_uv) {
            let inner = preview_rect.shrink2(egui::vec2(4.0, 4.0));
            let target_aspect = 10.0 / 16.0;
            let max_h = inner.height();
            let mut face_h = max_h;
            let mut face_w = face_h * target_aspect;
            if face_w > inner.width() {
                face_w = inner.width().max(0.0);
                face_h = face_w / target_aspect;
            }
            let y = inner.center().y - face_h * 0.5;
            let x = inner.center().x - face_w * 0.5;
            let back_rect = Rect::from_min_size(egui::pos2(x, y), egui::vec2(face_w, face_h));

            ui.painter().rect_stroke(
                back_rect,
                CornerRadius::same(4),
                Stroke::new(1.0, ui.visuals().widgets.inactive.bg_stroke.color),
                egui::StrokeKind::Middle,
            );

            egui::Image::from_bytes(uri, bytes.to_vec())
                .uv(back_uv)
                .fit_to_exact_size(back_rect.size())
                .texture_options(TextureOptions::NEAREST)
                .paint_at(ui, back_rect);
        } else {
            let image = egui::Image::from_bytes(uri, bytes.to_vec())
                .fit_to_exact_size(preview_rect.size())
                .texture_options(TextureOptions::NEAREST);
            image.paint_at(ui, preview_rect);
        }
    } else {
        ui.painter().rect_filled(
            preview_rect,
            CornerRadius::same(6),
            ui.visuals().widgets.noninteractive.bg_fill,
        );
    }

    let label_rect = Rect::from_min_size(
        Pos2::new(rect.left() + 6.0, rect.bottom() - 44.0),
        egui::vec2(rect.width() - 12.0, 34.0),
    );
    ui.scope_builder(egui::UiBuilder::new().max_rect(label_rect), |ui| {
        ui.set_clip_rect(label_rect);
        ui.with_layout(
            egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
            |ui| {
                let mut label_style = style::body(ui);
                label_style.wrap = false;
                let _ = text_ui.label(ui, ("skins_cape_label", label), label, &label_style);
            },
        );
    });

    response.clicked()
}

#[derive(Clone, Debug, Default)]
struct CapeChoice {
    id: String,
    label: String,
    texture_bytes: Option<Vec<u8>>,
    texture_size: Option<[u32; 2]>,
}

#[derive(Clone)]
struct SkinManagerState {
    active_profile_id: Option<String>,
    active_player_name: Option<String>,
    access_token: Option<String>,
    base_skin_png: Option<Vec<u8>>,
    pending_skin_png: Option<Vec<u8>>,
    pending_skin_path: Option<String>,
    pending_variant: MinecraftSkinVariant,
    available_capes: Vec<CapeChoice>,
    initial_cape_id: Option<String>,
    pending_cape_id: Option<String>,
    show_elytra: bool,
    status_message: Option<String>,
    save_in_progress: bool,
    refresh_in_progress: bool,
    worker_rx: Option<Arc<Mutex<Receiver<WorkerEvent>>>>,
    skin_texture_hash: Option<u64>,
    skin_texture: Option<TextureHandle>,
    skin_sample: Option<RgbaImage>,
    cape_texture_hash: Option<u64>,
    cape_texture: Option<TextureHandle>,
    cape_sample: Option<RgbaImage>,
    preview_texture: Option<TextureHandle>,
    cape_uv: FaceUvs,
    camera_yaw_offset: f32,
    camera_inertial_velocity: f32,
    camera_drag_active: bool,
    orbit_pause_started_at: Option<f64>,
    orbit_pause_accumulated_secs: f64,
    camera_last_frame_time: Option<f64>,
}

impl Default for SkinManagerState {
    fn default() -> Self {
        Self {
            active_profile_id: None,
            active_player_name: None,
            access_token: None,
            base_skin_png: None,
            pending_skin_png: None,
            pending_skin_path: None,
            pending_variant: MinecraftSkinVariant::Classic,
            available_capes: Vec::new(),
            initial_cape_id: None,
            pending_cape_id: None,
            show_elytra: false,
            status_message: None,
            save_in_progress: false,
            refresh_in_progress: false,
            worker_rx: None,
            skin_texture_hash: None,
            skin_texture: None,
            skin_sample: None,
            cape_texture_hash: None,
            cape_texture: None,
            cape_sample: None,
            preview_texture: None,
            cape_uv: default_cape_uv_layout(),
            camera_yaw_offset: 0.0,
            camera_inertial_velocity: 0.0,
            camera_drag_active: false,
            orbit_pause_started_at: None,
            orbit_pause_accumulated_secs: 0.0,
            camera_last_frame_time: None,
        }
    }
}

impl SkinManagerState {
    fn sync_active_account(&mut self, active_launch_auth: Option<&LaunchAuthContext>) {
        let Some(auth) = active_launch_auth else {
            if self.active_profile_id.is_some() {
                *self = Self::default();
            }
            return;
        };

        let normalized_profile_id = auth.player_uuid.to_ascii_lowercase();
        let changed = self.active_profile_id.as_deref() != Some(normalized_profile_id.as_str());
        if !changed {
            return;
        }

        self.save_in_progress = false;
        self.refresh_in_progress = false;
        self.worker_rx = None;
        self.status_message = None;
        self.show_elytra = false;
        self.active_profile_id = Some(normalized_profile_id.clone());
        self.active_player_name = Some(auth.player_name.clone());
        self.access_token = Some(auth.access_token.clone());
        self.base_skin_png = None;
        self.pending_skin_png = None;
        self.pending_skin_path = None;
        self.pending_variant = MinecraftSkinVariant::Classic;
        self.available_capes.clear();
        self.initial_cape_id = None;
        self.pending_cape_id = None;
        self.skin_texture_hash = None;
        self.skin_texture = None;
        self.skin_sample = None;
        self.cape_texture_hash = None;
        self.cape_texture = None;
        self.cape_sample = None;
        self.preview_texture = None;
        self.cape_uv = default_cape_uv_layout();
        self.camera_yaw_offset = 0.0;
        self.camera_inertial_velocity = 0.0;
        self.camera_drag_active = false;
        self.orbit_pause_started_at = None;
        self.orbit_pause_accumulated_secs = 0.0;
        self.camera_last_frame_time = None;

        self.load_snapshot_from_cache_for_profile(normalized_profile_id.as_str());
        self.start_refresh();
    }

    fn load_snapshot_from_cache_for_profile(&mut self, profile_id: &str) {
        match auth::load_cached_accounts() {
            Ok(accounts) => {
                let profile_id_lower = profile_id.to_ascii_lowercase();
                if let Some(account) = accounts.accounts.iter().find(|account| {
                    account.minecraft_profile.id.to_ascii_lowercase() == profile_id_lower
                }) {
                    self.apply_account_snapshot(account);
                }
            }
            Err(err) => {
                self.status_message = Some(format!("Failed to load account cache: {err}"));
            }
        }
    }

    fn apply_account_snapshot(&mut self, account: &CachedAccount) {
        self.active_profile_id = Some(account.minecraft_profile.id.to_ascii_lowercase());
        self.active_player_name = Some(account.minecraft_profile.name.clone());
        self.access_token = account.minecraft_access_token.clone();

        let active_skin = account
            .minecraft_profile
            .skins
            .iter()
            .find(|skin| skin.state.eq_ignore_ascii_case("active"))
            .or_else(|| account.minecraft_profile.skins.first());
        self.base_skin_png = active_skin.and_then(|skin| skin.texture_png_bytes());
        self.pending_variant = active_skin
            .and_then(|skin| skin.variant.as_deref())
            .map(parse_variant)
            .unwrap_or(MinecraftSkinVariant::Classic);
        self.pending_skin_png = None;
        self.pending_skin_path = None;
        self.skin_texture_hash = None;
        self.skin_texture = None;
        self.skin_sample = None;
        self.cape_texture_hash = None;
        self.cape_texture = None;
        self.cape_sample = None;
        self.preview_texture = None;
        self.cape_uv = default_cape_uv_layout();

        let mut choices = Vec::with_capacity(account.minecraft_profile.capes.len());
        let mut active_cape = None;
        for cape in &account.minecraft_profile.capes {
            if cape.state.eq_ignore_ascii_case("active") {
                active_cape = Some(cape.id.clone());
            }
            let texture_bytes = cape.texture_png_bytes();
            let texture_size = texture_bytes.as_deref().and_then(decode_image_dimensions);
            choices.push(CapeChoice {
                id: cape.id.clone(),
                label: cape
                    .alias
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .unwrap_or(cape.id.as_str())
                    .to_owned(),
                texture_bytes,
                texture_size,
            });
        }

        self.available_capes = choices;
        self.initial_cape_id = active_cape.clone();
        self.pending_cape_id = active_cape;
    }

    fn poll_worker(&mut self, ctx: &egui::Context) {
        if let Some(rx) = self.worker_rx.take() {
            let mut keep_rx = true;
            loop {
                let recv_result = match rx.lock() {
                    Ok(guard) => guard.try_recv(),
                    Err(_) => {
                        self.save_in_progress = false;
                        self.refresh_in_progress = false;
                        self.status_message =
                            Some("Background profile task lock was poisoned.".to_owned());
                        keep_rx = false;
                        break;
                    }
                };
                match recv_result {
                    Ok(WorkerEvent::Refreshed(result)) => {
                        self.refresh_in_progress = false;
                        match result {
                            Ok((profile_id, profile)) => {
                                if self.active_profile_id.as_deref() != Some(profile_id.as_str()) {
                                    keep_rx = false;
                                    break;
                                }
                                self.apply_loaded_profile(profile);
                                self.status_message = Some("Profile refreshed.".to_owned());
                            }
                            Err(err) => {
                                self.status_message = Some(err);
                            }
                        }
                        keep_rx = false;
                    }
                    Ok(WorkerEvent::Saved(result)) => {
                        self.save_in_progress = false;
                        match result {
                            Ok((profile_id, profile)) => {
                                if self.active_profile_id.as_deref() != Some(profile_id.as_str()) {
                                    keep_rx = false;
                                    break;
                                }
                                self.apply_loaded_profile(profile);
                                self.status_message =
                                    Some("Saved skin and cape changes.".to_owned());
                            }
                            Err(err) => {
                                self.status_message = Some(err);
                            }
                        }
                        keep_rx = false;
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        self.save_in_progress = false;
                        self.refresh_in_progress = false;
                        self.status_message =
                            Some("Background profile task stopped unexpectedly.".to_owned());
                        keep_rx = false;
                        break;
                    }
                }
            }
            if keep_rx {
                self.worker_rx = Some(rx);
            } else {
                ctx.request_repaint();
            }
        }
    }

    fn ensure_skin_texture(&mut self, ctx: &egui::Context) {
        let active_png = self.preview_skin_png();
        let Some(bytes) = active_png else {
            self.skin_texture = None;
            self.skin_texture_hash = None;
            self.skin_sample = None;
            return;
        };

        let mut hasher = DefaultHasher::new();
        bytes.hash(&mut hasher);
        let hash = hasher.finish();

        if self.skin_texture_hash == Some(hash) {
            return;
        }

        let Some(image) = decode_skin_rgba(bytes) else {
            self.skin_sample = None;
            self.status_message = Some("Selected skin image could not be decoded.".to_owned());
            return;
        };

        let size = [image.width() as usize, image.height() as usize];
        let color_image = egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw());
        let texture = ctx.load_texture(
            format!("skins/preview/{hash:016x}"),
            color_image,
            TextureOptions::NEAREST,
        );

        self.skin_texture = Some(texture);
        self.skin_sample = Some(image);
        self.skin_texture_hash = Some(hash);
    }

    fn ensure_cape_texture(&mut self, ctx: &egui::Context) {
        let active_png = self.selected_cape_png();
        let Some(bytes) = active_png else {
            self.cape_texture = None;
            self.cape_texture_hash = None;
            self.cape_sample = None;
            self.cape_uv = default_cape_uv_layout();
            return;
        };

        let mut hasher = DefaultHasher::new();
        bytes.hash(&mut hasher);
        let hash = hasher.finish();
        if self.cape_texture_hash == Some(hash) {
            return;
        }

        let Some(image) = decode_generic_rgba(bytes) else {
            self.cape_texture = None;
            self.cape_texture_hash = None;
            self.cape_sample = None;
            self.cape_uv = default_cape_uv_layout();
            return;
        };

        self.cape_uv =
            cape_uv_layout([image.width(), image.height()]).unwrap_or_else(default_cape_uv_layout);

        let size = [image.width() as usize, image.height() as usize];
        let color_image = egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw());
        let texture = ctx.load_texture(
            format!("skins/cape-preview/{hash:016x}"),
            color_image,
            TextureOptions::NEAREST,
        );
        self.cape_texture = Some(texture);
        self.cape_sample = Some(image);
        self.cape_texture_hash = Some(hash);
    }

    fn preview_skin_png(&self) -> Option<&[u8]> {
        self.pending_skin_png
            .as_deref()
            .or(self.base_skin_png.as_deref())
    }

    fn selected_cape_png(&self) -> Option<&[u8]> {
        let selected = self.pending_cape_id.as_deref()?;
        self.available_capes
            .iter()
            .find(|cape| cape.id == selected)
            .and_then(|cape| cape.texture_bytes.as_deref())
    }

    fn pick_skin_file(&mut self) {
        let picked = rfd::FileDialog::new()
            .add_filter("PNG", &["png"])
            .set_title("Select Minecraft Skin")
            .pick_file();

        let Some(path) = picked else {
            return;
        };

        match std::fs::read(path.as_path()) {
            Ok(bytes) => {
                if decode_skin_rgba(&bytes).is_none() {
                    self.status_message = Some(
                        "Selected image must be a valid PNG skin (expected 64x64 or 64x32)."
                            .to_owned(),
                    );
                    return;
                }
                self.pending_skin_png = Some(bytes);
                self.pending_skin_path = Some(path.display().to_string());
                self.skin_texture_hash = None;
                self.skin_sample = None;
                self.preview_texture = None;
                self.status_message = Some("Skin preview updated.".to_owned());
            }
            Err(err) => {
                self.status_message = Some(format!("Failed to read image: {err}"));
            }
        }
    }

    fn can_save(&self) -> bool {
        self.access_token
            .as_deref()
            .map(str::trim)
            .is_some_and(|token| !token.is_empty())
            && (self.pending_skin_png.is_some() || self.pending_cape_id != self.initial_cape_id)
    }

    fn start_refresh(&mut self) {
        if self.refresh_in_progress || self.save_in_progress {
            return;
        }
        let Some(profile_id) = self.active_profile_id.clone() else {
            return;
        };
        let Some(token) = self
            .access_token
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
        else {
            self.status_message =
                Some("Missing Minecraft access token for active account.".to_owned());
            return;
        };

        self.refresh_in_progress = true;
        let (tx, rx) = mpsc::channel();
        self.worker_rx = Some(Arc::new(Mutex::new(rx)));
        let profile_id_for_result = profile_id.clone();
        std::thread::spawn(move || {
            let result = std::panic::catch_unwind(|| fetch_and_cache_profile(profile_id, &token))
                .unwrap_or_else(|_| Err("Skin profile refresh task panicked.".to_owned()));
            let _ = tx.send(WorkerEvent::Refreshed(
                result.map(|loaded| (profile_id_for_result, loaded)),
            ));
        });
    }

    fn start_save(&mut self) {
        if self.save_in_progress || !self.can_save() {
            return;
        }
        let Some(profile_id) = self.active_profile_id.clone() else {
            return;
        };
        let Some(token) = self
            .access_token
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
        else {
            self.status_message =
                Some("Missing Minecraft access token for active account.".to_owned());
            return;
        };

        self.save_in_progress = true;
        let pending_skin = self.pending_skin_png.clone();
        let pending_variant = self.pending_variant;
        let pending_cape = self.pending_cape_id.clone();
        let initial_cape = self.initial_cape_id.clone();

        let (tx, rx) = mpsc::channel();
        self.worker_rx = Some(Arc::new(Mutex::new(rx)));
        let profile_id_for_result = profile_id.clone();

        std::thread::spawn(move || {
            let result = std::panic::catch_unwind(|| -> Result<LoadedProfile, String> {
                if let Some(bytes) = pending_skin.as_deref() {
                    auth::upload_minecraft_skin(&token, bytes, pending_variant)
                        .map_err(|err| format!("Failed to upload skin: {err}"))?;
                }

                if pending_cape != initial_cape {
                    if let Some(cape_id) = pending_cape.as_deref() {
                        auth::set_active_minecraft_cape(&token, cape_id)
                            .map_err(|err| format!("Failed to set cape: {err}"))?;
                    } else {
                        auth::clear_active_minecraft_cape(&token)
                            .map_err(|err| format!("Failed to clear cape: {err}"))?;
                    }
                }

                fetch_and_cache_profile(profile_id, &token)
            })
            .unwrap_or_else(|_| Err("Skin save task panicked.".to_owned()));

            let _ = tx.send(WorkerEvent::Saved(
                result.map(|loaded| (profile_id_for_result, loaded)),
            ));
        });
    }

    fn apply_loaded_profile(&mut self, profile: LoadedProfile) {
        self.active_player_name = Some(profile.player_name);
        self.base_skin_png = profile.active_skin_png;
        self.pending_skin_png = None;
        self.pending_skin_path = None;
        self.pending_variant = profile.skin_variant;
        self.available_capes = profile.capes;
        self.initial_cape_id = profile.active_cape_id.clone();
        self.pending_cape_id = profile.active_cape_id;
        self.skin_texture_hash = None;
        self.skin_sample = None;
        self.cape_texture_hash = None;
        self.cape_sample = None;
        self.preview_texture = None;
        self.cape_uv = default_cape_uv_layout();
    }

    fn begin_manual_camera_control(&mut self, now: f64) {
        if self.orbit_pause_started_at.is_none() {
            self.orbit_pause_started_at = Some(now);
        }
    }

    fn finish_manual_camera_control(&mut self, now: f64) {
        if let Some(started_at) = self.orbit_pause_started_at.take() {
            self.orbit_pause_accumulated_secs += (now - started_at).max(0.0);
        }
    }

    fn effective_orbit_time(&self, now: f64) -> f64 {
        let paused_now = self
            .orbit_pause_started_at
            .map(|started_at| (now - started_at).max(0.0))
            .unwrap_or(0.0);
        (now - self.orbit_pause_accumulated_secs - paused_now).max(0.0)
    }

    fn consume_frame_dt(&mut self, now: f64) -> f32 {
        let dt = self
            .camera_last_frame_time
            .map(|previous| (now - previous).max(0.0) as f32)
            .unwrap_or(0.0);
        self.camera_last_frame_time = Some(now);
        dt
    }
}

#[derive(Clone, Debug)]
struct LoadedProfile {
    player_name: String,
    active_skin_png: Option<Vec<u8>>,
    skin_variant: MinecraftSkinVariant,
    capes: Vec<CapeChoice>,
    active_cape_id: Option<String>,
}

#[derive(Clone, Debug)]
enum WorkerEvent {
    Refreshed(Result<(String, LoadedProfile), String>),
    Saved(Result<(String, LoadedProfile), String>),
}

fn fetch_and_cache_profile(
    profile_id: String,
    access_token: &str,
) -> Result<LoadedProfile, String> {
    let profile = auth::fetch_minecraft_profile(access_token)
        .map_err(|err| format!("Failed to fetch latest profile: {err}"))?;
    update_cached_profile(profile_id.as_str(), &profile)?;
    Ok(LoadedProfile::from_profile(profile))
}

fn update_cached_profile(profile_id: &str, profile: &MinecraftProfileState) -> Result<(), String> {
    let mut cache =
        auth::load_cached_accounts().map_err(|err| format!("Cache read failed: {err}"))?;
    let mut changed = false;

    for account in &mut cache.accounts {
        if account.minecraft_profile.id == profile_id {
            account.minecraft_profile = profile.clone();
            account.cached_at_unix_secs = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            changed = true;
            break;
        }
    }

    if changed {
        auth::save_cached_accounts(&cache).map_err(|err| format!("Cache write failed: {err}"))?;
    }

    Ok(())
}

impl LoadedProfile {
    fn from_profile(profile: MinecraftProfileState) -> Self {
        let active_skin = profile
            .skins
            .iter()
            .find(|skin| skin.state.eq_ignore_ascii_case("active"))
            .or_else(|| profile.skins.first());

        let active_skin_png = active_skin.and_then(|skin| skin.texture_png_bytes());
        let skin_variant = active_skin
            .and_then(|skin| skin.variant.as_deref())
            .map(parse_variant)
            .unwrap_or(MinecraftSkinVariant::Classic);

        let mut active_cape_id = None;
        let mut capes = Vec::with_capacity(profile.capes.len());
        for cape in profile.capes {
            let texture_bytes = cape.texture_png_bytes();
            let texture_size = texture_bytes.as_deref().and_then(decode_image_dimensions);
            if cape.state.eq_ignore_ascii_case("active") {
                active_cape_id = Some(cape.id.clone());
            }
            capes.push(CapeChoice {
                label: cape
                    .alias
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .unwrap_or(cape.id.as_str())
                    .to_owned(),
                id: cape.id,
                texture_bytes,
                texture_size,
            });
        }

        Self {
            player_name: profile.name,
            active_skin_png,
            skin_variant,
            capes,
            active_cape_id,
        }
    }
}

fn parse_variant(raw: &str) -> MinecraftSkinVariant {
    if raw.eq_ignore_ascii_case("slim") {
        MinecraftSkinVariant::Slim
    } else {
        MinecraftSkinVariant::Classic
    }
}

fn decode_skin_rgba(bytes: &[u8]) -> Option<RgbaImage> {
    let image = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (w, h) = image.dimensions();
    if w == 64 && (h == 64 || h == 32) {
        Some(image)
    } else {
        None
    }
}

fn decode_generic_rgba(bytes: &[u8]) -> Option<RgbaImage> {
    image::load_from_memory(bytes)
        .ok()
        .map(|image| image.to_rgba8())
}

fn decode_image_dimensions(bytes: &[u8]) -> Option<[u32; 2]> {
    let image = image::load_from_memory(bytes).ok()?;
    Some([image.width(), image.height()])
}

fn full_uv_rect() -> Rect {
    Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0))
}

fn full_face_uvs() -> FaceUvs {
    let full = full_uv_rect();
    FaceUvs {
        top: full,
        bottom: full,
        left: full,
        right: full,
        front: full,
        back: full,
    }
}

fn default_cape_uv_layout() -> FaceUvs {
    cape_uv_layout([64, 32]).unwrap_or_else(full_face_uvs)
}

fn cape_outer_face_uv(texture_size: [u32; 2]) -> Option<Rect> {
    if texture_size[0] < 22 || texture_size[1] < 17 {
        return None;
    }
    Some(uv_rect_with_inset(texture_size, 1, 1, 10, 16))
}

fn cape_uv_layout(texture_size: [u32; 2]) -> Option<FaceUvs> {
    let outer = cape_outer_face_uv(texture_size)?;
    let inner = uv_rect_with_inset(texture_size, 12, 1, 10, 16);
    Some(FaceUvs {
        top: uv_rect_with_inset(texture_size, 1, 0, 10, 1),
        bottom: uv_rect_with_inset(texture_size, 11, 0, 10, 1),
        left: uv_rect_with_inset(texture_size, 0, 1, 1, 16),
        right: uv_rect_with_inset(texture_size, 11, 1, 1, 16),
        // Cape sits behind the torso in our coordinate space, so the cuboid "back" face is the
        // outward-facing panel visible from behind the player.
        front: inner,
        back: outer,
    })
}

fn neutral_button_style(ui: &Ui) -> ButtonOptions {
    ButtonOptions {
        min_size: egui::vec2(160.0, style::CONTROL_HEIGHT),
        text_color: ui.visuals().text_color(),
        fill: ui.visuals().widgets.inactive.bg_fill,
        fill_hovered: ui.visuals().widgets.hovered.bg_fill,
        fill_active: ui.visuals().widgets.active.bg_fill,
        fill_selected: ui.visuals().selection.bg_fill,
        stroke: ui.visuals().widgets.inactive.bg_stroke,
        ..ButtonOptions::default()
    }
}

fn _absolute_path_string(path: &PathBuf) -> String {
    path.display().to_string()
}
