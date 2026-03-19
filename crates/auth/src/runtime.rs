use std::sync::OnceLock;

use runtime_bootstrap::{RuntimeBootstrapError, RuntimeBootstrapSpec, build_runtime};
use tokio::runtime::{Handle, Runtime};

static AUTH_TOKIO_RUNTIME: OnceLock<Result<Runtime, RuntimeBootstrapError>> = OnceLock::new();
const AUTH_RUNTIME_SPEC: RuntimeBootstrapSpec<'static> = RuntimeBootstrapSpec::new(
    "vertex-auth-tokio",
    "vertexlauncher/auth/runtime",
    "auth runtime",
);

fn auth_runtime_state() -> &'static Result<Runtime, RuntimeBootstrapError> {
    AUTH_TOKIO_RUNTIME.get_or_init(|| build_runtime(&AUTH_RUNTIME_SPEC))
}

pub(crate) fn auth_runtime() -> Result<&'static Runtime, RuntimeBootstrapError> {
    match auth_runtime_state() {
        Ok(runtime) => Ok(runtime),
        Err(error) => Err(error.clone()),
    }
}

pub(crate) fn auth_runtime_handle() -> Result<&'static Handle, RuntimeBootstrapError> {
    auth_runtime().map(|runtime| runtime.handle())
}
