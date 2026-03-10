use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use curseforge::Client as CurseForgeClient;
use eframe::egui;
use instances::{
    InstanceRecord, InstanceStore, NewInstanceSpec, create_instance, delete_instance,
    instance_root_path,
};
use launcher_ui::{
    ui::style,
    ui::{components::settings_widgets, modal},
};
use modrinth::Client as ModrinthClient;
use serde::Deserialize;
use textui::{ButtonOptions, LabelOptions, TextUi};

const MODAL_GAP_SM: f32 = 6.0;
const MODAL_GAP_MD: f32 = 8.0;
const MODAL_GAP_LG: f32 = 10.0;
const ACTION_BUTTON_MAX_WIDTH: f32 = 260.0;
const MANAGED_CONTENT_MANIFEST_FILE_NAME: &str = ".vertex-content-manifest.toml";

#[derive(Debug, Default)]
pub struct ImportInstanceState {
    pub package_path: String,
    pub instance_name: String,
    pub error: Option<String>,
    preview: Option<ImportPreview>,
}

impl ImportInstanceState {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

#[derive(Clone, Debug)]
pub struct ImportRequest {
    pub package_path: PathBuf,
    pub instance_name: String,
}

#[derive(Clone, Debug)]
pub enum ModalAction {
    None,
    Cancel,
    Import(ImportRequest),
}

#[derive(Clone, Debug)]
struct ImportPreview {
    kind: ImportPackageKind,
    detected_name: String,
    game_version: String,
    modloader: String,
    modloader_version: String,
    summary: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ImportPackageKind {
    VertexPack,
    ModrinthPack,
}

impl ImportPackageKind {
    fn label(self) -> &'static str {
        match self {
            ImportPackageKind::VertexPack => "Vertex .vtmpack",
            ImportPackageKind::ModrinthPack => "Modrinth .mrpack",
        }
    }
}

pub fn render(
    ctx: &egui::Context,
    text_ui: &mut TextUi,
    state: &mut ImportInstanceState,
) -> ModalAction {
    let mut action = ModalAction::None;
    let viewport_rect = ctx.input(|i| i.content_rect());
    let modal_max_width = (viewport_rect.width() * 0.85).max(1.0);
    let modal_max_height = (viewport_rect.height() * 0.82).max(1.0);
    let modal_pos = egui::pos2(
        (viewport_rect.center().x - modal_max_width * 0.5).clamp(
            viewport_rect.left(),
            viewport_rect.right() - modal_max_width,
        ),
        (viewport_rect.center().y - modal_max_height * 0.5).clamp(
            viewport_rect.top(),
            viewport_rect.bottom() - modal_max_height,
        ),
    );

    modal::show_scrim(ctx, "import_instance_modal_scrim", viewport_rect);
    egui::Window::new("Import Profile")
        .id(egui::Id::new("import_instance_modal_window"))
        .order(egui::Order::Foreground)
        .fixed_pos(modal_pos)
        .fixed_size(egui::vec2(modal_max_width, modal_max_height))
        .collapsible(false)
        .resizable(false)
        .movable(false)
        .title_bar(false)
        .hscroll(false)
        .vscroll(true)
        .constrain(true)
        .constrain_to(viewport_rect)
        .frame(modal::window_frame(ctx))
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(MODAL_GAP_MD, MODAL_GAP_MD);
            let text_color = ui.visuals().text_color();
            let heading_style = LabelOptions {
                font_size: 34.0,
                line_height: 38.0,
                weight: 700,
                color: text_color,
                wrap: false,
                ..LabelOptions::default()
            };
            let body_style = LabelOptions {
                font_size: 18.0,
                line_height: 24.0,
                color: ui.visuals().weak_text_color(),
                wrap: true,
                ..LabelOptions::default()
            };

            let _ = text_ui.label(
                ui,
                "instance_import_heading",
                "Import Profile",
                &heading_style,
            );
            let _ = text_ui.label(
                ui,
                "instance_import_subheading",
                "Import a Vertex .vtmpack or Modrinth .mrpack into a new profile.",
                &body_style,
            );

            let previous_path = state.package_path.clone();
            let _ = settings_widgets::full_width_text_input_row(
                text_ui,
                ui,
                "instance_import_package_path",
                "Package file",
                Some("Select a .vtmpack or .mrpack file."),
                &mut state.package_path,
            );
            if state.package_path != previous_path {
                state.preview = None;
                state.error = None;
            }

            ui.horizontal(|ui| {
                if settings_widgets::full_width_button(
                    text_ui,
                    ui,
                    "instance_import_choose_file",
                    "Choose package",
                    (ui.available_width() * 0.5).clamp(120.0, ACTION_BUTTON_MAX_WIDTH),
                    false,
                )
                .clicked()
                {
                    if let Some(path) = pick_import_file() {
                        state.package_path = path.display().to_string();
                        load_preview_from_state(state);
                    }
                }

                if settings_widgets::full_width_button(
                    text_ui,
                    ui,
                    "instance_import_inspect_file",
                    "Inspect package",
                    (ui.available_width()).clamp(120.0, ACTION_BUTTON_MAX_WIDTH),
                    false,
                )
                .clicked()
                {
                    load_preview_from_state(state);
                }
            });

            ui.add_space(MODAL_GAP_SM);
            let _ = settings_widgets::full_width_text_input_row(
                text_ui,
                ui,
                "instance_import_name",
                "Imported profile name",
                Some("Defaults to the package name, but you can override it."),
                &mut state.instance_name,
            );

            if let Some(preview) = state.preview.as_ref() {
                ui.add_space(MODAL_GAP_SM);
                ui.group(|ui| {
                    ui.set_width(ui.available_width());
                    let _ = text_ui.label(
                        ui,
                        "instance_import_preview_title",
                        "Detected package",
                        &LabelOptions {
                            font_size: 20.0,
                            line_height: 24.0,
                            weight: 600,
                            color: ui.visuals().text_color(),
                            wrap: false,
                            ..LabelOptions::default()
                        },
                    );
                    let _ = text_ui.label(
                        ui,
                        "instance_import_preview_kind",
                        preview.kind.label(),
                        &body_style,
                    );
                    let _ = text_ui.label(
                        ui,
                        "instance_import_preview_versions",
                        format!(
                            "Minecraft {} • {}",
                            preview.game_version,
                            format_loader_label(
                                preview.modloader.as_str(),
                                preview.modloader_version.as_str()
                            )
                        )
                        .as_str(),
                        &body_style,
                    );
                    let _ = text_ui.label(
                        ui,
                        "instance_import_preview_summary",
                        preview.summary.as_str(),
                        &body_style,
                    );
                });
            }

            if let Some(error) = state.error.as_deref() {
                let _ = text_ui.label(
                    ui,
                    "instance_import_error",
                    error,
                    &LabelOptions {
                        color: ui.visuals().error_fg_color,
                        wrap: true,
                        ..LabelOptions::default()
                    },
                );
            }

            ui.add_space(MODAL_GAP_LG);
            ui.horizontal(|ui| {
                let button_style = ButtonOptions {
                    min_size: egui::vec2(160.0, style::CONTROL_HEIGHT),
                    text_color: ui.visuals().text_color(),
                    fill: ui.visuals().widgets.inactive.bg_fill,
                    fill_hovered: ui.visuals().widgets.hovered.bg_fill,
                    fill_active: ui.visuals().widgets.active.bg_fill,
                    fill_selected: ui.visuals().selection.bg_fill,
                    stroke: ui.visuals().widgets.inactive.bg_stroke,
                    ..ButtonOptions::default()
                };
                if text_ui
                    .button(ui, "instance_import_cancel", "Cancel", &button_style)
                    .clicked()
                {
                    action = ModalAction::Cancel;
                }

                let import_disabled = state.package_path.trim().is_empty();
                if ui
                    .add_enabled_ui(!import_disabled, |ui| {
                        text_ui.button(
                            ui,
                            "instance_import_confirm",
                            "Import profile",
                            &button_style,
                        )
                    })
                    .inner
                    .clicked()
                {
                    if state.preview.is_none() {
                        load_preview_from_state(state);
                    }
                    if let Some(preview) = state.preview.as_ref() {
                        let instance_name = non_empty(state.instance_name.as_str())
                            .unwrap_or_else(|| preview.detected_name.clone());
                        action = ModalAction::Import(ImportRequest {
                            package_path: PathBuf::from(state.package_path.trim()),
                            instance_name,
                        });
                    }
                }
            });
        });

    action
}

