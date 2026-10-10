use super::*;

#[derive(Debug, thiserror::Error)]
pub enum InstallationError {
    #[error("HTTP status {status} for {url}: {body}")]
    HttpStatus {
        url: String,
        status: u16,
        body: String,
    },
    #[error("HTTP transport error while requesting {url}: {message}")]
    Transport { url: String, message: String },
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error(
        "Java executable was not found: {executable}. Source: {java_source}. Requested: {requested}. OS error: {message}. Configure a valid Java path or install Java."
    )]
    JavaExecutableNotFound {
        executable: String,
        java_source: String,
        requested: String,
        message: String,
    },
    #[error("Minecraft version '{0}' was not found in Mojang manifest")]
    UnknownMinecraftVersion(String),
    #[error("Version metadata for '{0}' is missing client download information")]
    MissingClientDownload(String),
    #[error("No modloader version was provided for {loader} on Minecraft {game_version}")]
    MissingModloaderVersion {
        loader: String,
        game_version: String,
    },
    #[error("Java runtime is required to install {loader} but was not configured")]
    MissingJavaRuntime { loader: String },
    #[error(
        "{loader} installer failed for Minecraft {game_version} ({loader_version}); command: {command}; status: {status}; stderr: {stderr}"
    )]
    ModloaderInstallerFailed {
        loader: String,
        game_version: String,
        loader_version: String,
        command: String,
        status: String,
        stderr: String,
    },
    #[error(
        "{loader} installer did not produce a usable version profile for Minecraft {game_version} ({loader_version}) in {}",
        .versions_dir.display()
    )]
    ModloaderInstallOutputMissing {
        loader: String,
        game_version: String,
        loader_version: String,
        versions_dir: PathBuf,
    },
    #[error("OpenJDK provisioning is not supported on this platform ({0})")]
    UnsupportedPlatform(String),
    #[error("Could not resolve OpenJDK {runtime_major} package metadata from Adoptium API")]
    OpenJdkMetadataMissing { runtime_major: u8 },
    #[error("Could not resolve a launch profile for {modloader} on Minecraft {game_version}")]
    LaunchProfileMissing {
        modloader: String,
        game_version: String,
    },
    #[error("Launch profile {profile_id} is missing mainClass")]
    LaunchMainClassMissing { profile_id: String },
    #[error("Launch profile {profile_id} is missing required file: {}", .path.display())]
    LaunchFileMissing { profile_id: String, path: PathBuf },
    #[error("Minecraft exited immediately (status: {status}). See launch log: {}", .log_path.display())]
    LaunchExitedImmediately { status: String, log_path: PathBuf },
    #[error("Minecraft for {instance_root} is already running (pid {pid})")]
    InstanceAlreadyRunning { instance_root: String, pid: u32 },
    #[error("Account '{account}' is already running Minecraft in {instance_root}")]
    AccountAlreadyInUse {
        account: String,
        instance_root: String,
    },
}
