use std::collections::BTreeSet;
use std::io::{self, IsTerminal, Write};
use std::sync::{Arc, Mutex};

use fresnica_client::{
    prepare_system_auth_enrollment, system_auth_slot, verify_passcode, FresnicaClient,
    LedgerAuthorizationSnapshot, LedgerSignerAvailability, SystemAuthRelease, SystemAuthSlot,
    SystemAuthUnlockProvider, WalletStorage,
};

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DeviceUnlockState {
    Unavailable,
    Disabled,
    Locked,
    Ready,
}

pub(crate) trait DeviceUnlockBackend: Send + Sync {
    fn provider_name(&self) -> &'static str;
    fn state(&self, slot: &SystemAuthSlot) -> Result<DeviceUnlockState, String>;
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
    crate::device_unlock_linux::backend()
}
#[cfg(target_os = "macos")]
fn default_backend() -> Arc<dyn DeviceUnlockBackend> {
    crate::device_unlock_macos::backend()
}

#[cfg(target_os = "windows")]
fn default_backend() -> Arc<dyn DeviceUnlockBackend> {
    crate::device_unlock_windows::backend()
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
    match backend.state(&slot)? {
        DeviceUnlockState::Unavailable => {
            return Err("device unlock is unavailable on this platform".to_owned())
        }
        DeviceUnlockState::Locked | DeviceUnlockState::Ready => {
            return Err(format!(
                "device unlock is already enabled for wallet \"{}\"",
                record.name
            ))
        }
        DeviceUnlockState::Disabled => {}
    }
    let passphrase = crate::prompt_hidden("Fresnica passphrase: ")?;
    enable_with_passphrase(&record, backend.as_ref(), &passphrase)?;
    println!(
        "Device unlock enabled for wallet \"{}\" on this device.",
        record.name
    );
    Ok(())
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
        DeviceUnlockState::Locked | DeviceUnlockState::Ready => {}
    }
    let passphrase = crate::prompt_hidden("Fresnica passphrase: ")?;
    disable_with_passphrase(&record, backend.as_ref(), &passphrase)?;
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
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DeviceUnlockChoice {
    UseDevice,
    UsePassphrase,
    Cancel,
}

fn parse_device_unlock_choice(answer: &str) -> Option<DeviceUnlockChoice> {
    match answer.trim().to_ascii_lowercase().as_str() {
        "" | "y" | "yes" => Some(DeviceUnlockChoice::UseDevice),
        "p" | "passphrase" => Some(DeviceUnlockChoice::UsePassphrase),
        "c" | "cancel" | "n" | "no" => Some(DeviceUnlockChoice::Cancel),
        _ => None,
    }
}

fn prompt_device_unlock_choice(
    state: DeviceUnlockState,
    provider_name: &str,
) -> Result<DeviceUnlockChoice, String> {
    println!("Device unlock: {} ({provider_name})", state_label(state));
    loop {
        let prompt = match state {
            DeviceUnlockState::Ready => {
                "Sign with device unlock? [Y]es / [p]assphrase / [c]ancel: "
            }
            DeviceUnlockState::Locked => {
                "Unlock this device to sign? [Y]es / [p]assphrase / [c]ancel: "
            }
            DeviceUnlockState::Disabled | DeviceUnlockState::Unavailable => {
                return Ok(DeviceUnlockChoice::UsePassphrase)
            }
        };
        print!("{prompt}");
        io::stdout()
            .flush()
            .map_err(|error| format!("unable to write device unlock prompt: {error}"))?;
        let mut answer = String::new();
        let read = io::stdin()
            .read_line(&mut answer)
            .map_err(|error| format!("unable to read device unlock choice: {error}"))?;
        if read == 0 {
            return Ok(DeviceUnlockChoice::Cancel);
        }
        if let Some(choice) = parse_device_unlock_choice(&answer) {
            return Ok(choice);
        }
        println!("Enter y, p, or c.");
    }
}
pub(crate) fn one_shot_providers(
    client: &FresnicaClient,
) -> Result<Vec<SystemAuthUnlockProvider>, String> {
    providers_for_backend(
        client,
        default_backend(),
        io::stdin().is_terminal(),
        None,
        None,
    )
}