pub fn import_package(
    store: &mut InstanceStore,
    installations_root: &Path,
    request: ImportRequest,
) -> Result<InstanceRecord, String> {
    let preview = inspect_package(request.package_path.as_path())?;
    match preview.kind {
        ImportPackageKind::VertexPack => import_vtmpack(store, installations_root, &request),
        ImportPackageKind::ModrinthPack => import_mrpack(store, installations_root, &request),
    }
}

fn load_preview_from_state(state: &mut ImportInstanceState) {
    let path = PathBuf::from(state.package_path.trim());
    if path.as_os_str().is_empty() {
        state.preview = None;
        state.error = Some("Choose a .vtmpack or .mrpack file first.".to_owned());
        return;
    }

    match inspect_package(path.as_path()) {
        Ok(preview) => {
            if state.instance_name.trim().is_empty() {
                state.instance_name = preview.detected_name.clone();
            }
            state.preview = Some(preview);
            state.error = None;
        }
        Err(err) => {
            state.preview = None;
            state.error = Some(err);
        }
    }
}

fn pick_import_file() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter("Launcher profiles", &["vtmpack", "mrpack"])
        .add_filter("Vertex packs", &["vtmpack"])
        .add_filter("Modrinth packs", &["mrpack"])
        .pick_file()
}

