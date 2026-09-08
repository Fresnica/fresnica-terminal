use std::io::{self, IsTerminal};
use std::sync::Arc;

use fresnica_client::{
    prepare_system_auth_enrollment, system_auth_slot, verify_passcode, FresnicaClient,
    SystemAuthRelease, SystemAuthSlot, SystemAuthUnlockProvider, WalletStorage,
};

pub(crate) trait SystemAuthBackend: Send + Sync {
    fn available(&self) -> bool;
    fn has(&self, slot: &SystemAuthSlot) -> Result<bool, String>;
    fn enroll(&self, slot: &SystemAuthSlot, unlock_key: &[u8]) -> Result<(), String>;
    fn release(&self, slot: &SystemAuthSlot) -> SystemAuthRelease;
    fn delete(&self, slot: &SystemAuthSlot) -> Result<(), String>;
}

struct UnavailableSystemAuthBackend;

impl SystemAuthBackend for UnavailableSystemAuthBackend {
    fn available(&self) -> bool {
        false
    }

    fn has(&self, _slot: &SystemAuthSlot) -> Result<bool, String> {
        Ok(false)
    }

    fn enroll(&self, _slot: &SystemAuthSlot, _unlock_key: &[u8]) -> Result<(), String> {
        Err("system authentication is unavailable on this platform".to_owned())
    }

    fn release(&self, _slot: &SystemAuthSlot) -> SystemAuthRelease {
        SystemAuthRelease::PassphraseRequired
    }

    fn delete(&self, _slot: &SystemAuthSlot) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(target_os = "macos")]
fn default_backend() -> Arc<dyn SystemAuthBackend> {
    crate::system_auth_macos::backend()
}

#[cfg(not(target_os = "macos"))]
fn default_backend() -> Arc<dyn SystemAuthBackend> {
    Arc::new(UnavailableSystemAuthBackend)
}

pub(crate) fn command(storage: &WalletStorage, arguments: &[String]) -> Result<(), String> {
    match arguments {
        [command, name] if command == "enable" => enable(storage, name, default_backend()),
        [command, name] if command == "disable" => disable(storage, name, default_backend()),
        [command, name] if command == "status" => status(storage, name, default_backend()),
        _ => Err("usage: fresnica wallet system-auth enable|disable|status NAME".to_owned()),
    }
}

fn enable(
    storage: &WalletStorage,
    name: &str,
    backend: Arc<dyn SystemAuthBackend>,
) -> Result<(), String> {
    if !backend.available() {
        return Err("system authentication is unavailable on this platform".to_owned());
    }
    let record = storage.load(name)?;
    let slot = system_auth_slot(&record)?;
    if backend.has(&slot)? {
        return Err(format!(
            "system authentication is already enabled for wallet \"{}\"",
            record.name
        ));
    }
    let passphrase = crate::prompt_hidden("Fresnica passphrase: ")?;
    enable_with_passphrase(&record, backend.as_ref(), &passphrase)?;
    println!(
        "System authentication enabled for wallet \"{}\" on this device.",
        record.name
    );
    Ok(())
}

fn enable_with_passphrase(
    record: &fresnica_client::WalletRecord,
    backend: &dyn SystemAuthBackend,
    passphrase: &str,
) -> Result<(), String> {
    let enrollment = prepare_system_auth_enrollment(record, passphrase)?;
    backend.enroll(&enrollment.slot, enrollment.unlock_key())
}

fn disable(
    storage: &WalletStorage,
    name: &str,
    backend: Arc<dyn SystemAuthBackend>,
) -> Result<(), String> {
    if !backend.available() {
        return Err("system authentication is unavailable on this platform".to_owned());
    }
    let record = storage.load(name)?;
    let slot = system_auth_slot(&record)?;
    if !backend.has(&slot)? {
        return Err(format!(
            "system authentication is not enabled for wallet \"{}\"",
            record.name
        ));
    }
    let passphrase = crate::prompt_hidden("Fresnica passphrase: ")?;
    disable_with_passphrase(&record, backend.as_ref(), &passphrase)?;
    println!(
        "System authentication disabled for wallet \"{}\" on this device.",
        record.name
    );
    Ok(())
}

