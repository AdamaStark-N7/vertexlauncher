//! Running a child process to completion without holding up an async task.

use std::io;

/// Runs `command` and collects its output, on the blocking pool.
pub async fn output(mut command: std::process::Command) -> io::Result<std::process::Output> {
    match crate::spawn_blocking(move || command.output()).await {
        Ok(result) => result,
        Err(error) => Err(io::Error::other(error.to_string())),
    }
}
