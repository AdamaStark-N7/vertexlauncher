use super::*;

pub(super) fn render_graphics_adapter_settings(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    config: &mut Config,
    settings_info: &SettingsInfo,
) {
    let type_setting = DropdownSettingId::GraphicsAdapterPreferenceType.spec();
    let type_options = GraphicsAdapterPreferenceType::ALL
        .iter()
        .map(|option| option.settings_label())
        .collect::<Vec<_>>();
    let mut selected_type_index = GraphicsAdapterPreferenceType::ALL
        .iter()
        .position(|option| *option == config.graphics_adapter_preference_type())
        .unwrap_or(0);
    let type_response = settings_widgets::dropdown_row(
        text_ui,
        ui,
        "graphics_adapter_preference_type",
        type_setting.label,
        type_setting.info_tooltip,
        &mut selected_type_index,
        &type_options,
    );
    if type_response.changed() {
        if let Some(next_type) = GraphicsAdapterPreferenceType::ALL.get(selected_type_index) {
            config.set_graphics_adapter_preference_type(*next_type);
        }
    }
    ui.add_space(style::SPACE_MD);

    let preference_setting = DropdownSettingId::GraphicsAdapterPreference.spec();
    match config.graphics_adapter_preference_type() {
        GraphicsAdapterPreferenceType::PerformanceProfile => {
            let profile_options = GraphicsAdapterProfile::ALL
                .iter()
                .map(|option| option.settings_label())
                .collect::<Vec<_>>();
            let mut selected_profile_index = GraphicsAdapterProfile::ALL
                .iter()
                .position(|option| *option == config.graphics_adapter_profile())
                .unwrap_or(0);
            let response = settings_widgets::dropdown_row(
                text_ui,
                ui,
                "graphics_adapter_profile",
                preference_setting.label,
                preference_setting.info_tooltip,
                &mut selected_profile_index,
                &profile_options,
            );
            if response.changed() {
                if let Some(next_profile) = GraphicsAdapterProfile::ALL.get(selected_profile_index)
                {
                    config.set_graphics_adapter_profile(*next_profile);
                }
            }
        }
        GraphicsAdapterPreferenceType::ExplicitAdapter => {
            let mut labels = settings_info
                .available_graphics_adapters
                .iter()
                .map(|adapter| adapter.label.as_str())
                .collect::<Vec<_>>();
            let explicit_hash = config.graphics_adapter_explicit_hash();
            let mut missing_label = None::<String>;
            let mut selected_index = settings_info
                .available_graphics_adapters
                .iter()
                .position(|adapter| Some(adapter.hash) == explicit_hash)
                .unwrap_or_else(|| {
                    if let Some(hash) = explicit_hash {
                        let _ = hash;
                        missing_label =
                            Some("Missing adapter (falls back to High Performance)".to_owned());
                        labels.insert(0, missing_label.as_ref().unwrap().as_str());
                        0
                    } else {
                        0
                    }
                });

            if labels.is_empty() {
                labels.push("No graphics adapters detected");
                selected_index = 0;
            }

            let response = settings_widgets::dropdown_row(
                text_ui,
                ui,
                "graphics_adapter_explicit",
                preference_setting.label,
                preference_setting.info_tooltip,
                &mut selected_index,
                &labels,
            );
            if response.changed() {
                if missing_label.is_some() && selected_index == 0 {
                    config.set_graphics_adapter_explicit_hash(explicit_hash);
                } else {
                    let actual_index =
                        selected_index.saturating_sub(usize::from(missing_label.is_some()));
                    let selected_hash = settings_info
                        .available_graphics_adapters
                        .get(actual_index)
                        .map(|adapter| adapter.hash);
                    config.set_graphics_adapter_explicit_hash(selected_hash);
                }
            }
        }
    }
    ui.add_space(style::SPACE_MD);
}

pub(super) fn supported_graphics_api_preferences(config: &Config) -> Vec<GraphicsApiPreference> {
    let transparent_viewport = config.window_transparency().is_translucent();

    GraphicsApiPreference::ALL
        .into_iter()
        .filter(|preference| match preference {
            GraphicsApiPreference::Auto => true,
            GraphicsApiPreference::Metal => cfg!(target_os = "macos"),
            GraphicsApiPreference::Dx12 => cfg!(target_os = "windows"),
            GraphicsApiPreference::Vulkan => {
                if cfg!(target_os = "windows") {
                    !transparent_viewport
                } else {
                    !cfg!(target_os = "macos")
                }
            }
        })
        .collect()
}

