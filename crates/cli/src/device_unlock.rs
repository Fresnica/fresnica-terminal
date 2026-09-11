use std::io::{self, IsTerminal};
use std::sync::{Arc, OnceLock};

use fresnica_client::{
    prepare_system_auth_enrollment, system_auth_slot, verify_passcode, FresnicaClient,
    SystemAuthRelease, SystemAuthSlot, SystemAuthUnlockProvider, WalletStorage,
};

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DeviceUnlockState {
    Unavailable,
    Disabled,
    Locked,
    Ready,
    NeedsReauthorization,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DeviceAuthenticationOutcome {
    Authenticated,
    #[allow(dead_code)]
    Cancelled,
    PassphraseRequired,
}

pub(crate) trait DeviceAuthenticator: Send + Sync {
    fn authenticate(&self) -> Result<DeviceAuthenticationOutcome, String>;
}

#[allow(dead_code)]
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum DeviceSecretRead {
    Secret(Vec<u8>),
    Missing,
    Cancelled,
}

pub(crate) trait DeviceSecretStore: Send + Sync {
    fn state(&self, slot: &SystemAuthSlot) -> Result<DeviceUnlockState, String>;
    fn enroll(&self, slot: &SystemAuthSlot, unlock_key: &[u8]) -> Result<(), String>;
    #[allow(dead_code)]
    fn update_enrollment(&self, slot: &SystemAuthSlot) -> Result<(), String> {
        let _ = slot;
        Err("device unlock migration is unavailable".to_owned())
    }
    fn read(&self, slot: &SystemAuthSlot) -> Result<DeviceSecretRead, String>;
    fn delete(&self, slot: &SystemAuthSlot) -> Result<(), String>;
}

pub(crate) trait DeviceUnlockBackend: Send + Sync {
    fn provider_name(&self) -> &'static str;
    fn state(&self, slot: &SystemAuthSlot) -> Result<DeviceUnlockState, String>;
    fn prepare_system_support(&self) -> Result<(), String> {
        Ok(())
    }
    fn authorize_enrollment(&self) -> Result<(), String> {
        Ok(())
    }
    fn cleanup_system_support_if_unused(&self) -> Result<(), String> {
        Ok(())
    }
    fn enroll(&self, slot: &SystemAuthSlot, unlock_key: &[u8]) -> Result<(), String>;
    fn release(&self, slot: &SystemAuthSlot) -> SystemAuthRelease;
    fn delete(&self, slot: &SystemAuthSlot) -> Result<(), String>;
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
struct UnavailableDeviceUnlockBackend;
#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
impl DeviceUnlockBackend for UnavailableDeviceUnlockBackend {
    fn provider_name(&self) -> &'static str {
        "Unavailable"
    }

    fn state(&self, _slot: &SystemAuthSlot) -> Result<DeviceUnlockState, String> {
        Ok(DeviceUnlockState::Unavailable)
    }

    fn enroll(&self, _slot: &SystemAuthSlot, _unlock_key: &[u8]) -> Result<(), String> {
        Err("device unlock is unavailable on this platform".to_owned())
    }

    fn release(&self, _slot: &SystemAuthSlot) -> SystemAuthRelease {
        SystemAuthRelease::PassphraseRequired
    }

    fn delete(&self, _slot: &SystemAuthSlot) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(target_os = "linux")]
fn default_backend() -> Arc<dyn DeviceUnlockBackend> {
    static BACKEND: OnceLock<Arc<dyn DeviceUnlockBackend>> = OnceLock::new();
    Arc::clone(BACKEND.get_or_init(crate::device_unlock_linux::backend))
}
#[cfg(target_os = "macos")]
fn default_backend() -> Arc<dyn DeviceUnlockBackend> {
    static BACKEND: OnceLock<Arc<dyn DeviceUnlockBackend>> = OnceLock::new();
    Arc::clone(BACKEND.get_or_init(crate::device_unlock_macos::backend))
}

#[cfg(target_os = "windows")]
fn default_backend() -> Arc<dyn DeviceUnlockBackend> {
    static BACKEND: OnceLock<Arc<dyn DeviceUnlockBackend>> = OnceLock::new();
    Arc::clone(BACKEND.get_or_init(crate::device_unlock_windows::backend))
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn default_backend() -> Arc<dyn DeviceUnlockBackend> {
    Arc::new(UnavailableDeviceUnlockBackend)
}

pub(crate) fn command(storage: &WalletStorage, arguments: &[String]) -> Result<(), String> {
    match arguments {
        [command, name] if command == "status" => status(storage, name, default_backend()),
        [command, name] if matches!(command.as_str(), "enable" | "disable") => {
            if !io::stdin().is_terminal() {
                return Err(format!(
                    "device-unlock {command} requires an interactive terminal"
                ));
            }
            if command == "enable" {
                enable(storage, name, default_backend())
            } else {
                disable(storage, name, default_backend())
            }
        }
        _ => Err("usage: fresnica wallet device-unlock enable|disable|status NAME".to_owned()),
    }
}

fn enable(
    storage: &WalletStorage,
    name: &str,
    backend: Arc<dyn DeviceUnlockBackend>,
) -> Result<(), String> {
    let record = storage.load(name)?;
    let slot = system_auth_slot(&record)?;
    let result = (|| {
        backend.prepare_system_support()?;
        match backend.state(&slot)? {
            DeviceUnlockState::Unavailable => {
                return Err("device unlock is unavailable on this platform".to_owned())
            }
            DeviceUnlockState::Locked
            | DeviceUnlockState::Ready
            | DeviceUnlockState::NeedsReauthorization => {
                return Err(format!(
                    "device unlock is already enabled for wallet \"{}\"",
                    record.name
                ))
            }
            DeviceUnlockState::Disabled => {}
        }
        backend.authorize_enrollment()?;
        let passphrase = crate::prompt_hidden("Fresnica passphrase: ")?;
        enable_with_passphrase(&record, backend.as_ref(), &passphrase)?;
        println!(
            "Device unlock enabled for wallet \"{}\" on this device.",
            record.name
        );
        Ok(())
    })();
    match result {
        Ok(()) => Ok(()),
        Err(error) => Err(rollback_failed_enable(backend.as_ref(), error)),
    }
}

fn rollback_failed_enable(backend: &dyn DeviceUnlockBackend, error: String) -> String {
    match backend.cleanup_system_support_if_unused() {
        Ok(()) => error,
        Err(cleanup_error) => {
            format!("{error}; Device Unlock system support rollback failed: {cleanup_error}")
        }
    }
}

fn enable_with_passphrase(
    record: &fresnica_client::WalletRecord,
    backend: &dyn DeviceUnlockBackend,
    passphrase: &str,
) -> Result<(), String> {
    let enrollment = prepare_system_auth_enrollment(record, passphrase)?;
    backend.enroll(&enrollment.slot, enrollment.unlock_key())
}
fn disable(
    storage: &WalletStorage,
    name: &str,
    backend: Arc<dyn DeviceUnlockBackend>,
) -> Result<(), String> {
    let record = storage.load(name)?;
    let slot = system_auth_slot(&record)?;
    match backend.state(&slot)? {
        DeviceUnlockState::Unavailable => {
            return Err("device unlock is unavailable on this platform".to_owned())
        }
        DeviceUnlockState::Disabled => {
            return Err(format!(
                "device unlock is not enabled for wallet \"{}\"",
                record.name
            ))
        }
        DeviceUnlockState::Locked
        | DeviceUnlockState::Ready
        | DeviceUnlockState::NeedsReauthorization => {}
    }
    let passphrase = crate::prompt_hidden("Fresnica passphrase: ")?;
    disable_with_passphrase(&record, backend.as_ref(), &passphrase)?;
    backend.cleanup_system_support_if_unused().map_err(|error| {
        format!(
            "device unlock was disabled for wallet \"{}\", but system support cleanup failed: {error}",
            record.name
        )
    })?;
    println!(
        "Device unlock disabled for wallet \"{}\" on this device.",
        record.name
    );
    Ok(())
}

fn disable_with_passphrase(
    record: &fresnica_client::WalletRecord,
    backend: &dyn DeviceUnlockBackend,
    passphrase: &str,
) -> Result<(), String> {
    verify_passcode(record, passphrase)?;
    backend.delete(&system_auth_slot(record)?)
}

fn status(
    storage: &WalletStorage,
    name: &str,
    backend: Arc<dyn DeviceUnlockBackend>,
) -> Result<(), String> {
    let record = storage.load(name)?;
    if record.watch_only() || record.secret.is_none() {
        println!("Device unlock: not applicable (no software signer)");
        return Ok(());
    }
    let slot = system_auth_slot(&record)?;
    let state = backend.state(&slot)?;
    println!("Device unlock: {}", state_label(state));
    if state != DeviceUnlockState::Unavailable {
        println!("Provider: {}", backend.provider_name());
    }
    Ok(())
}

fn state_label(state: DeviceUnlockState) -> &'static str {
    match state {
        DeviceUnlockState::Unavailable => "unavailable",
        DeviceUnlockState::Disabled => "disabled",
        DeviceUnlockState::Locked => "locked",
        DeviceUnlockState::Ready => "ready",
        DeviceUnlockState::NeedsReauthorization => "needs-reauthorization",
    }
}

pub(crate) fn one_shot_providers(
    client: &FresnicaClient,
) -> Result<Vec<SystemAuthUnlockProvider>, String> {
    providers_for_backend(client, default_backend(), io::stdin().is_terminal())
}

fn providers_for_backend(
    client: &FresnicaClient,
    backend: Arc<dyn DeviceUnlockBackend>,
    interactive: bool,
) -> Result<Vec<SystemAuthUnlockProvider>, String> {
    if !interactive {
        return Ok(Vec::new());
    }

    let mut providers = Vec::new();
    for record in client.wallets()? {
        if record.watch_only() || record.secret.is_none() {
            continue;
        }
        let slot = system_auth_slot(&record)?;
        if !matches!(
            backend.state(&slot)?,
            DeviceUnlockState::Locked
                | DeviceUnlockState::Ready
                | DeviceUnlockState::NeedsReauthorization
        ) {
            continue;
        }
        let expected_slot = slot.clone();
        let provider_backend = Arc::clone(&backend);
        providers.push(SystemAuthUnlockProvider::new(
            &record.address,
            move |requested_slot| {
                if requested_slot != &expected_slot {
                    return SystemAuthRelease::Failed(
                        "device-unlock enrollment is stale for the current signer envelope"
                            .to_owned(),
                    );
                }
                provider_backend.release(requested_slot)
            },
        )?);
    }
    Ok(providers)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Mutex;

    use fresnica_client::{wallet as wallet_ops, NetworkProfile};

    use super::*;

    const SECRET: &str = "SCOWDMM5576VUYF2QRFPJEXMFTCEISOFNF5TE2IZOA52YAY4VZ7WBQNO";
    const PASSCODE: &str = "correct horse battery staple";

    #[derive(Default)]
    struct FakeBackend {
        values: Mutex<BTreeMap<String, Vec<u8>>>,
        cleanup_calls: Mutex<usize>,
    }

    impl FakeBackend {
        fn enroll_value(&self, slot: &SystemAuthSlot, unlock_key: &[u8]) {
            self.values
                .lock()
                .unwrap()
                .insert(slot.storage_id(), unlock_key.to_vec());
        }
    }

    impl DeviceUnlockBackend for FakeBackend {
        fn provider_name(&self) -> &'static str {
            "Fake Device Store"
        }
        fn state(&self, slot: &SystemAuthSlot) -> Result<DeviceUnlockState, String> {
            Ok(
                if self.values.lock().unwrap().contains_key(&slot.storage_id()) {
                    DeviceUnlockState::Ready
                } else {
                    DeviceUnlockState::Disabled
                },
            )
        }

        fn cleanup_system_support_if_unused(&self) -> Result<(), String> {
            *self.cleanup_calls.lock().unwrap() += 1;
            Ok(())
        }

        fn enroll(&self, slot: &SystemAuthSlot, unlock_key: &[u8]) -> Result<(), String> {
            self.enroll_value(slot, unlock_key);
            Ok(())
        }

        fn release(&self, slot: &SystemAuthSlot) -> SystemAuthRelease {
            match self.values.lock().unwrap().get(&slot.storage_id()).cloned() {
                Some(value) => SystemAuthRelease::UnlockKey(value),
                None => SystemAuthRelease::PassphraseRequired,
            }
        }

        fn delete(&self, slot: &SystemAuthSlot) -> Result<(), String> {
            self.values.lock().unwrap().remove(&slot.storage_id());
            Ok(())
        }
    }

    fn client() -> (FresnicaClient, std::path::PathBuf) {
        let root = std::env::temp_dir().join(format!(
            "fresnica-terminal-device-unlock-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        let _ = std::fs::remove_dir_all(&root);
        let client =
            FresnicaClient::from_profile(&root, NetworkProfile::for_network("testnet").unwrap())
                .unwrap();
        (client, root)
    }

    #[test]
    fn failed_enable_rolls_back_unused_system_support() {
        let backend = FakeBackend::default();
        let error = rollback_failed_enable(&backend, "authentication cancelled".to_owned());
        assert_eq!(error, "authentication cancelled");
        assert_eq!(*backend.cleanup_calls.lock().unwrap(), 1);
    }

    #[test]
    fn enrolled_exact_signer_becomes_one_shot_provider() {
        let (client, root) = client();
        let record =
            wallet_ops::import_secret_record("wallet", "testnet", SECRET, PASSCODE).unwrap();
        client.storage().save(&record, false).unwrap();
        let enrollment = prepare_system_auth_enrollment(&record, PASSCODE).unwrap();
        let backend = Arc::new(FakeBackend::default());
        backend.enroll_value(&enrollment.slot, enrollment.unlock_key());

        let providers = providers_for_backend(&client, backend, true).unwrap();
        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0].public_key(), record.address);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn noninteractive_invocation_never_activates_device_unlock() {
        let (client, root) = client();
        let record =
            wallet_ops::import_secret_record("wallet", "testnet", SECRET, PASSCODE).unwrap();
        client.storage().save(&record, false).unwrap();
        let enrollment = prepare_system_auth_enrollment(&record, PASSCODE).unwrap();
        let backend = Arc::new(FakeBackend::default());
        backend.enroll_value(&enrollment.slot, enrollment.unlock_key());

        assert!(providers_for_backend(&client, backend, false)
            .unwrap()
            .is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn enrollment_and_removal_require_fresh_fresnica_passphrase() {
        let (client, root) = client();
        let record =
            wallet_ops::import_secret_record("wallet", "testnet", SECRET, PASSCODE).unwrap();
        client.storage().save(&record, false).unwrap();
        let backend = Arc::new(FakeBackend::default());
        let slot = system_auth_slot(&record).unwrap();

        assert!(enable_with_passphrase(&record, backend.as_ref(), "wrong passphrase").is_err());
        assert_eq!(backend.state(&slot).unwrap(), DeviceUnlockState::Disabled);

        enable_with_passphrase(&record, backend.as_ref(), PASSCODE).unwrap();
        assert_eq!(backend.state(&slot).unwrap(), DeviceUnlockState::Ready);
        assert!(disable_with_passphrase(&record, backend.as_ref(), "wrong passphrase").is_err());
        assert_eq!(backend.state(&slot).unwrap(), DeviceUnlockState::Ready);

        disable_with_passphrase(&record, backend.as_ref(), PASSCODE).unwrap();
        assert_eq!(backend.state(&slot).unwrap(), DeviceUnlockState::Disabled);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn changed_envelope_does_not_reuse_device_unlock_enrollment() {
        let (client, root) = client();
        let record =
            wallet_ops::import_secret_record("wallet", "testnet", SECRET, PASSCODE).unwrap();
        let enrollment = prepare_system_auth_enrollment(&record, PASSCODE).unwrap();
        let backend = Arc::new(FakeBackend::default());
        backend.enroll_value(&enrollment.slot, enrollment.unlock_key());
        let changed = wallet_ops::import_secret_record(
            "wallet",
            "testnet",
            SECRET,
            "another correct horse battery staple",
        )
        .unwrap();
        client.storage().save(&changed, false).unwrap();

        assert!(providers_for_backend(&client, backend, true)
            .unwrap()
            .is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }
}
