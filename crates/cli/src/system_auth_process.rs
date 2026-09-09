use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use fresnica_client::{SystemAuthRelease, SystemAuthSlot};

use crate::system_auth::SystemAuthBackend;

const EXIT_PASSPHRASE_REQUIRED: i32 = 10;
const EXIT_CANCELLED: i32 = 11;

pub(crate) struct TrustedSystemAuthProcessBackend {
    executable: PathBuf,
}

impl TrustedSystemAuthProcessBackend {
    pub(crate) fn new(executable: PathBuf) -> Self {
        Self { executable }
    }

    fn command(&self, action: &str, slot: Option<&SystemAuthSlot>) -> Command {
        let mut command = Command::new(&self.executable);
        command.arg(action);
        if let Some(slot) = slot {
            command.arg(slot.storage_id());
        }
        command
    }
    fn output_error(&self, action: &str, output: &std::process::Output) -> String {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        if detail.is_empty() {
            format!("trusted System Auth provider {action} failed")
        } else {
            format!("trusted System Auth provider {action} failed: {detail}")
        }
    }

    fn executable_available(&self) -> bool {
        fs::metadata(&self.executable)
            .map(|metadata| metadata.is_file())
            .unwrap_or(false)
    }
}

impl SystemAuthBackend for TrustedSystemAuthProcessBackend {
    fn available(&self) -> bool {
        if !self.executable_available() {
            return false;
        }
        self.command("probe", None)
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }

    fn has(&self, slot: &SystemAuthSlot) -> Result<bool, String> {
        let output = self
            .command("has", Some(slot))
            .output()
            .map_err(|error| format!("unable to run trusted System Auth provider: {error}"))?;
        match output.status.code() {
            Some(0) => Ok(true),
            Some(EXIT_PASSPHRASE_REQUIRED) => Ok(false),
            _ => Err(self.output_error("status", &output)),
        }
    }

    fn enroll(&self, slot: &SystemAuthSlot, unlock_key: &[u8]) -> Result<(), String> {
        let mut child = self
            .command("enroll", Some(slot))
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| format!("unable to run trusted System Auth provider: {error}"))?;
        child
            .stdin
            .take()
            .ok_or_else(|| "trusted System Auth provider stdin is unavailable".to_owned())?
            .write_all(unlock_key)
            .map_err(|error| {
                format!("unable to send unlock key to System Auth provider: {error}")
            })?;
        let output = child
            .wait_with_output()
            .map_err(|error| format!("unable to wait for System Auth provider: {error}"))?;
        if output.status.success() {
            Ok(())
        } else {
            Err(self.output_error("enrollment", &output))
        }
    }
    fn release(&self, slot: &SystemAuthSlot) -> SystemAuthRelease {
        let output = match self.command("release", Some(slot)).output() {
            Ok(output) => output,
            Err(error) => {
                return SystemAuthRelease::Failed(format!(
                    "unable to run trusted System Auth provider: {error}"
                ))
            }
        };
        match output.status.code() {
            Some(0) => SystemAuthRelease::UnlockKey(output.stdout),
            Some(EXIT_PASSPHRASE_REQUIRED) => SystemAuthRelease::PassphraseRequired,
            Some(EXIT_CANCELLED) => SystemAuthRelease::Cancelled,
            _ => SystemAuthRelease::Failed(self.output_error("release", &output)),
        }
    }

    fn delete(&self, slot: &SystemAuthSlot) -> Result<(), String> {
        let output = self
            .command("delete", Some(slot))
            .output()
            .map_err(|error| format!("unable to run trusted System Auth provider: {error}"))?;
        if output.status.success() || output.status.code() == Some(EXIT_PASSPHRASE_REQUIRED) {
            Ok(())
        } else {
            Err(self.output_error("delete", &output))
        }
    }
}
#[cfg(all(test, unix))]
mod tests {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;

    use super::*;

    const PUBLIC: &str = "GDLVVGABQKYQVN6VJP7NHSLEA45A5YLS6PNKMIZFV4BBU2HXA5IRVHUR";

    fn slot() -> SystemAuthSlot {
        SystemAuthSlot {
            signer_public_key: PUBLIC.to_owned(),
            envelope_fingerprint: "0123456789abcdef".to_owned(),
        }
    }

    fn provider(root: &Path, release_code: i32) -> TrustedSystemAuthProcessBackend {
        let path = root.join("provider");
        let script = format!(
            "#!/bin/sh\ncase \"$1\" in\nprobe) exit 0;;\nhas) exit 0;;\nenroll) cat > \"{}/key\"; exit 0;;\nrelease) cat \"{}/key\"; exit {};;\ndelete) rm -f \"{}/key\"; exit 0;;\nesac\nexit 20\n",
            root.display(), root.display(), release_code, root.display()
        );
        fs::write(&path, script).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        TrustedSystemAuthProcessBackend::new(path)
    }
    #[test]
    fn process_backend_keeps_unlock_key_on_pipes() {
        let root = std::env::temp_dir().join(format!(
            "fresnica-system-auth-process-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let backend = provider(&root, 0);
        let slot = slot();
        let key = [7u8; 32];

        assert!(backend.available());
        assert!(backend.has(&slot).unwrap());
        backend.enroll(&slot, &key).unwrap();
        assert_eq!(
            backend.release(&slot),
            SystemAuthRelease::UnlockKey(key.to_vec())
        );
        backend.delete(&slot).unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn process_backend_preserves_final_auth_outcomes() {
        let root = std::env::temp_dir().join(format!(
            "fresnica-system-auth-outcomes-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let slot = slot();
        fs::write(root.join("key"), [1u8; 32]).unwrap();

        let passphrase = provider(&root, EXIT_PASSPHRASE_REQUIRED);
        assert_eq!(
            passphrase.release(&slot),
            SystemAuthRelease::PassphraseRequired
        );

        let cancelled = provider(&root, EXIT_CANCELLED);
        assert_eq!(cancelled.release(&slot), SystemAuthRelease::Cancelled);

        let failed = provider(&root, 20);
        assert!(matches!(
            failed.release(&slot),
            SystemAuthRelease::Failed(_)
        ));
        fs::remove_dir_all(root).unwrap();
    }
}
