use std::io::{self, IsTerminal};
use std::sync::Arc;

use fresnica_client::{system_auth_slot, FresnicaClient, SystemAuthSlot, SystemAuthUnlockProvider};

pub(crate) trait SystemAuthBackend: Send + Sync {
    fn available(&self) -> bool;
    fn has(&self, slot: &SystemAuthSlot) -> Result<bool, String>;
    fn release(&self, slot: &SystemAuthSlot) -> Result<Vec<u8>, String>;
}

struct UnavailableSystemAuthBackend;

impl SystemAuthBackend for UnavailableSystemAuthBackend {
    fn available(&self) -> bool {
        false
    }

    fn has(&self, _slot: &SystemAuthSlot) -> Result<bool, String> {
        Ok(false)
    }

    fn release(&self, _slot: &SystemAuthSlot) -> Result<Vec<u8>, String> {
        Err("system authentication is unavailable on this client".to_owned())
    }
}

pub(crate) fn one_shot_providers(
    client: &FresnicaClient,
) -> Result<Vec<SystemAuthUnlockProvider>, String> {
    providers_for_backend(
        client,
        Arc::new(UnavailableSystemAuthBackend),
        io::stdin().is_terminal(),
    )
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
                    return Err(
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

    use fresnica_client::{prepare_system_auth_enrollment, wallet as wallet_ops, NetworkProfile};

    use super::*;

    const SECRET: &str = "SCOWDMM5576VUYF2QRFPJEXMFTCEISOFNF5TE2IZOA52YAY4VZ7WBQNO";
    const PASSCODE: &str = "correct horse battery staple";

    #[derive(Default)]
    struct FakeBackend {
        values: Mutex<BTreeMap<String, Vec<u8>>>,
    }

    impl FakeBackend {
        fn enroll(&self, slot: &SystemAuthSlot, unlock_key: &[u8]) {
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

        fn release(&self, slot: &SystemAuthSlot) -> Result<Vec<u8>, String> {
            self.values
                .lock()
                .unwrap()
                .get(&slot.storage_id())
                .cloned()
                .ok_or_else(|| "not enrolled".to_owned())
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
        backend.enroll(&enrollment.slot, enrollment.unlock_key());

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
        backend.enroll(&enrollment.slot, enrollment.unlock_key());

        assert!(providers_for_backend(&client, backend, false)
            .unwrap()
            .is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn changed_envelope_does_not_reuse_old_enrollment() {
        let (client, root) = client();
        let record =
            wallet_ops::import_secret_record("wallet", "testnet", SECRET, PASSCODE).unwrap();
        let enrollment = prepare_system_auth_enrollment(&record, PASSCODE).unwrap();
        let backend = Arc::new(FakeBackend::default());
        backend.enroll(&enrollment.slot, enrollment.unlock_key());
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
