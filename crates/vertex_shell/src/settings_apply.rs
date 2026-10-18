//! Live settings: turns a [`Config`] change into engine calls, one applier per [`SettingsDomain`].
//!
//! Nothing here needs a relaunch. The pure `*_request` / `*_settings` functions map config values onto engine request
//! types and are unit-tested without a GPU; [`Appliers`] runs them against a live [`Engine`].

use config::{
    Config, GraphicsAdapterPreferenceType, GraphicsAdapterProfile, GraphicsApiPreference, SettingsDiff, SettingsDomain,
    WindowTransparency,
};
use sturdy::device::{Backend, DevicePreference, DeviceRequest, SwitchOutcome};
use sturdy::render::{FrameRateLimit, LinuxBlurProtocol, PresentationSettings, SurfaceHandle, WindowEffectKind, WindowEffectParams};
use sturdy::Engine;

/// What an applier achieved. `Failed` means the previous value is still in effect and the caller should restore it in
/// its `Config` and tell the user.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Applied {
    Ok,
    /// Applied, but not as asked (for example blur requested on a compositor without blur support).
    Degraded(String),
    Failed(String),
}

impl Applied {
    pub fn is_failure(&self) -> bool {
        matches!(self, Applied::Failed(_))
    }
}

/// Stable 64-bit id for an engine adapter, stored in `Config::graphics_adapter_explicit_hash`.
///
/// Configs saved by the egui build hold a different hash, so those never match and the engine falls back to the
/// performance profile until the user picks the adapter again.
pub fn adapter_hash(stable_id: &str) -> u64 {
    // FNV-1a: tiny, deterministic across platforms and releases (unlike `DefaultHasher`).
    stable_id.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, byte| (hash ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3))
}

pub fn backend_for(preference: GraphicsApiPreference) -> Option<Backend> {
    match preference {
        GraphicsApiPreference::Auto => None,
        GraphicsApiPreference::Vulkan => Some(Backend::Vulkan),
        GraphicsApiPreference::Dx12 => Some(Backend::D3D12),
        // macOS is WebGPU-only on purpose; the egui build's "Metal" choice maps to it.
        GraphicsApiPreference::Metal | GraphicsApiPreference::WebGpu => Some(Backend::WebGpu),
    }
}

/// The API Auto tries first on this platform; `None` where the engine's own choice is right (macOS is WebGPU-only).
pub fn native_backend() -> Option<Backend> {
    if cfg!(target_os = "windows") {
        Some(Backend::D3D12)
    } else if cfg!(target_os = "macos") {
        None
    } else {
        Some(Backend::Vulkan)
    }
}

pub fn device_preference_for(profile: GraphicsAdapterProfile) -> DevicePreference {
    match profile {
        GraphicsAdapterProfile::Default => DevicePreference::Default,
        GraphicsAdapterProfile::HighPerformance => DevicePreference::HighPerformance,
        GraphicsAdapterProfile::LowPower => DevicePreference::LowPower,
        GraphicsAdapterProfile::DiscreteOnly => DevicePreference::DiscreteOnly,
        GraphicsAdapterProfile::IntegratedOnly => DevicePreference::IntegratedOnly,
    }
}

/// `explicit_stable_id` is the engine adapter whose [`adapter_hash`] equals the saved hash, if one is still present.
pub fn device_request(config: &Config, explicit_stable_id: Option<String>) -> DeviceRequest {
    let mut request = DeviceRequest::new();
    if let Some(backend) = backend_for(config.graphics_api_preference()) {
        request = request.backend(backend);
    }
    match config.graphics_adapter_preference_type() {
        GraphicsAdapterPreferenceType::PerformanceProfile => {
            request.preference(device_preference_for(config.graphics_adapter_profile()))
        }
        GraphicsAdapterPreferenceType::ExplicitAdapter => {
            // An unknown adapter falls back to the High Performance profile, as the setting's help text promises.
            let request = request.preference(DevicePreference::HighPerformance);
            match explicit_stable_id {
                Some(id) => request.adapter(id),
                None => request,
            }
        }
    }
}