fn inspect_package(path: &Path) -> Result<ImportPreview, String> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_default();
    match extension.as_str() {
        "vtmpack" => inspect_vtmpack(path),
        "mrpack" => inspect_mrpack(path),
        _ => Err(format!(
            "Unsupported import file {}. Expected .vtmpack or .mrpack.",
            path.display()
        )),
    }
}

fn inspect_vtmpack(path: &Path) -> Result<ImportPreview, String> {
    let manifest = read_vtmpack_manifest(path)?;
    Ok(ImportPreview {
        kind: ImportPackageKind::VertexPack,
        detected_name: manifest.instance.name.clone(),
        game_version: manifest.instance.game_version.clone(),
        modloader: manifest.instance.modloader.clone(),
        modloader_version: manifest.instance.modloader_version.clone(),
        summary: format!(
            "{} for Minecraft {} ({}) with {} downloadable items, {} bundled mods, {} config files.",
            manifest.instance.name,
            manifest.instance.game_version,
            format_loader_label(
                manifest.instance.modloader.as_str(),
                manifest.instance.modloader_version.as_str()
            ),
            manifest.downloadable_content.len(),
            manifest.bundled_mods.len(),
            manifest.configs.len()
        ),
    })
}

fn inspect_mrpack(path: &Path) -> Result<ImportPreview, String> {
    let manifest = read_mrpack_manifest(path)?;
    let dependency_info = resolve_mrpack_dependencies(&manifest.dependencies)?;
    Ok(ImportPreview {
        kind: ImportPackageKind::ModrinthPack,
        detected_name: non_empty(manifest.name.as_str())
            .unwrap_or_else(|| "Imported Modrinth Pack".to_owned()),
        game_version: dependency_info.game_version.clone(),
        modloader: dependency_info.modloader.clone(),
        modloader_version: dependency_info.modloader_version.clone(),
        summary: format!(
            "{} {} for Minecraft {} ({}) with {} packaged files.",
            non_empty(manifest.name.as_str()).unwrap_or_else(|| "Modrinth pack".to_owned()),
            non_empty(manifest.version_id.as_str()).unwrap_or_default(),
            dependency_info.game_version,
            format_loader_label(
                dependency_info.modloader.as_str(),
                dependency_info.modloader_version.as_str()
            ),
            manifest.files.len()
        )
        .trim()
        .to_owned(),
    })
}