fn disable_with_passphrase(
    record: &fresnica_client::WalletRecord,
    backend: &dyn SystemAuthBackend,
    passphrase: &str,
) -> Result<(), String> {
    verify_passcode(record, passphrase)?;
    backend.delete(&system_auth_slot(record)?)
}

fn status(
    storage: &WalletStorage,
    name: &str,
    backend: Arc<dyn SystemAuthBackend>,
) -> Result<(), String> {
    let record = storage.load(name)?;
    if record.watch_only() || record.secret.is_none() {
        println!("System authentication: not applicable (no software signer)");
        return Ok(());
    }
    if !backend.available() {
        println!("System authentication: unavailable on this platform");
        return Ok(());
    }
    let slot = system_auth_slot(&record)?;
    println!(
        "System authentication: {}",
        if backend.has(&slot)? {
            "enabled"
        } else {
            "disabled"
        }
    );
    Ok(())
}

pub(crate) fn one_shot_providers(
    client: &FresnicaClient,
) -> Result<Vec<SystemAuthUnlockProvider>, String> {
    providers_for_backend(client, default_backend(), io::stdin().is_terminal())
}

pub(crate) fn providers_for_backend(
    client: &FresnicaClient,
    backend: Arc<dyn SystemAuthBackend>,
    interactive: bool,
) -> Result<Vec<SystemAuthUnlockProvider>, String> {
    if !interactive || !backend.available() {
        return Ok(Vec::new());
    }

    let mut providers = Vec::new();
    for record in client.wallets()? {
        if record.watch_only() || record.secret.is_none() {
            continue;
        }
        let slot = system_auth_slot(&record)?;
        if !backend.has(&slot)? {
            continue;
        }
        let expected_slot = slot.clone();
        let provider_backend = Arc::clone(&backend);
        providers.push(SystemAuthUnlockProvider::new(
            &record.address,
            move |requested_slot| {
                if requested_slot != &expected_slot {
                    return SystemAuthRelease::Failed(
                        "system-auth enrollment is stale for the current signer envelope"
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
    }

    impl FakeBackend {
        fn enroll_value(&self, slot: &SystemAuthSlot, unlock_key: &[u8]) {
            self.values
                .lock()
                .unwrap()
                .insert(slot.storage_id(), unlock_key.to_vec());
        }
    }

    impl SystemAuthBackend for FakeBackend {
        fn available(&self) -> bool {
            true
        }

        fn has(&self, slot: &SystemAuthSlot) -> Result<bool, String> {
            Ok(self.values.lock().unwrap().contains_key(&slot.storage_id()))
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
            "fresnica-terminal-system-auth-{}-{}",
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
    fn noninteractive_invocation_never_activates_system_auth() {
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
    fn enrollment_and_removal_require_the_fresh_fresnica_passphrase() {
        let (client, root) = client();
        let record =
            wallet_ops::import_secret_record("wallet", "testnet", SECRET, PASSCODE).unwrap();
        client.storage().save(&record, false).unwrap();
        let backend = Arc::new(FakeBackend::default());
        let slot = system_auth_slot(&record).unwrap();

        assert!(enable_with_passphrase(&record, backend.as_ref(), "wrong passphrase").is_err());
        assert!(!backend.has(&slot).unwrap());

        enable_with_passphrase(&record, backend.as_ref(), PASSCODE).unwrap();
        assert!(backend.has(&slot).unwrap());

        assert!(disable_with_passphrase(&record, backend.as_ref(), "wrong passphrase").is_err());
        assert!(backend.has(&slot).unwrap());

        disable_with_passphrase(&record, backend.as_ref(), PASSCODE).unwrap();
        assert!(!backend.has(&slot).unwrap());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn changed_envelope_does_not_reuse_old_enrollment() {
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