pub(super) fn render_graphics_api_setting(ui: &mut Ui, text_ui: &mut TextUi, config: &mut Config) {
    let setting = DropdownSettingId::GraphicsApiPreference.spec();
    let options = supported_graphics_api_preferences(config);
    if !options.contains(&config.graphics_api_preference()) {
        let fallback = if options.contains(&GraphicsApiPreference::Dx12) {
            GraphicsApiPreference::Dx12
        } else {
            GraphicsApiPreference::Auto
        };
        config.set_graphics_api_preference(fallback);
    }
    let labels = options
        .iter()
        .map(|option| option.settings_label())
        .collect::<Vec<_>>();
    let mut selected_index = options
        .iter()
        .position(|option| *option == config.graphics_api_preference())
        .unwrap_or(0);
    let response = settings_widgets::dropdown_row(
        text_ui,
        ui,
        "graphics_api_preference",
        setting.label,
        setting.info_tooltip,
        &mut selected_index,
        &labels,
    );
    if response.changed() {
        if let Some(next_preference) = options.get(selected_index) {
            config.set_graphics_api_preference(*next_preference);
        }
    }
    ui.add_space(style::SPACE_MD);
}

pub(super) fn render_svg_aa_setting(ui: &mut Ui, text_ui: &mut TextUi, config: &mut Config) {
    let mut selected = SvgAaMode::ALL
        .iter()
        .position(|mode| *mode == config.svg_aa_mode())
        .unwrap_or(0);
    let labels: Vec<&str> = SvgAaMode::ALL.iter().map(|mode| mode.label()).collect();

    let response = settings_widgets::dropdown_row(
        text_ui,
        ui,
        "svg_aa_mode",
        "SVG Anti-Aliasing",
        Some(
            "Controls supersampled SVG rasterization for launcher icons. Changes apply immediately. Default: Balanced (SSAA 2x).",
        ),
        &mut selected,
        &labels,
    );

    if response.changed() {
        if let Some(next) = SvgAaMode::ALL.get(selected).copied() {
            config.set_svg_aa_mode(next);
        }
    }
}

pub(super) fn render_skin_preview_setting(ui: &mut Ui, text_ui: &mut TextUi, config: &mut Config) {
    let mut texel_selected = SkinPreviewTexelAaMode::ALL
        .iter()
        .position(|mode| *mode == config.skin_preview_texel_aa_mode())
        .unwrap_or(0);
    let texel_labels: Vec<&str> = SkinPreviewTexelAaMode::ALL
        .iter()
        .map(|mode| mode.label())
        .collect();
    let texel_response = settings_widgets::dropdown_row(
        text_ui,
        ui,
        "skins_preview_texel_aa_mode",
        "Skin Preview Texel Edge AA",
        Some(
            "Smooths the hard pixel edges between skin texels inside the skin shader, on top of the post anti-aliasing below. Off keeps crisp pixel-art edges. Default: Off.",
        ),
        &mut texel_selected,
        &texel_labels,
    );
    if texel_response.changed() {
        if let Some(next) = SkinPreviewTexelAaMode::ALL.get(texel_selected).copied() {
            config.set_skin_preview_texel_aa_mode(next);
        }
    }

    ui.add_space(style::SPACE_MD);

    let mut selected = SkinPreviewAaMode::ALL
        .iter()
        .position(|mode| *mode == config.skin_preview_aa_mode())
        .unwrap_or(0);
    let labels: Vec<&str> = SkinPreviewAaMode::ALL
        .iter()
        .map(|mode| mode.label())
        .collect();
    let response = settings_widgets::dropdown_row(
        text_ui,
        ui,
        "skins_preview_aa_mode",
        "Skin Preview Post Anti-Aliasing",
        Some(
            "How the skin preview is smoothed. MSAA samples the scene itself (see MSAA Samples); SMAA and FXAA are post-process filters; TAA blends frames over time; Off is fastest. Changes apply immediately. Default: FXAA (Post).",
        ),
        &mut selected,
        &labels,
    );
    if response.changed() {
        if let Some(next) = SkinPreviewAaMode::ALL.get(selected).copied() {
            config.set_skin_preview_aa_mode(next);
        }
    }

    ui.add_space(style::SPACE_MD);
}