fn import_vtmpack(
    store: &mut InstanceStore,
    installations_root: &Path,
    request: &ImportRequest,
) -> Result<InstanceRecord, String> {
    let manifest = read_vtmpack_manifest(request.package_path.as_path())?;
    let instance = create_instance(
        store,
        installations_root,
        NewInstanceSpec {
            name: request.instance_name.clone(),
            description: None,
            thumbnail_path: None,
            modloader: default_if_blank(manifest.instance.modloader.as_str(), "Vanilla".to_owned()),
            game_version: default_if_blank(
                manifest.instance.game_version.as_str(),
                "latest".to_owned(),
            ),
            modloader_version: manifest.instance.modloader_version.clone(),
        },
    )
    .map_err(|err| format!("failed to create imported profile: {err}"))?;
    let instance_root = instance_root_path(installations_root, &instance);

    if let Err(err) = populate_vtmpack_instance(
        request.package_path.as_path(),
        manifest,
        instance_root.as_path(),
    ) {
        let _ = delete_instance(store, instance.id.as_str(), installations_root);
        return Err(err);
    }

    Ok(instance)
}

fn import_mrpack(
    store: &mut InstanceStore,
    installations_root: &Path,
    request: &ImportRequest,
) -> Result<InstanceRecord, String> {
    let manifest = read_mrpack_manifest(request.package_path.as_path())?;
    let dependency_info = resolve_mrpack_dependencies(&manifest.dependencies)?;
    let instance = create_instance(
        store,
        installations_root,
        NewInstanceSpec {
            name: request.instance_name.clone(),
            description: non_empty(manifest.summary.as_deref().unwrap_or_default()),
            thumbnail_path: None,
            modloader: dependency_info.modloader.clone(),
            game_version: dependency_info.game_version.clone(),
            modloader_version: dependency_info.modloader_version.clone(),
        },
    )
    .map_err(|err| format!("failed to create imported profile: {err}"))?;
    let instance_root = instance_root_path(installations_root, &instance);

    if let Err(err) = populate_mrpack_instance(
        request.package_path.as_path(),
        manifest,
        instance_root.as_path(),
    ) {
        let _ = delete_instance(store, instance.id.as_str(), installations_root);
        return Err(err);
    }

    Ok(instance)
}

fn populate_vtmpack_instance(
    package_path: &Path,
    manifest: VtmpackManifest,
    instance_root: &Path,
) -> Result<(), String> {
    extract_vtmpack_payload(package_path, instance_root)?;

    for downloadable in &manifest.downloadable_content {
        if downloadable.file_path.trim().is_empty() {
            continue;
        }
        let destination = join_safe(instance_root, downloadable.file_path.as_str())?;
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|err| {
                format!(
                    "failed to create import directory {}: {err}",
                    parent.display()
                )
            })?;
        }
        download_vtmpack_entry(downloadable, destination.as_path())?;
    }

    Ok(())
}

fn extract_vtmpack_payload(package_path: &Path, instance_root: &Path) -> Result<(), String> {
    let file = fs::File::open(package_path)
        .map_err(|err| format!("failed to open {}: {err}", package_path.display()))?;
    let decoder = xz2::read::XzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);

    for entry in archive
        .entries()
        .map_err(|err| format!("failed to read {}: {err}", package_path.display()))?
    {
        let mut entry = entry.map_err(|err| {
            format!(
                "failed to read archive entry in {}: {err}",
                package_path.display()
            )
        })?;
        let entry_path = entry
            .path()
            .map_err(|err| format!("failed to decode archive path: {err}"))?
            .to_path_buf();
        let entry_string = entry_path.to_string_lossy().replace('\\', "/");

        if entry_string == "manifest.toml" {
            continue;
        }
        if entry_string == format!("metadata/{MANAGED_CONTENT_MANIFEST_FILE_NAME}") {
            let destination = instance_root.join(MANAGED_CONTENT_MANIFEST_FILE_NAME);
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent).map_err(|err| {
                    format!(
                        "failed to create metadata directory {}: {err}",
                        parent.display()
                    )
                })?;
            }
            entry.unpack(destination.as_path()).map_err(|err| {
                format!(
                    "failed to restore managed metadata into {}: {err}",
                    destination.display()
                )
            })?;
            continue;
        }
        if let Some(relative) = entry_string.strip_prefix("bundled_mods/") {
            let destination = join_safe(&instance_root.join("mods"), relative)?;
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent).map_err(|err| {
                    format!(
                        "failed to create bundled mod directory {}: {err}",
                        parent.display()
                    )
                })?;
            }
            entry.unpack(destination.as_path()).map_err(|err| {
                format!(
                    "failed to import bundled mod {}: {err}",
                    destination.display()
                )
            })?;
            continue;
        }
        if let Some(relative) = entry_string.strip_prefix("configs/") {
            let destination = join_safe(&instance_root.join("config"), relative)?;
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent).map_err(|err| {
                    format!(
                        "failed to create config directory {}: {err}",
                        parent.display()
                    )
                })?;
            }
            entry.unpack(destination.as_path()).map_err(|err| {
                format!("failed to import config {}: {err}", destination.display())
            })?;
        }
    }
    Ok(())
}