pub fn presentation_settings(config: &Config, base: PresentationSettings) -> PresentationSettings {
    let limit = if config.frame_limiter_enabled() && config.frame_limit_fps() > 0 {
        FrameRateLimit::Fps(f64::from(config.frame_limit_fps()))
    } else {
        FrameRateLimit::Unlimited
    };
    // Immediate mode: frames are drawn as fast as the GPU allows, so vsync stays off and the limiter (when set) is
    // the only thing pacing them.
    PresentationSettings {
        hdr_enabled: config.hdr_when_available(),
        hdr_color_space: hdr_color_space(config.hdr_color_space()),
        frame_rate_limit: limit,
        vsync: sturdy::VSync::Off,
        ..base
    }
}

pub fn hdr_color_space(space: config::HdrColorSpace) -> sturdy::render::HdrColorSpace {
    match space {
        config::HdrColorSpace::Hdr10St2084 => sturdy::render::HdrColorSpace::Hdr10St2084,
        config::HdrColorSpace::ScrgbLinear => sturdy::render::HdrColorSpace::ScrgbLinear,
        config::HdrColorSpace::Hdr10Hlg => sturdy::render::HdrColorSpace::Hdr10Hlg,
        config::HdrColorSpace::DolbyVision => sturdy::render::HdrColorSpace::DolbyVision,
    }
}

pub fn linux_blur_protocol(protocol: config::LinuxBlurProtocol) -> LinuxBlurProtocol {
    match protocol {
        config::LinuxBlurProtocol::Auto => LinuxBlurProtocol::Automatic,
        config::LinuxBlurProtocol::ExtBackgroundEffect => LinuxBlurProtocol::ExtBackgroundEffect,
        config::LinuxBlurProtocol::KdeBlur => LinuxBlurProtocol::KdeBlur,
    }
}

/// The window-effect kind that implements the user's blur choice on this platform.
pub fn blur_effect_kind(config: &Config) -> WindowEffectKind {
    if cfg!(target_os = "windows") {
        use config::WindowsBackdropType as B;
        match config.windows_backdrop_type() {
            B::Auto => WindowEffectKind::BackdropAuto,
            B::None => WindowEffectKind::BackdropNone,
            B::Mica => WindowEffectKind::Mica,
            B::Acrylic => WindowEffectKind::Acrylic,
            B::MicaAlt => WindowEffectKind::MicaAlt,
            B::LegacyBlur => WindowEffectKind::AccentBlur,
        }
    } else if cfg!(target_os = "macos") {
        WindowEffectKind::VisualEffect
    } else {
        WindowEffectKind::Blur
    }
}

/// Owns the surface and the last-applied state, and applies config changes to a running engine.
pub struct Appliers {
    surface: SurfaceHandle,
    active_blur: Option<WindowEffectKind>,
    blur_radius: f32,
    /// Linux blur protocols still worth trying if the compositor refuses the one in use, in order.
    blur_fallbacks: Vec<LinuxBlurProtocol>,
    /// Why the requested protocol was refused, kept to explain a later fallback or final failure.
    blur_refusals: Vec<String>,
    /// MaxCLL/MaxFALL still to send to the display, with the attempts made so far.
    pending_light_levels: Option<(f32, f32, u8)>,
}

impl Appliers {
    pub fn new(window_id: u64) -> Self {
        Self { surface: SurfaceHandle::from_window_id(window_id), active_blur: None, blur_radius: 0.0, blur_fallbacks: Vec::new(), blur_refusals: Vec::new(), pending_light_levels: None }
    }

    /// Applies every domain in `diff`. Returns the failures and degradations, which the caller turns into
    /// notifications and, for failures, a rollback of the matching `Config` fields.
    pub fn apply(&mut self, engine: &mut Engine<'_>, new: &Config, diff: SettingsDiff) -> Vec<(SettingsDomain, Applied)> {
        let mut results = Vec::new();
        for domain in diff.domains() {
            let result = match domain {
                SettingsDomain::GraphicsDevice => self.graphics_device(engine, new),
                SettingsDomain::WindowEffect => self.window_effect(engine, new),
                SettingsDomain::Presentation => self.presentation(engine, new),
                // The rest are applied by Vertex-side code that does not need the engine handle.
                _ => continue,
            };
            if result != Applied::Ok {
                results.push((domain, result));
            }
        }
        results
    }

