//! The game's data version, which `options.txt` records as `version` so the game can tell which
//! release last wrote the file.

use std::io::Read;
use std::path::Path;

/// Data version of the game in `client_jar`, from the `version.json` inside it (present in
/// 1.14 and newer). `None` if the jar is missing or predates that file.
pub fn read_world_version(client_jar: &Path) -> Option<i64> {
    let file = std::fs::File::open(client_jar).ok()?;
    let mut archive = zip::ZipArchive::new(file).ok()?;
    let mut entry = archive.by_name("version.json").ok()?;
    let mut text = String::new();
    entry.read_to_string(&mut text).ok()?;
    let json: serde_json::Value = serde_json::from_str(&text).ok()?;
    json.get("world_version")?.as_i64()
}

/// Where the launcher keeps the client jar for `game_version` inside an instance.
pub fn client_jar_path(instance_root: &Path, game_version: &str) -> std::path::PathBuf {
    instance_root
        .join("versions")
        .join(game_version)
        .join(format!("{game_version}.jar"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn reads_world_version_from_the_jar() {
        let dir = std::env::temp_dir().join(format!("vertex-dv-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let jar = dir.join("client.jar");
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&jar).unwrap());
        zip.start_file("version.json", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(br#"{"id":"1.20.1","world_version":3465}"#)
            .unwrap();
        zip.finish().unwrap();

        assert_eq!(read_world_version(&jar), Some(3465));
        assert_eq!(read_world_version(&dir.join("missing.jar")), None);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