fn download_vtmpack_entry(
    entry: &VtmpackDownloadableEntry,
    destination: &Path,
) -> Result<(), String> {
    match normalize_source_name(entry.selected_source.as_deref()) {
        Some(ManagedSource::Modrinth) => {
            let version_id = entry
                .selected_version_id
                .as_deref()
                .ok_or_else(|| format!("missing Modrinth version id for {}", entry.name))?;
            let version = ModrinthClient::default()
                .get_version(version_id)
                .map_err(|err| format!("failed to fetch Modrinth version {version_id}: {err}"))?;
            let file = version
                .files
                .iter()
                .find(|file| file.primary)
                .or_else(|| version.files.first())
                .ok_or_else(|| {
                    format!("no downloadable file found for Modrinth version {version_id}")
                })?;
            download_file(file.url.as_str(), destination)
        }
        Some(ManagedSource::CurseForge) => {
            let project_id = entry
                .curseforge_project_id
                .ok_or_else(|| format!("missing CurseForge project id for {}", entry.name))?;
            let file_id = entry
                .selected_version_id
                .as_deref()
                .ok_or_else(|| format!("missing CurseForge file id for {}", entry.name))?
                .parse::<u64>()
                .map_err(|err| format!("invalid CurseForge file id for {}: {err}", entry.name))?;
            let client = CurseForgeClient::from_env().ok_or_else(|| {
                "CurseForge API key missing; set VERTEX_CURSEFORGE_API_KEY or CURSEFORGE_API_KEY to import this pack."
                    .to_owned()
            })?;
            let file = find_curseforge_file(&client, project_id, file_id)?;
            let download_url = file.download_url.ok_or_else(|| {
                format!("CurseForge file {file_id} for project {project_id} has no download URL")
            })?;
            download_file(download_url.as_str(), destination)
        }
        None => {
            if let Some(version_id) = entry.selected_version_id.as_deref() {
                let version = ModrinthClient::default()
                    .get_version(version_id)
                    .map_err(|err| {
                        format!("failed to fetch Modrinth fallback version {version_id}: {err}")
                    })?;
                let file = version
                    .files
                    .iter()
                    .find(|file| file.primary)
                    .or_else(|| version.files.first())
                    .ok_or_else(|| {
                        format!("no downloadable file found for Modrinth version {version_id}")
                    })?;
                return download_file(file.url.as_str(), destination);
            }
            Err(format!(
                "download source for {} could not be determined from the pack metadata",
                entry.name
            ))
        }
    }
}

fn find_curseforge_file(
    client: &CurseForgeClient,
    project_id: u64,
    file_id: u64,
) -> Result<curseforge::File, String> {
    let mut index = 0u32;
    loop {
        let files = client
            .list_mod_files(project_id, None, None, index, 50)
            .map_err(|err| format!("failed to list CurseForge files for {project_id}: {err}"))?;
        if files.is_empty() {
            break;
        }
        if let Some(found) = files.into_iter().find(|file| file.id == file_id) {
            return Ok(found);
        }
        index += 50;
    }
    Err(format!(
        "CurseForge file {file_id} was not found for project {project_id}"
    ))
}