pub(super) fn render_skin_preview_motion_blur_settings(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    config: &mut Config,
) {
    let mut enabled = config.skin_preview_motion_blur_enabled();
    let response = settings_widgets::toggle_row(
        text_ui,
        ui,
        "Enable Skin Preview Motion Blur",
        Some(
            "Blurs the moving 3D skin preview by blending several moments across the shutter interval. Applies immediately. Default: Off.",
        ),
        &mut enabled,
    );
    if response.changed() {
        config.set_skin_preview_motion_blur_enabled(enabled);
    }
    ui.add_space(style::SPACE_MD);

    if !config.skin_preview_motion_blur_enabled() {
        return;
    }

    let spec = FloatSettingId::SkinPreviewMotionBlurAmount.spec();
    let mut amount = config.skin_preview_motion_blur_amount();

    let response = settings_widgets::float_slider_row(
        text_ui,
        ui,
        spec.id,
        spec.label,
        Some(
            "Longer shutter intervals produce stronger blur but more trailing. Best paired with SMAA, FXAA, or TAA.",
        ),
        &mut amount,
        spec.min,
        spec.max,
        true,
    );

    if response.changed()
        || (amount - config.skin_preview_motion_blur_amount()).abs() > f32::EPSILON
    {
        config.set_skin_preview_motion_blur_amount(amount);
    }
    ui.add_space(style::SPACE_MD);

    let shutter_spec = FloatSettingId::SkinPreviewMotionBlurShutterFrames.spec();
    let mut shutter_frames = config.skin_preview_motion_blur_shutter_frames();
    let shutter_response = settings_widgets::float_stepper_row(
        text_ui,
        ui,
        shutter_spec.id,
        shutter_spec.label,
        Some(
            "Total shutter interval measured in 60 FPS frames. Higher values create longer streaks.",
        ),
        &mut shutter_frames,
        shutter_spec.min,
        shutter_spec.max,
        shutter_spec.step,
    );
    if shutter_response.changed()
        || (shutter_frames - config.skin_preview_motion_blur_shutter_frames()).abs() > f32::EPSILON
    {
        config.set_skin_preview_motion_blur_shutter_frames(shutter_frames);
    }
    ui.add_space(style::SPACE_MD);

    let sample_spec = IntSettingId::SkinPreviewMotionBlurSampleCount.spec();
    let mut sample_count = config.skin_preview_motion_blur_sample_count();
    let sample_response = settings_widgets::int_stepper_row(
        text_ui,
        ui,
        sample_spec.id,
        sample_spec.label,
        Some(
            "How many snapshots of the skin preview are blended across the shutter interval (2\u{2013}16). More samples make smoother blur and cost more GPU work. Default: 5.",
        ),
        &mut sample_count,
        sample_spec.min,
        sample_spec.max,
        sample_spec.step,
    );
    if sample_response.changed() || sample_count != config.skin_preview_motion_blur_sample_count() {
        config.set_skin_preview_motion_blur_sample_count(sample_count);
    }
    ui.add_space(style::SPACE_MD);
}

pub(super) fn render_download_settings(ui: &mut Ui, text_ui: &mut TextUi, config: &mut Config) {
    let concurrency_spec = IntSettingId::FrameLimitFps; // placeholder to satisfy formatting block removal
    let _ = concurrency_spec;

    let mut max_concurrent = config.download_max_concurrent() as i32;
    let response = settings_widgets::int_stepper_row(
        text_ui,
        ui,
        "download_max_concurrent",
        "Max Concurrent Downloads",
        Some(
            "How many files the launcher downloads at the same time (1\u{2013}128) for game files, libraries and mods. Higher can be faster on fast connections but may trigger rate limits from download servers. Default: 8.",
        ),
        &mut max_concurrent,
        DOWNLOAD_CONCURRENCY_MIN as i32,
        DOWNLOAD_CONCURRENCY_MAX as i32,
        1,
    );
    if response.changed() {
        config.set_download_max_concurrent(max_concurrent.max(1) as u32);
    }
    ui.add_space(style::SPACE_MD);

    let mut speed_limit_enabled = config.download_speed_limit_enabled();
    settings_widgets::toggle_row(
        text_ui,
        ui,
        "Enable Download Speed Limit",
        Some(
            "Limits how fast the launcher downloads, using the value below. If the value is empty or not valid, no limit is applied. Default: Off.",
        ),
        &mut speed_limit_enabled,
    );
    config.set_download_speed_limit_enabled(speed_limit_enabled);
    ui.add_space(style::SPACE_MD);

    let _ = settings_widgets::text_input_row(
        text_ui,
        ui,
        "download_speed_limit",
        "Download Speed Limit",
        Some(
            "A number plus a unit in bits per second: Kbps, Mbps, Gbps or Tbps, for example 250Mbps. Decimals are allowed. Note this is bits, so 80Mbps is about 10 MB/s. A missing or unknown unit means no limit. Default: empty (no limit).",
        ),
        config.download_speed_limit_mut(),
    );
    ui.add_space(style::SPACE_MD);
}
