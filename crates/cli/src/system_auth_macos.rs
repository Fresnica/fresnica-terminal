use std::env;
use std::path::PathBuf;
use std::sync::Arc;

use crate::system_auth::SystemAuthBackend;
use crate::system_auth_process::TrustedSystemAuthProcessBackend;

const PROVIDER_APP: &str = "FresnicaSystemAuth.app";
const PROVIDER_BINARY: &str = "fresnica-system-auth-provider";

pub(crate) fn backend() -> Arc<dyn SystemAuthBackend> {
    Arc::new(TrustedSystemAuthProcessBackend::new(provider_path()))
}

fn provider_path() -> PathBuf {
    let executable = env::current_exe().unwrap_or_else(|_| PathBuf::from("fresnica"));
    let directory = executable
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));
    directory
        .join(PROVIDER_APP)
        .join("Contents")
        .join("MacOS")
        .join(PROVIDER_BINARY)
}