fn populate_mrpack_instance(
    package_path: &Path,
    manifest: MrpackManifest,
    instance_root: &Path,
) -> Result<(), String> {
    extract_mrpack_overrides(package_path, instance_root)?;
    for file in manifest.files {
        if matches!(
            file.env.as_ref().and_then(|env| env.client.as_deref()),
            Some("unsupported")
        ) {
            continue;
        }
        let destination = join_safe(instance_root, file.path.as_str())?;
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|err| {
                format!(
                    "failed to create import directory {}: {err}",
                    parent.display()
                )
            })?;
        }
        let download_url = file
            .downloads
            .first()
            .cloned()
            .ok_or_else(|| format!("Modrinth pack entry {} has no download URL", file.path))?;
        download_file(download_url.as_str(), destination.as_path())?;
    }
    Ok(())
}

fn extract_mrpack_overrides(package_path: &Path, instance_root: &Path) -> Result<(), String> {
    let file = fs::File::open(package_path)
        .map_err(|err| format!("failed to open {}: {err}", package_path.display()))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|err| format!("failed to read {}: {err}", package_path.display()))?;

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|err| {
            format!(
                "failed to read zip entry in {}: {err}",
                package_path.display()
            )
        })?;
        let entry_name = entry.name().replace('\\', "/");
        let Some(relative) = entry_name
            .strip_prefix("overrides/")
            .or_else(|| entry_name.strip_prefix("client-overrides/"))
        else {
            continue;
        };
        if relative.is_empty() {
            continue;
        }
        let destination = join_safe(instance_root, relative)?;
        if entry.is_dir() {
            fs::create_dir_all(destination.as_path()).map_err(|err| {
                format!(
                    "failed to create override directory {}: {err}",
                    destination.display()
                )
            })?;
            continue;
        }
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|err| {
                format!(
                    "failed to create override parent {}: {err}",
                    parent.display()
                )
            })?;
        }
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).map_err(|err| {
            format!(
                "failed to read override {} from {}: {err}",
                entry_name,
                package_path.display()
            )
        })?;
        fs::write(destination.as_path(), bytes)
            .map_err(|err| format!("failed to write override {}: {err}", destination.display()))?;
    }

    Ok(())
}

fn read_vtmpack_manifest(path: &Path) -> Result<VtmpackManifest, String> {
    let file =
        fs::File::open(path).map_err(|err| format!("failed to open {}: {err}", path.display()))?;
    let decoder = xz2::read::XzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);

    for entry in archive
        .entries()
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?
    {
        let mut entry = entry.map_err(|err| format!("failed to read archive entry: {err}"))?;
        let entry_path = entry
            .path()
            .map_err(|err| format!("failed to decode archive path: {err}"))?;
        if entry_path == Path::new("manifest.toml") {
            let mut raw = String::new();
            entry
                .read_to_string(&mut raw)
                .map_err(|err| format!("failed to read manifest.toml: {err}"))?;
            return toml::from_str(&raw)
                .map_err(|err| format!("failed to parse vtmpack manifest: {err}"));
        }
    }

    Err(format!(
        "No manifest.toml found in Vertex pack {}",
        path.display()
    ))
}

fn read_mrpack_manifest(path: &Path) -> Result<MrpackManifest, String> {
    let file =
        fs::File::open(path).map_err(|err| format!("failed to open {}: {err}", path.display()))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    let mut manifest = archive
        .by_name("modrinth.index.json")
        .map_err(|err| format!("missing modrinth.index.json in {}: {err}", path.display()))?;
    let mut raw = String::new();
    manifest
        .read_to_string(&mut raw)
        .map_err(|err| format!("failed to read modrinth.index.json: {err}"))?;
    serde_json::from_str(&raw).map_err(|err| format!("failed to parse modrinth.index.json: {err}"))
}

fn resolve_mrpack_dependencies(
    dependencies: &HashMap<String, String>,
) -> Result<MrpackDependencyInfo, String> {
    let game_version = dependencies
        .get("minecraft")
        .cloned()
        .ok_or_else(|| "Modrinth pack is missing the required minecraft dependency.".to_owned())?;

    let loader_candidates = [
        ("neoforge", "NeoForge"),
        ("forge", "Forge"),
        ("fabric-loader", "Fabric"),
        ("quilt-loader", "Quilt"),
    ];
    for (key, label) in loader_candidates {
        if let Some(version) = dependencies.get(key) {
            return Ok(MrpackDependencyInfo {
                game_version,
                modloader: label.to_owned(),
                modloader_version: version.clone(),
            });
        }
    }

    Ok(MrpackDependencyInfo {
        game_version,
        modloader: "Vanilla".to_owned(),
        modloader_version: String::new(),
    })
}

