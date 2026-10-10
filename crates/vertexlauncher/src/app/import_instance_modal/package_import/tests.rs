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
fn rejects_invalid_mrpack_game_version() {
    let dependencies = HashMap::from([
        (
            "minecraft".to_owned(),
            "fabric-loader-0.16.10-1.21.1".to_owned(),
        ),
        ("fabric-loader".to_owned(), "0.16.10".to_owned()),
    ]);

    let result = resolve_mrpack_dependencies(&dependencies);
    assert!(result.is_err());
}

#[test]
fn safe_join_rejects_parent_traversal() {
    let result = join_safe(Path::new("/tmp/root"), Path::new("../mods/evil.jar"));
    assert!(result.is_err());
}

#[test]
fn modrinth_fallback_queries_include_project_name_once() {
    let file = curseforge::File {
        id: 1,
        display_name: "Sodium".to_owned(),
        file_name: "sodium-fabric-1.0.0.jar".to_owned(),
        file_date: String::new(),
        download_count: 0,
        download_url: None,
        hashes: Vec::new(),
        dependencies: Vec::new(),
        game_versions: Vec::new(),
    };

    let queries = modrinth_fallback_queries(&file, Some("Sodium"));
    assert_eq!(queries.first().map(String::as_str), Some("Sodium"));
    assert_eq!(
        queries
            .iter()
            .filter(|query| query.as_str() == "Sodium")
            .count(),
        1
    );
}

#[test]
fn search_fallback_requires_exact_filename_match() {
    let curseforge_file = curseforge::File {
        id: 1,
        display_name: "GeckoLib".to_owned(),
        file_name: "geckolib-forge-1.20.1-4.4.9.jar".to_owned(),
        file_date: String::new(),
        download_count: 0,
        download_url: None,
        hashes: Vec::new(),
        dependencies: Vec::new(),
        game_versions: Vec::new(),
    };
    let version = modrinth::ProjectVersion {
        id: "version".to_owned(),
        project_id: "project".to_owned(),
        version_number: "4.4.9".to_owned(),
        date_published: String::new(),
        downloads: 0,
        loaders: vec!["forge".to_owned()],
        game_versions: vec!["1.20.1".to_owned()],
        dependencies: Vec::new(),
        files: vec![modrinth::ProjectVersionFile {
            url: "https://example.invalid/geckolib-neoforge.jar".to_owned(),
            filename: "geckolib-neoforge-1.20.1-4.4.9.jar".to_owned(),
            primary: true,
            hashes: std::collections::HashMap::new(),
        }],
        environment: modrinth::Environment::Unknown,
    };

    assert!(
        select_modrinth_backup_file(&version, &curseforge_file, "1.20.1", "Forge", true).is_none()
    );
}

#[test]
fn hash_fallback_rejects_loader_or_game_version_mismatch() {
    let curseforge_file = curseforge::File {
        id: 1,
        display_name: "Crop Marker".to_owned(),
        file_name: "crop-marker-forge-1.20.1-1.2.2.jar".to_owned(),
        file_date: String::new(),
        download_count: 0,
        download_url: None,
        hashes: Vec::new(),
        dependencies: Vec::new(),
        game_versions: Vec::new(),
    };
    let version = modrinth::ProjectVersion {
        id: "version".to_owned(),
        project_id: "project".to_owned(),
        version_number: "1.2.2".to_owned(),
        date_published: String::new(),
        downloads: 0,
        loaders: vec!["forge".to_owned()],
        game_versions: vec!["1.20.1".to_owned()],
        dependencies: Vec::new(),
        files: vec![modrinth::ProjectVersionFile {
            url: "https://example.invalid/crop-marker-forge-1.20.4.jar".to_owned(),
            filename: "crop-marker-forge-1.20.4-1.2.2.jar".to_owned(),
            primary: true,
            hashes: std::collections::HashMap::new(),
        }],
        environment: modrinth::Environment::Unknown,
    };

    assert!(
        select_modrinth_backup_file(&version, &curseforge_file, "1.20.1", "Forge", false).is_none()
    );
}

#[test]
fn normalizes_real_minecraft_versions_only() {
    assert_eq!(
        normalize_minecraft_game_version("1.21.1").as_deref(),
        Some("1.21.1")
    );
    assert_eq!(
        normalize_minecraft_game_version("24w14a").as_deref(),
        Some("24w14a")
    );
    assert_eq!(
        normalize_minecraft_game_version("1.20.5-rc1").as_deref(),
        Some("1.20.5-rc1")
    );
    assert!(normalize_minecraft_game_version("fabric-loader-0.16.10-1.21.1").is_none());
    assert!(normalize_minecraft_game_version("2.4.0").is_none());
    assert_eq!(
        normalize_minecraft_game_version("26.3").as_deref(),
        Some("26.3")
    );
    assert_eq!(
        normalize_minecraft_game_version("26.1.2").as_deref(),
        Some("26.1.2")
    );
    assert_eq!(
        normalize_minecraft_game_version("26.2-snapshot-3").as_deref(),
        Some("26.2-snapshot-3")
    );
}

