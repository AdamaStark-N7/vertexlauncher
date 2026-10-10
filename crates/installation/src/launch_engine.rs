use super::*;

#[path = "launch_engine/java_executable.rs"]
mod java_executable;
#[path = "launch_engine/launch_arguments.rs"]
mod launch_arguments;
#[path = "launch_engine/launch_classpath.rs"]
mod launch_classpath;
#[path = "launch_engine/launch_natives.rs"]
mod launch_natives;
#[path = "launch_engine/launch_profile.rs"]
mod launch_profile;

pub(crate) use java_executable::*;
pub(crate) use launch_arguments::*;
pub(crate) use launch_classpath::*;
pub(crate) use launch_natives::*;
pub(crate) use launch_profile::*;

#[path = "launch_engine_downloads.rs"]
mod launch_engine_downloads;
#[path = "launch_engine_modloaders.rs"]
mod launch_engine_modloaders;

pub(crate) use launch_engine_downloads::*;
pub(crate) use launch_engine_modloaders::*;

pub(crate) fn report_install_progress(
    progress: Option<&InstallProgressSink>,
    event: InstallProgress,
) {
    if let Some(callback) = progress {
        callback(event);
    }
}
