use std::fs;
use std::io::Read;
use std::path::Path;

use crate::InstalledContentKind;

const ARCHIVE_SCAN_LIMIT: usize = 512;

pub fn detect_installed_content_kind(path: &Path) -> Option<InstalledContentKind> {
    if path.is_dir() {
        return detect_directory_kind(path).or_else(|| detect_kind_from_name(path));
    }

    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase);
    match extension.as_deref() {
        Some("jar") => Some(InstalledContentKind::Mods),
        Some("zip") => detect_archive_kind(path).or_else(|| detect_kind_from_name(path)),
        _ => detect_kind_from_name(path),
    }
}

fn detect_directory_kind(path: &Path) -> Option<InstalledContentKind> {
    if path.join("META-INF/mods.toml").is_file()
        || path.join("META-INF/neoforge.mods.toml").is_file()
        || path.join("fabric.mod.json").is_file()
        || path.join("quilt.mod.json").is_file()
        || path.join("mcmod.info").is_file()
    {
        return Some(InstalledContentKind::Mods);
    }
    if path.join("shaders").is_dir() {
        return Some(InstalledContentKind::ShaderPacks);
    }
    let has_pack_mcmeta = path.join("pack.mcmeta").is_file();
    if has_pack_mcmeta && path.join("assets").is_dir() {
        return Some(InstalledContentKind::ResourcePacks);
    }
    if has_pack_mcmeta && path.join("data").is_dir() {
        return Some(InstalledContentKind::DataPacks);
    }
    None
}

fn detect_archive_kind(path: &Path) -> Option<InstalledContentKind> {
    let file = fs::File::open(path).ok()?;
    let mut archive = zip::ZipArchive::new(file).ok()?;

    let mut has_pack_mcmeta = false;
    let mut has_assets = false;
    let mut has_data = false;
    let mut has_shaders = false;
    let mut has_mod_marker = false;

    for index in 0..archive.len().min(ARCHIVE_SCAN_LIMIT) {
        let Ok(entry) = archive.by_index(index) else {
            continue;
        };
        let name = entry.name().replace('\\', "/");
        let trimmed = name.trim_matches('/');
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.eq_ignore_ascii_case("pack.mcmeta") {
            has_pack_mcmeta = true;
        }
        if path_starts_with_component(trimmed, "assets") {
            has_assets = true;
        }
        if path_starts_with_component(trimmed, "data") {
            has_data = true;
        }
        if path_starts_with_component(trimmed, "shaders") {
            has_shaders = true;
        }
        if is_mod_marker_path(trimmed) {
            has_mod_marker = true;
        }

        if has_mod_marker {
            return Some(InstalledContentKind::Mods);
        }
    }

    if has_shaders {
        return Some(InstalledContentKind::ShaderPacks);
    }
    if has_pack_mcmeta && has_assets {
        return Some(InstalledContentKind::ResourcePacks);
    }
    if has_pack_mcmeta && has_data {
        return Some(InstalledContentKind::DataPacks);
    }

    let mut root_pack_mcmeta = archive.by_name("pack.mcmeta").ok()?;
    let mut contents = String::new();
    let _ = root_pack_mcmeta.read_to_string(&mut contents);
    let lower = contents.to_ascii_lowercase();
    if lower.contains("shader") {
        return Some(InstalledContentKind::ShaderPacks);
    }
    if lower.contains("resource") || lower.contains("texture") {
        return Some(InstalledContentKind::ResourcePacks);
    }
    if lower.contains("data pack") || lower.contains("datapack") {
        return Some(InstalledContentKind::DataPacks);
    }
    None
}

fn detect_kind_from_name(path: &Path) -> Option<InstalledContentKind> {
    let file_name = path.file_name()?.to_string_lossy().to_ascii_lowercase();
    if file_name.contains("shader") {
        return Some(InstalledContentKind::ShaderPacks);
    }
    if file_name.contains("resource")
        || file_name.contains("texturepack")
        || file_name.contains("texture-pack")
    {
        return Some(InstalledContentKind::ResourcePacks);
    }
    if file_name.contains("datapack") || file_name.contains("data-pack") {
        return Some(InstalledContentKind::DataPacks);
    }
    let extension = path.extension().and_then(|value| value.to_str())?;
    if extension.eq_ignore_ascii_case("jar") {
        return Some(InstalledContentKind::Mods);
    }
    None
}

fn path_starts_with_component(path: &str, component: &str) -> bool {
    path == component
        || path
            .strip_prefix(component)
            .is_some_and(|rest| rest.starts_with('/'))
}

fn is_mod_marker_path(path: &str) -> bool {
    matches!(
        path,
        "fabric.mod.json"
            | "quilt.mod.json"
            | "mcmod.info"
            | "META-INF/mods.toml"
            | "META-INF/neoforge.mods.toml"
    )
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    fn temp_path(label: &str, extension: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!("vertexlauncher-{label}-{unique}.{extension}"))
    }

    fn write_zip(path: &Path, entries: &[(&str, &[u8])]) {
        let file = fs::File::create(path).expect("create zip");
        let mut writer = zip::ZipWriter::new(file);
        let options: zip::write::SimpleFileOptions = zip::write::SimpleFileOptions::default();
        for (name, bytes) in entries {
            writer.start_file(*name, options).expect("start file");
            writer.write_all(bytes).expect("write file");
        }
        writer.finish().expect("finish zip");
    }

    #[test]
    fn detects_mod_jar_from_extension() {
        let path = Path::new("example-mod.jar");
        assert_eq!(
            detect_installed_content_kind(path),
            Some(InstalledContentKind::Mods)
        );
    }

    #[test]
    fn detects_resource_pack_zip_from_archive_structure() {
        let path = temp_path("resource-pack", "zip");
        write_zip(
            path.as_path(),
            &[
                (
                    "pack.mcmeta",
                    br#"{"pack":{"description":"pack","pack_format":15}}"#,
                ),
                ("assets/minecraft/lang/en_us.json", b"{}"),
            ],
        );
        assert_eq!(
            detect_installed_content_kind(path.as_path()),
            Some(InstalledContentKind::ResourcePacks)
        );
        let _ = fs::remove_file(path);
    }

    #[test]
    fn detects_shader_pack_zip_from_archive_structure() {
        let path = temp_path("shader-pack", "zip");
        write_zip(
            path.as_path(),
            &[
                (
                    "pack.mcmeta",
                    br#"{"pack":{"description":"shader pack","pack_format":15}}"#,
                ),
                ("shaders/composite.fsh", b"void main(){}"),
            ],
        );
        assert_eq!(
            detect_installed_content_kind(path.as_path()),
            Some(InstalledContentKind::ShaderPacks)
        );
        let _ = fs::remove_file(path);
    }

    #[test]
    fn detects_data_pack_zip_from_archive_structure() {
        let path = temp_path("data-pack", "zip");
        write_zip(
            path.as_path(),
            &[
                (
                    "pack.mcmeta",
                    br#"{"pack":{"description":"data pack","pack_format":15}}"#,
                ),
                ("data/example/functions/load.mcfunction", b"say hi"),
            ],
        );
        assert_eq!(
            detect_installed_content_kind(path.as_path()),
            Some(InstalledContentKind::DataPacks)
        );
        let _ = fs::remove_file(path);
    }
}