pub(crate) fn one_shot_providers_with_choice(
    client: &FresnicaClient,
    authorization: &LedgerAuthorizationSnapshot,
    choice: DeviceUnlockChoice,
) -> Result<Vec<SystemAuthUnlockProvider>, String> {
    let local_keys = authorization_local_keys(authorization);
    providers_for_backend(
        client,
        default_backend(),
        io::stdin().is_terminal(),
        Some(choice),
        Some(&local_keys),
    )
}

pub(crate) fn transaction_choice(
    client: &FresnicaClient,
    authorization: &LedgerAuthorizationSnapshot,
    assume_yes: bool,
) -> Result<Option<DeviceUnlockChoice>, String> {
    if !io::stdin().is_terminal() {
        return Ok(None);
    }
    let local_keys = authorization_local_keys(authorization);
    if local_keys.is_empty() {
        return Ok(None);
    }
    let backend = default_backend();
    let mut state = None;
    for record in client.wallets()? {
        if !local_keys.contains(&record.address) || record.watch_only() || record.secret.is_none() {
            continue;
        }
        match backend.state(&system_auth_slot(&record)?)? {
            DeviceUnlockState::Ready => {
                state = Some(DeviceUnlockState::Ready);
                break;
            }
            DeviceUnlockState::Locked => state = Some(DeviceUnlockState::Locked),
            DeviceUnlockState::Disabled | DeviceUnlockState::Unavailable => {}
        }
    }
    let Some(state) = state else {
        return Ok(None);
    };
    println!(
        "Device unlock: {} ({})",
        state_label(state),
        backend.provider_name()
    );
    if assume_yes {
        return Ok(Some(DeviceUnlockChoice::UseDevice));
    }
    let prompt = match state {
        DeviceUnlockState::Ready => {
            "[Enter] Sign and submit / [p] Fresnica Passphrase / [c] Cancel: "
        }
        DeviceUnlockState::Locked => {
            "[Enter] Unlock, sign and submit / [p] Fresnica Passphrase / [c] Cancel: "
        }
        DeviceUnlockState::Disabled | DeviceUnlockState::Unavailable => unreachable!(),
    };
    loop {
        print!("{prompt}");
        io::stdout()
            .flush()
            .map_err(|error| format!("unable to write device unlock prompt: {error}"))?;
        let mut answer = String::new();
        let read = io::stdin()
            .read_line(&mut answer)
            .map_err(|error| format!("unable to read device unlock choice: {error}"))?;
        if read == 0 {
            return Ok(Some(DeviceUnlockChoice::Cancel));
        }
        if let Some(choice) = parse_device_unlock_choice(&answer) {
            return Ok(Some(choice));
        }
        println!("Enter p or c, or press Enter to use device unlock.");
    }
}

fn authorization_local_keys(authorization: &LedgerAuthorizationSnapshot) -> BTreeSet<String> {
    let account_keys = authorization.accounts.iter().flat_map(|account| {
        account
            .signers
            .iter()
            .filter(|signer| signer.availability == LedgerSignerAvailability::LocalEd25519)
            .map(|signer| signer.condition.key.clone())
    });
    let extra_keys = authorization
        .extra_signers
        .iter()
        .filter(|signer| signer.availability == LedgerSignerAvailability::LocalEd25519)
        .map(|signer| signer.condition.key.clone());
    account_keys.chain(extra_keys).collect()
}

