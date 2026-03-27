use std::{
    io,
    path::Path,
    process::{Command, Stdio},
};

pub fn open_in_file_manager(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Err(format!("path does not exist: {}", path.display()));
    }

    #[cfg(target_os = "windows")]
    {
        return spawn_first_available(path, &[ProgramLaunch::new("explorer", &[])]);
    }

    #[cfg(target_os = "macos")]
    {
        return spawn_first_available(path, &[ProgramLaunch::new("open", &[])]);
    }

    #[cfg(target_os = "linux")]
    {
        if Path::new("/.flatpak-info").exists() {
            return spawn_first_available(
                path,
                &[ProgramLaunch::new("flatpak-spawn", &["--host", "xdg-open"])],
            );
        }
        return spawn_first_available(
            path,
            &[
                ProgramLaunch::new("xdg-open", &[]),
                ProgramLaunch::new("gio", &["open"]),
                ProgramLaunch::new("kioclient5", &["exec"]),
                ProgramLaunch::new("kioclient", &["exec"]),
            ],
        );
    }

    #[allow(unreachable_code)]
    Err("opening folders is not supported on this platform".to_owned())
}

fn spawn_first_available(path: &Path, candidates: &[ProgramLaunch<'_>]) -> Result<(), String> {
    let mut unavailable = Vec::new();
    let mut failures = Vec::new();

    for candidate in candidates {
        match spawn_program(path, candidate) {
            Ok(()) => return Ok(()),
            Err(LaunchOutcome::Unavailable) => unavailable.push(candidate.program),
            Err(LaunchOutcome::Failed(error)) => {
                failures.push(format!("{}: {error}", candidate.program))
            }
        }
    }

    if !failures.is_empty() {
        return Err(failures.join(" | "));
    }
    if !unavailable.is_empty() {
        return Err(format!(
            "no supported file manager launcher was found ({})",
            unavailable.join(", ")
        ));
    }
    Err("no supported file manager launcher succeeded".to_owned())
}

fn spawn_program(path: &Path, candidate: &ProgramLaunch<'_>) -> Result<(), LaunchOutcome> {
    let mut command = Command::new(candidate.program);
    command
        .args(candidate.args)
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    match command.spawn() {
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Err(LaunchOutcome::Unavailable),
        Err(error) => Err(LaunchOutcome::Failed(error)),
    }
}

enum LaunchOutcome {
    Unavailable,
    Failed(io::Error),
}

struct ProgramLaunch<'a> {
    program: &'a str,
    args: &'a [&'a str],
}

impl<'a> ProgramLaunch<'a> {
    const fn new(program: &'a str, args: &'a [&'a str]) -> Self {
        Self { program, args }
    }
}
