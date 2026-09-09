use std::env;
use std::path::PathBuf;
use std::sync::Arc;

use crate::system_auth::SystemAuthBackend;
use crate::system_auth_process::TrustedSystemAuthProcessBackend;

const PROVIDER_RELATIVE_PATH: &str = r"Fresnica\SystemAuth\fresnica-system-auth-provider.exe";

pub(crate) fn backend() -> Arc<dyn SystemAuthBackend> {
    Arc::new(TrustedSystemAuthProcessBackend::new(provider_path()))
}

fn provider_path() -> PathBuf {
    env::var_os("ProgramFiles")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Program Files"))
        .join(PROVIDER_RELATIVE_PATH)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_is_fixed_under_program_files_not_path_discovered() {
        assert!(PROVIDER_RELATIVE_PATH.ends_with("fresnica-system-auth-provider.exe"));
        assert!(PROVIDER_RELATIVE_PATH.starts_with(r"Fresnica\SystemAuth\"));
    }
}