#[test]
fn extracts_game_version_from_meta_style_identifiers() {
    assert_eq!(
        find_minecraft_version_in_text("fabric-loader-0.16.10-1.21.1").as_deref(),
        Some("1.21.1")
    );
}

#[test]
fn curseforge_file_api_download_requires_non_empty_download_url() {
    let mut file = curseforge::File {
        id: 1,
        display_name: "Test".to_owned(),
        file_name: "test.jar".to_owned(),
        file_date: String::new(),
        download_count: 0,
        download_url: Some("https://example.invalid/test.jar".to_owned()),
        hashes: Vec::new(),
        dependencies: Vec::new(),
        game_versions: Vec::new(),
    };
    assert!(curseforge_file_has_api_download(&file));

    file.download_url = Some("   ".to_owned());
    assert!(!curseforge_file_has_api_download(&file));

    file.download_url = None;
    assert!(!curseforge_file_has_api_download(&file));
}

#[test]
fn imports_standard_xz_vtmpack_payload() {
    imports_vtmpack_payload(vtmpack::VtmpackCompressionMode::Standard, "standard-xz");
}

#[test]
fn imports_extreme_xz_vtmpack_payload() {
    imports_vtmpack_payload(vtmpack::VtmpackCompressionMode::Extreme, "extreme-xz");
}

fn imports_vtmpack_payload(compression_mode: vtmpack::VtmpackCompressionMode, suffix: &str) {
    let root = std::env::temp_dir().join(format!(
        "vertexlauncher-import-vtmpack-test-{}-{}",
        std::process::id(),
        suffix
    ));
    let source_root = root.join("source");
    let installations_root = root.join("instances");
    let package_path = root.join("pack.vtmpack");
    let _ = fs::remove_dir_all(root.as_path());
    fs::create_dir_all(source_root.join("mods")).expect("create source mods");
    fs::create_dir_all(source_root.join("config")).expect("create source config");
    fs::write(source_root.join("mods/local.jar"), b"local mod").expect("write local mod");
    fs::write(source_root.join("config/options.txt"), b"ao=true").expect("write config");
    fs::write(source_root.join("servers.dat"), b"servers").expect("write root file");

    let mut included_root_entries = std::collections::BTreeMap::new();
    included_root_entries.insert("mods".to_owned(), true);
    included_root_entries.insert("config".to_owned(), true);
    included_root_entries.insert("servers.dat".to_owned(), true);
    vtmpack::export_instance_as_vtmpack(
        &vtmpack::VtmpackInstanceMetadata {
            id: "source".to_owned(),
            name: "XZ Import Pack".to_owned(),
            game_version: "1.21.1".to_owned(),
            modloader: "Fabric".to_owned(),
            modloader_version: "0.16.10".to_owned(),
        },
        source_root.as_path(),
        package_path.as_path(),
        &vtmpack::VtmpackExportOptions {
            provider_mode: vtmpack::VtmpackProviderMode::ExcludeCurseForge,
            compression_mode,
            included_root_entries,
        },
    )
    .expect("export vtmpack");

    let preview = inspect_vtmpack(package_path.as_path()).expect("inspect vtmpack");
    assert_eq!(preview.detected_name, "XZ Import Pack");
    assert!(preview.summary.starts_with("XZ"));

    let mut store = InstanceStore::default();
    let request = ImportRequest {
        source: ImportSource::ManifestFile(package_path.clone()),
        instance_name: "Imported XZ Pack".to_owned(),
        manual_curseforge_files: HashMap::new(),
        manual_curseforge_staging_dir: None,
        max_concurrent_downloads: 4,
    };
    let mut progress_updates = Vec::new();
    let instance = import_vtmpack(
        &mut store,
        installations_root.as_path(),
        &request,
        &mut |progress| progress_updates.push(progress),
    )
    .expect("import xz vtmpack");
    let imported_root = instance_root_path(installations_root.as_path(), &instance);

    assert_eq!(
        fs::read(imported_root.join("mods/local.jar")).expect("read imported mod"),
        b"local mod"
    );
    assert_eq!(
        fs::read(imported_root.join("config/options.txt")).expect("read imported config"),
        b"ao=true"
    );
    assert_eq!(
        fs::read(imported_root.join("servers.dat")).expect("read imported root file"),
        b"servers"
    );
    assert!(
        progress_updates
            .iter()
            .any(|progress| progress.message == "Import complete.")
    );

    let _ = fs::remove_dir_all(root.as_path());
}