fn providers_for_backend(
    client: &FresnicaClient,
    backend: Arc<dyn DeviceUnlockBackend>,
    interactive: bool,
    preset_choice: Option<DeviceUnlockChoice>,
    allowed_keys: Option<&BTreeSet<String>>,
) -> Result<Vec<SystemAuthUnlockProvider>, String> {
    if !interactive {
        return Ok(Vec::new());
    }

    let mut providers = Vec::new();
    let shared_choice = Arc::new(Mutex::new(preset_choice));
    for record in client.wallets()? {
        if record.watch_only() || record.secret.is_none() {
            continue;
        }
        if let Some(allowed_keys) = allowed_keys {
            if !allowed_keys.contains(&record.address) {
                continue;
            }
        }
        let slot = system_auth_slot(&record)?;
        if allowed_keys.is_none()
            && !matches!(
                backend.state(&slot)?,
                DeviceUnlockState::Locked | DeviceUnlockState::Ready
            )
        {
            continue;
        }
        let expected_slot = slot.clone();
        let provider_backend = Arc::clone(&backend);
        let provider_choice = Arc::clone(&shared_choice);
        providers.push(SystemAuthUnlockProvider::new(
            &record.address,
            move |requested_slot| {
                if requested_slot != &expected_slot {
                    return SystemAuthRelease::Failed(
                        "device-unlock enrollment is stale for the current signer envelope"
                            .to_owned(),
                    );
                }
                let preset = provider_choice.lock().ok().and_then(|choice| *choice);
                let (state, choice) = if let Some(choice) = preset {
                    (None, choice)
                } else {
                    let state = match provider_backend.state(requested_slot) {
                        Ok(DeviceUnlockState::Ready) => DeviceUnlockState::Ready,
                        Ok(DeviceUnlockState::Locked) => DeviceUnlockState::Locked,
                        Ok(DeviceUnlockState::Disabled | DeviceUnlockState::Unavailable) => {
                            return SystemAuthRelease::PassphraseRequired
                        }
                        Err(error) => return SystemAuthRelease::Failed(error),
                    };
                    let selected = match prompt_device_unlock_choice(
                        state,
                        provider_backend.provider_name(),
                    ) {
                        Ok(selected) => selected,
                        Err(error) => return SystemAuthRelease::Failed(error),
                    };
                    match provider_choice.lock() {
                        Ok(mut choice) => *choice = Some(selected),
                        Err(_) => {
                            return SystemAuthRelease::Failed(
                                "device unlock choice state is unavailable".to_owned(),
                            )
                        }
                    }
                    (Some(state), selected)
                };
                match choice {
                    DeviceUnlockChoice::UseDevice => {
                        if state == Some(DeviceUnlockState::Locked) {
                            println!("Requesting {} unlock...", provider_backend.provider_name());
                        }
                        provider_backend.release(requested_slot)
                    }
                    DeviceUnlockChoice::UsePassphrase => SystemAuthRelease::PassphraseRequired,
                    DeviceUnlockChoice::Cancel => SystemAuthRelease::Cancelled,
                }
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
    fn enrolled_exact_signer_becomes_one_shot_provider() {
        let (client, root) = client();
        let record =
            wallet_ops::import_secret_record("wallet", "testnet", SECRET, PASSCODE).unwrap();
        client.storage().save(&record, false).unwrap();
        let enrollment = prepare_system_auth_enrollment(&record, PASSCODE).unwrap();
        let backend = Arc::new(FakeBackend::default());
        backend.enroll_value(&enrollment.slot, enrollment.unlock_key());

        let providers = providers_for_backend(&client, backend, true, None, None).unwrap();
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

        assert!(providers_for_backend(&client, backend, false, None, None)
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
    fn device_unlock_choice_requires_explicit_supported_input() {
        assert_eq!(
            parse_device_unlock_choice(""),
            Some(DeviceUnlockChoice::UseDevice)
        );
        assert_eq!(
            parse_device_unlock_choice("p"),
            Some(DeviceUnlockChoice::UsePassphrase)
        );
        assert_eq!(
            parse_device_unlock_choice("cancel"),
            Some(DeviceUnlockChoice::Cancel)
        );
        assert_eq!(parse_device_unlock_choice("maybe"), None);
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

        assert!(providers_for_backend(&client, backend, true, None, None)
            .unwrap()
            .is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }
}