    fn graphics_device(&mut self, engine: &mut Engine<'_>, config: &Config) -> Applied {
        let explicit = config.graphics_adapter_explicit_hash().and_then(|hash| {
            engine.available_adapters().into_iter().find(|adapter| adapter_hash(&adapter.stable_id) == hash).map(|adapter| adapter.stable_id)
        });
        let mut request = device_request(config, explicit.clone());
        // Auto means the platform's native API first, with WebGPU only as the way in where neither it nor the other
        // native API can run: the engine's own default would otherwise be free to pick WebGPU (Dawn) when it is built in.
        let native = if config.graphics_api_preference() == GraphicsApiPreference::Auto { native_backend() } else { None };
        if let Some(native) = native {
            request = request.backend(native);
        }
        let mut switch = engine.switch_device(&request);
        if native.is_some() && matches!(switch.outcome, SwitchOutcome::Failed(_)) {
            switch = engine.switch_device(&device_request(config, explicit));
        }
        match switch.outcome {
            SwitchOutcome::Switched => Applied::Ok,
            SwitchOutcome::SwitchedWithFallback(reason) => Applied::Degraded(reason),
            SwitchOutcome::Failed(reason) => Applied::Failed(reason),
        }
    }

    /// Blur parameters with the blurred region rounded like the window's own corners, so the backdrop does not show
    /// (or blur) outside them.
    fn blur_params(&self, protocol: LinuxBlurProtocol) -> WindowEffectParams {
        let mut params = WindowEffectParams { protocol, ..Default::default() };
        params.options.corner_radius = self.blur_radius;
        params
    }

    fn window_effect(&mut self, engine: &mut Engine<'_>, config: &Config) -> Applied {
        let translucent = config.window_transparency().is_translucent();
        // The window draws its own (configurable) corners whenever it is translucent, or opaque with a corner radius
        // set; either way it needs an alpha-capable surface for the area outside the corners. A radius of 0 on an
        // opaque window keeps the OS's own look.
        let own_corners = translucent || config.window_corner_radius() > 0;
        // Windows 11 rounds window frames itself, so switch the OS rounding off when the window draws its own or the
        // two would fight.
        if cfg!(windows) {
            use sturdy::render::{WindowCornerPreference, WindowEffectOptions};
            let mut params = WindowEffectParams::default();
            params.options = WindowEffectOptions {
                corner_preference: if own_corners { WindowCornerPreference::DoNotRound } else { WindowCornerPreference::Default },
                ..WindowEffectOptions::default()
            };
            engine.set_window_effect_with(self.surface, WindowEffectKind::CornerPreference, true, &params);
        }
        engine.set_transparent(self.surface, own_corners);
        // Start from the window's live settings so only composition changes.
        let presentation = PresentationSettings { transparent_composition: own_corners, ..engine.presentation_settings(self.surface) };
        if let Err(err) = engine.set_presentation_settings(self.surface, presentation) {
            return Applied::Failed(err.to_string());
        }

        if let Some(kind) = self.active_blur.take() {
            engine.set_window_effect(self.surface, kind, false);
        }
        if config.window_transparency() != WindowTransparency::Blurred {
            return Applied::Ok;
        }
        let kind = blur_effect_kind(config);
        let protocol = linux_blur_protocol(config.linux_blur_protocol());
        // If the compositor refuses the chosen protocol, try the others rather than failing the whole setting.
        self.blur_fallbacks = [LinuxBlurProtocol::ExtBackgroundEffect, LinuxBlurProtocol::KdeBlur, LinuxBlurProtocol::Automatic]
            .into_iter()
            .filter(|candidate| *candidate != protocol)
            .collect();
        self.blur_refusals.clear();
        self.blur_radius = f32::from(config.window_corner_radius());
        let params = self.blur_params(protocol);
        engine.set_window_effect_with(self.surface, kind, true, &params);
        self.active_blur = Some(kind);
        // The platform reports its verdict a frame later (`window_effect_status`); `poll_effect_status` reads it.
        Applied::Ok
    }