fn normalize_source_name(source: Option<&str>) -> Option<ManagedSource> {
    match source?.trim().to_ascii_lowercase().as_str() {
        "modrinth" => Some(ManagedSource::Modrinth),
        "curseforge" => Some(ManagedSource::CurseForge),
        _ => None,
    }
}

fn join_safe(root: &Path, relative: &str) -> Result<PathBuf, String> {
    let relative = Path::new(relative);
    let mut clean = PathBuf::new();
    for component in relative.components() {
        match component {
            Component::Normal(part) => clean.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(format!(
                    "unsafe path in import package: {}",
                    relative.display()
                ));
            }
        }
    }
    Ok(root.join(clean))
}

fn download_file(url: &str, destination: &Path) -> Result<(), String> {
    let response = ureq::get(url)
        .call()
        .map_err(|err| format!("download request failed for {url}: {err}"))?;
    let mut reader = response.into_body().into_reader();
    let mut bytes = Vec::new();
    reader
        .read_to_end(&mut bytes)
        .map_err(|err| format!("failed to read download body from {url}: {err}"))?;
    fs::write(destination, bytes)
        .map_err(|err| format!("failed to write {}: {err}", destination.display()))
}

fn non_empty(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

fn default_if_blank(value: &str, fallback: String) -> String {
    non_empty(value).unwrap_or(fallback)
}

fn format_loader_label(modloader: &str, version: &str) -> String {
    let version = version.trim();
    if version.is_empty() {
        modloader.trim().to_owned()
    } else {
        format!("{} {}", modloader.trim(), version)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ManagedSource {
    Modrinth,
    CurseForge,
}

#[derive(Debug)]
struct MrpackDependencyInfo {
    game_version: String,
    modloader: String,
    modloader_version: String,
}

#[derive(Debug, Clone, Deserialize)]
struct VtmpackManifest {
    instance: VtmpackInstanceMetadata,
    #[serde(default)]
    downloadable_content: Vec<VtmpackDownloadableEntry>,
    #[serde(default)]
    bundled_mods: Vec<String>,
    #[serde(default)]
    configs: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct VtmpackInstanceMetadata {
    name: String,
    game_version: String,
    modloader: String,
    #[serde(default)]
    modloader_version: String,
}

#[derive(Debug, Clone, Deserialize)]
struct VtmpackDownloadableEntry {
    #[serde(default)]
    name: String,
    file_path: String,
    #[serde(default)]
    curseforge_project_id: Option<u64>,
    #[serde(default)]
    selected_source: Option<String>,
    #[serde(default)]
    selected_version_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct MrpackManifest {
    #[serde(default)]
    name: String,
    #[serde(rename = "versionId", default)]
    version_id: String,
    #[serde(default)]
    summary: Option<String>,
    dependencies: HashMap<String, String>,
    #[serde(default)]
    files: Vec<MrpackFile>,
}

#[derive(Debug, Clone, Deserialize)]
struct MrpackFile {
    path: String,
    #[serde(default)]
    downloads: Vec<String>,
    #[serde(default)]
    env: Option<MrpackFileEnv>,
}

#[derive(Debug, Clone, Deserialize)]
struct MrpackFileEnv {
    #[serde(default)]
    client: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_mrpack_dependencies_for_fabric() {
        let dependencies = HashMap::from([
            ("minecraft".to_owned(), "1.21.1".to_owned()),
            ("fabric-loader".to_owned(), "0.16.10".to_owned()),
        ]);

        let resolved = resolve_mrpack_dependencies(&dependencies).expect("expected dependencies");
        assert_eq!(resolved.game_version, "1.21.1");
        assert_eq!(resolved.modloader, "Fabric");
        assert_eq!(resolved.modloader_version, "0.16.10");
    }

    #[test]
    fn safe_join_rejects_parent_traversal() {
        let result = join_safe(Path::new("/tmp/root"), "../mods/evil.jar");
        assert!(result.is_err());
    }
}
