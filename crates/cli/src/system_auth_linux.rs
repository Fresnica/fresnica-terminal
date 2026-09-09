use std::path::PathBuf;
use std::sync::Arc;

use crate::system_auth::SystemAuthBackend;
use crate::system_auth_process::TrustedSystemAuthProcessBackend;

const PROVIDER_PATH: &str = "/usr/libexec/fresnica-system-auth-provider";

pub(crate) fn backend() -> Arc<dyn SystemAuthBackend> {
    Arc::new(TrustedSystemAuthProcessBackend::new(PathBuf::from(
        PROVIDER_PATH,
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_path_is_fixed_and_not_path_discovered() {
        assert_eq!(PROVIDER_PATH, "/usr/libexec/fresnica-system-auth-provider");
    }
}