    /// Reads the verdict for the blur requested by the last `WindowEffect` apply. Call a frame or two after it.
    ///
    /// A refused blur never fails the setting (which would roll it back): other Linux protocols are tried first, and
    /// if none work the window stays translucent and the verdict is a `Degraded` explaining why.
    pub fn poll_effect_status(&mut self, engine: &mut Engine<'_>) -> Option<Applied> {
        use sturdy::render::WindowEffectOutcome;
        let kind = self.active_blur?;
        let status = engine.window_effect_status(self.surface, kind)?;
        if !status.requested_enabled {
            // A stale verdict for the disable that preceded this request.
            return None;
        }
        Some(match status.outcome {
            WindowEffectOutcome::Applied => Applied::Ok,
            WindowEffectOutcome::Degraded => Applied::Degraded(status.details),
            _ => {
                self.blur_refusals.push(status.details);
                if cfg!(target_os = "linux") && !self.blur_fallbacks.is_empty() {
                    let protocol = self.blur_fallbacks.remove(0);
                    let params = self.blur_params(protocol);
                    engine.set_window_effect_with(self.surface, kind, true, &params);
                    return None;
                }
                let reasons: Vec<&str> = self.blur_refusals.iter().map(String::as_str).filter(|reason| !reason.is_empty()).collect();
                let reasons = if reasons.is_empty() { "the compositor did not accept the blur request".to_owned() } else { reasons.join("; ") };
                Applied::Degraded(format!("Blur is unavailable ({reasons}); using plain transparency."))
            }
        })
    }

    /// Sends the pending HDR light levels once the swapchain can take them. Only an HDR10 (PQ) swapchain carries this
    /// metadata, and only where the driver supports it, so running out of attempts is not an error worth a notice.
    /// Returns true while another attempt is wanted.
    pub fn poll_hdr_metadata(&mut self, engine: &mut Engine<'_>) -> bool {
        const ATTEMPTS: u8 = 20;
        let Some((cll, fall, tries)) = self.pending_light_levels else { return false };
        match engine.update_hdr_content_light_level(self.surface, cll, fall) {
            Ok(()) => {
                self.pending_light_levels = None;
                false
            }
            Err(err) if tries + 1 >= ATTEMPTS => {
                tracing::debug!(target: "vertex_shell", "HDR light levels not applied: {err}");
                self.pending_light_levels = None;
                false
            }
            Err(_) => {
                self.pending_light_levels = Some((cll, fall, tries + 1));
                true
            }
        }
    }

    fn presentation(&mut self, engine: &mut Engine<'_>, config: &Config) -> Applied {
        let presentation = presentation_settings(config, engine.presentation_settings(self.surface));
        if let Err(err) = engine.set_presentation_settings(self.surface, presentation) {
            return Applied::Failed(err.to_string());
        }
        // The UI brightness applies at once. The light levels need the HDR swapchain to exist, which the engine
        // recreates a frame or so after the change, so they are sent by `poll_hdr_metadata` instead.
        sturdy::ui::set_hdr_reference_white(config.hdr_ui_white_scale());
        self.pending_light_levels = (config.hdr_when_available() && config.hdr_color_space() == config::HdrColorSpace::Hdr10St2084)
            .then(|| (config.hdr_max_cll_nits() as f32, (config.hdr_max_fall_nits() as f32).min(config.hdr_max_cll_nits() as f32), 0));
        Applied::Ok
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adapter_hash_is_stable_and_distinguishes_adapters() {
        assert_eq!(adapter_hash("abc"), adapter_hash("abc"));
        assert_ne!(adapter_hash("abc"), adapter_hash("abd"));
    }

    #[test]
    fn default_config_has_no_frame_cap() {
        let settings = presentation_settings(&Config::default(), PresentationSettings::default());
        assert_eq!(settings.frame_rate_limit, FrameRateLimit::Unlimited);
    }

    #[test]
    fn webgpu_is_selectable_directly() {
        assert_eq!(backend_for(GraphicsApiPreference::WebGpu), Some(Backend::WebGpu));
    }

    #[test]
    fn metal_maps_to_webgpu() {
        assert_eq!(backend_for(GraphicsApiPreference::Metal), Some(Backend::WebGpu));
        assert_eq!(backend_for(GraphicsApiPreference::Auto), None);
    }
}
