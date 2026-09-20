//! The "Default: ..." text in the launcher settings tooltips must match what the config and text
//! renderer really default to, so a changed default can't leave a stale tooltip behind.

use config::{Config, FloatSettingId, IntSettingId, TextSettingId};

fn tooltip(text: Option<&'static str>) -> &'static str {
    text.expect("setting has a tooltip")
}

#[test]
fn toggle_tooltips_state_the_real_default() {
    let mut config = Config::default();
    config.for_each_toggle_mut(|spec, value| {
        let expected = format!("Default: {}.", if *value { "On" } else { "Off" });
        assert!(
            tooltip(spec.info_tooltip).ends_with(&expected),
            "{} tooltip should end with {expected:?}: {:?}",
            spec.label,
            spec.info_tooltip
        );
    });
}

#[test]
fn numeric_tooltips_state_the_real_default() {
    let config = Config::default();
    let cases: [(&str, String); 7] = [
        (
            tooltip(FloatSettingId::UiFontSize.spec().info_tooltip),
            format!("Default: {}.", config.ui_font_size()),
        ),
        (
            tooltip(
                FloatSettingId::SkinPreviewMotionBlurAmount
                    .spec()
                    .info_tooltip,
            ),
            format!("Default: {}.", config.skin_preview_motion_blur_amount()),
        ),
        (
            tooltip(
                FloatSettingId::SkinPreviewMotionBlurShutterFrames
                    .spec()
                    .info_tooltip,
            ),
            format!(
                "Default: {}.",
                config.skin_preview_motion_blur_shutter_frames()
            ),
        ),
        (
            tooltip(IntSettingId::UiFontWeight.spec().info_tooltip),
            format!("Default: {}.", config.ui_font_weight()),
        ),
        (
            tooltip(IntSettingId::FrameLimitFps.spec().info_tooltip),
            format!("Default: {}.", config.frame_limit_fps()),
        ),
        (
            tooltip(IntSettingId::SkinPreviewMsaaSamples.spec().info_tooltip),
            format!("Default: {}.", config.skin_preview_msaa_samples()),
        ),
        (
            tooltip(
                IntSettingId::SkinPreviewMotionBlurSampleCount
                    .spec()
                    .info_tooltip,
            ),
            format!(
                "Default: {}.",
                config.skin_preview_motion_blur_sample_count()
            ),
        ),
    ];
    for (tip, expected) in cases {
        assert!(
            tip.ends_with(&expected),
            "{tip:?} should end with {expected:?}"
        );
    }
}

#[test]
fn open_type_tooltips_list_the_real_default_features() {
    let defaults = textui::DEFAULT_OPEN_TYPE_FEATURE_TAGS;
    let list = tooltip(TextSettingId::OpenTypeFeaturesToEnable.spec().info_tooltip);
    assert!(
        list.contains(defaults),
        "{list:?} should mention {defaults:?}"
    );
    let toggle = tooltip(
        config::ToggleSettingId::OpenTypeFeaturesEnabled
            .spec()
            .info_tooltip,
    );
    assert!(
        toggle.contains(defaults),
        "{toggle:?} should mention {defaults:?}"
    );
    assert_eq!(Config::default().open_type_features_to_enable(), "");
}
