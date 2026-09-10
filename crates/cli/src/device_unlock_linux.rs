use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use fresnica_client::{SystemAuthRelease, SystemAuthSlot, SYSTEM_AUTH_UNLOCK_KEY_LENGTH};
use secret_service::blocking::SecretService;
use secret_service::EncryptionType;
use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::OwnedObjectPath;
use zeroize::Zeroize;

use crate::device_unlock::{
    DeviceAuthenticationOutcome, DeviceAuthenticator, DeviceSecretRead, DeviceSecretStore,
    DeviceUnlockBackend, DeviceUnlockState,
};

const PROVIDER_NAME: &str = "Linux fingerprint";
const ITEM_LABEL: &str = "Fresnica Device Unlock";
const CONTENT_TYPE: &str = "application/octet-stream";
const FPRINT_SERVICE: &str = "net.reactivated.Fprint";
const FPRINT_MANAGER_PATH: &str = "/net/reactivated/Fprint/Manager";
const FPRINT_MANAGER_INTERFACE: &str = "net.reactivated.Fprint.Manager";
const FPRINT_DEVICE_INTERFACE: &str = "net.reactivated.Fprint.Device";
const FINGERPRINT_MAX_TRIES: u8 = 3;
const TRANSACTION_AUTH_PROMPT: &str =
    "Touch the fingerprint sensor to authenticate this Fresnica transaction.";
const ENROLLMENT_AUTH_PROMPT: &str =
    "Touch the fingerprint sensor to enable Fresnica Device Unlock.";

pub(crate) fn backend() -> Arc<dyn DeviceUnlockBackend> {
    Arc::new(LinuxDeviceUnlockBackend {
        authenticator: LinuxFingerprintAuthenticator {
            authenticated: Mutex::new(false),
        },
        store: LinuxSecretServiceStore,
    })
}

struct LinuxDeviceUnlockBackend {
    authenticator: LinuxFingerprintAuthenticator,
    store: LinuxSecretServiceStore,
}

struct LinuxFingerprintAuthenticator {
    authenticated: Mutex<bool>,
}

struct LinuxSecretServiceStore;

impl DeviceUnlockBackend for LinuxDeviceUnlockBackend {
    fn provider_name(&self) -> &'static str {
        PROVIDER_NAME
    }

    fn state(&self, slot: &SystemAuthSlot) -> Result<DeviceUnlockState, String> {
        self.store.state(slot)
    }

    fn authorize_enrollment(&self) -> Result<(), String> {
        self.store.ensure_unlocked()?;
        match self.authenticator.verify(ENROLLMENT_AUTH_PROMPT)? {
            FingerprintVerificationOutcome::Verified => Ok(()),
            FingerprintVerificationOutcome::Rejected => {
                Err("fingerprint did not match; Device Unlock was not enabled".to_owned())
            }
            FingerprintVerificationOutcome::Unavailable(reason) => Err(format!(
                "Linux fingerprint authentication unavailable ({reason}); Device Unlock was not enabled"
            )),
        }
    }

    fn enroll(&self, slot: &SystemAuthSlot, unlock_key: &[u8]) -> Result<(), String> {
        self.store.enroll(slot, unlock_key)
    }

    fn release(&self, slot: &SystemAuthSlot) -> SystemAuthRelease {
        match self.store.state(slot) {
            Ok(DeviceUnlockState::Ready) => {}
            Ok(DeviceUnlockState::Locked) => {
                eprintln!("Desktop keyring is locked; Fresnica Passphrase required.");
                return SystemAuthRelease::PassphraseRequired;
            }
            Ok(DeviceUnlockState::Disabled | DeviceUnlockState::Unavailable) => {
                return SystemAuthRelease::PassphraseRequired
            }
            Err(error) => return SystemAuthRelease::Failed(error),
        }
        match self.authenticator.authenticate() {
            Ok(DeviceAuthenticationOutcome::Authenticated) => {}
            Ok(DeviceAuthenticationOutcome::Cancelled) => return SystemAuthRelease::Cancelled,
            Ok(DeviceAuthenticationOutcome::PassphraseRequired) => {
                return SystemAuthRelease::PassphraseRequired
            }
            Err(error) => return SystemAuthRelease::Failed(error),
        }
        match self.store.read(slot) {
            Ok(DeviceSecretRead::Secret(key)) => SystemAuthRelease::UnlockKey(key),
            Ok(DeviceSecretRead::Missing) => SystemAuthRelease::PassphraseRequired,
            Ok(DeviceSecretRead::Cancelled) => SystemAuthRelease::Cancelled,
            Err(error) => SystemAuthRelease::Failed(error),
        }
    }

    fn delete(&self, slot: &SystemAuthSlot) -> Result<(), String> {
        self.store.delete(slot)
    }
}

#[derive(Debug, PartialEq, Eq)]
enum FingerprintVerificationOutcome {
    Verified,
    Rejected,
    Unavailable(String),
}

impl LinuxFingerprintAuthenticator {
    fn verify(&self, prompt: &str) -> Result<FingerprintVerificationOutcome, String> {
        let mut authenticated = self
            .authenticated
            .lock()
            .map_err(|_| "Linux fingerprint authentication state is unavailable".to_owned())?;
        if *authenticated {
            return Ok(FingerprintVerificationOutcome::Verified);
        }

        let outcome = verify_current_user_fingerprint(prompt)?;
        if outcome == FingerprintVerificationOutcome::Verified {
            *authenticated = true;
        }
        Ok(outcome)
    }
}

impl DeviceAuthenticator for LinuxFingerprintAuthenticator {
    fn authenticate(&self) -> Result<DeviceAuthenticationOutcome, String> {
        match self.verify(TRANSACTION_AUTH_PROMPT)? {
            FingerprintVerificationOutcome::Verified => {
                Ok(DeviceAuthenticationOutcome::Authenticated)
            }
            FingerprintVerificationOutcome::Rejected => {
                Err("fingerprint did not match; transaction was not authorized".to_owned())
            }
            FingerprintVerificationOutcome::Unavailable(reason) => {
                eprintln!(
                    "Linux fingerprint authentication unavailable ({reason}); Fresnica Passphrase required."
                );
                Ok(DeviceAuthenticationOutcome::PassphraseRequired)
            }
        }
    }
}

fn verify_current_user_fingerprint(prompt: &str) -> Result<FingerprintVerificationOutcome, String> {
    let connection = match Connection::system() {
        Ok(connection) => connection,
        Err(error) => {
            return Ok(FingerprintVerificationOutcome::Unavailable(format!(
                "unable to connect to system bus: {error}"
            )))
        }
    };
    let manager = Proxy::new(
        &connection,
        FPRINT_SERVICE,
        FPRINT_MANAGER_PATH,
        FPRINT_MANAGER_INTERFACE,
    )
    .map_err(|error| format!("unable to open fprintd manager interface: {error}"))?;
    let device_path: OwnedObjectPath = match manager.call("GetDefaultDevice", &()) {
        Ok(path) => path,
        Err(error) => {
            return Ok(FingerprintVerificationOutcome::Unavailable(format!(
                "no usable fingerprint reader: {error}"
            )))
        }
    };
    let device = Proxy::new(
        &connection,
        FPRINT_SERVICE,
        device_path.as_str(),
        FPRINT_DEVICE_INTERFACE,
    )
    .map_err(|error| format!("unable to open fprintd device interface: {error}"))?;
    let mut statuses = device
        .receive_signal("VerifyStatus")
        .map_err(|error| format!("unable to receive fingerprint verification status: {error}"))?;

    if let Err(error) = device.call::<_, _, ()>("Claim", &"") {
        return Ok(FingerprintVerificationOutcome::Unavailable(format!(
            "unable to claim fingerprint reader: {error}"
        )));
    }

    let mut outcome = FingerprintVerificationOutcome::Rejected;
    for attempt in 1..=FINGERPRINT_MAX_TRIES {
        match device.call::<_, _, ()>("VerifyStart", &"any") {
            Ok(()) => {}
            Err(error) => {
                outcome = FingerprintVerificationOutcome::Unavailable(format!(
                    "unable to start fingerprint verification: {error}"
                ));
                break;
            }
        }

        if attempt == 1 {
            eprintln!("{prompt}");
        } else {
            eprintln!("Fingerprint did not match; try again ({attempt}/{FINGERPRINT_MAX_TRIES}).");
        }
        let attempt_outcome = fingerprint_status_loop(&mut statuses);
        let _ = device.call::<_, _, ()>("VerifyStop", &());
        let attempt_outcome = match attempt_outcome {
            Ok(outcome) => outcome,
            Err(error) => {
                let _ = device.call::<_, _, ()>("Release", &());
                return Err(error);
            }
        };

        match attempt_outcome {
            FingerprintVerificationOutcome::Rejected if attempt < FINGERPRINT_MAX_TRIES => continue,
            result => {
                outcome = result;
                break;
            }
        }
    }

    let _ = device.call::<_, _, ()>("Release", &());
    Ok(outcome)
}

#[derive(Debug, PartialEq, Eq)]
enum FingerprintStatus {
    Continue(&'static str),
    Complete(FingerprintVerificationOutcome),
}

fn classify_fingerprint_status(result: &str, done: bool) -> Result<FingerprintStatus, String> {
    if !done {
        return match result {
            "verify-retry-scan" => Ok(FingerprintStatus::Continue(
                "Fingerprint scan incomplete; try again.",
            )),
            "verify-swipe-too-short" => Ok(FingerprintStatus::Continue(
                "Fingerprint swipe too short; try again.",
            )),
            "verify-finger-not-centered" => Ok(FingerprintStatus::Continue(
                "Fingerprint was not centered; try again.",
            )),
            "verify-remove-and-retry" => Ok(FingerprintStatus::Continue(
                "Remove your finger from the sensor and try again.",
            )),
            "verify-too-fast" => Ok(FingerprintStatus::Continue(
                "Fingerprint scan was too fast; try again.",
            )),
            other => Err(format!(
                "fprintd returned unknown non-terminal verification status {other}"
            )),
        };
    }
    match result {
        "verify-match" => Ok(FingerprintStatus::Complete(
            FingerprintVerificationOutcome::Verified,
        )),
        "verify-no-match" => Ok(FingerprintStatus::Complete(
            FingerprintVerificationOutcome::Rejected,
        )),
        "verify-disconnected" => Ok(FingerprintStatus::Complete(
            FingerprintVerificationOutcome::Unavailable(
                "fingerprint reader disconnected".to_owned(),
            ),
        )),
        "verify-unknown-error" => Ok(FingerprintStatus::Complete(
            FingerprintVerificationOutcome::Unavailable(
                "fingerprint reader reported an unknown error".to_owned(),
            ),
        )),
        other => Err(format!(
            "fprintd returned unknown terminal verification status {other}"
        )),
    }
}

fn fingerprint_status_loop(
    statuses: &mut zbus::blocking::proxy::SignalIterator<'_>,
) -> Result<FingerprintVerificationOutcome, String> {
    loop {
        let message = statuses.next().ok_or_else(|| {
            "fprintd disconnected before fingerprint verification completed".to_owned()
        })?;
        let (result, done): (String, bool) = message.body().deserialize().map_err(|error| {
            format!("unable to decode fingerprint verification status: {error}")
        })?;
        match classify_fingerprint_status(&result, done)? {
            FingerprintStatus::Continue(message) => eprintln!("{message}"),
            FingerprintStatus::Complete(outcome) => return Ok(outcome),
        }
    }
}

impl LinuxSecretServiceStore {
    fn ensure_unlocked(&self) -> Result<(), String> {
        let service = connect_secret_service()?;
        let collection = service
            .get_default_collection()
            .map_err(|error| format!("unable to open desktop default keyring: {error}"))?;
        if collection
            .is_locked()
            .map_err(|error| format!("unable to query desktop keyring state: {error}"))?
        {
            return Err(
                "desktop default keyring is locked; unlock it in the desktop session and retry"
                    .to_owned(),
            );
        }
        Ok(())
    }
}

impl DeviceSecretStore for LinuxSecretServiceStore {
    fn state(&self, slot: &SystemAuthSlot) -> Result<DeviceUnlockState, String> {
        let service = match connect_secret_service() {
            Ok(service) => service,
            Err(_) => return Ok(DeviceUnlockState::Unavailable),
        };
        let collection = match service.get_default_collection() {
            Ok(collection) => collection,
            Err(_) => return Ok(DeviceUnlockState::Unavailable),
        };
        let slot_id = slot.storage_id();
        let items = collection
            .search_items(attributes(&slot_id))
            .map_err(|error| format!("unable to query desktop keyring: {error}"))?;
        if items.is_empty() {
            return Ok(DeviceUnlockState::Disabled);
        }
        if collection
            .is_locked()
            .map_err(|error| format!("unable to query desktop keyring state: {error}"))?
        {
            Ok(DeviceUnlockState::Locked)
        } else {
            Ok(DeviceUnlockState::Ready)
        }
    }

    fn enroll(&self, slot: &SystemAuthSlot, unlock_key: &[u8]) -> Result<(), String> {
        if unlock_key.len() != SYSTEM_AUTH_UNLOCK_KEY_LENGTH {
            return Err("device unlock requires exactly 32 key bytes".to_owned());
        }
        self.ensure_unlocked()?;
        let service = connect_secret_service()?;
        let collection = service
            .get_default_collection()
            .map_err(|error| format!("unable to open desktop default keyring: {error}"))?;
        let slot_id = slot.storage_id();
        collection
            .create_item(
                ITEM_LABEL,
                attributes(&slot_id),
                unlock_key,
                true,
                CONTENT_TYPE,
            )
            .map_err(|error| format!("unable to store device unlock key: {error}"))?;
        Ok(())
    }

    fn read(&self, slot: &SystemAuthSlot) -> Result<DeviceSecretRead, String> {
        let service = connect_secret_service()?;
        let collection = service
            .get_default_collection()
            .map_err(|error| format!("unable to open desktop default keyring: {error}"))?;
        if collection
            .is_locked()
            .map_err(|error| format!("unable to query desktop keyring state: {error}"))?
        {
            return Err("desktop default keyring became locked after authentication".to_owned());
        }
        let slot_id = slot.storage_id();
        let mut items = collection
            .search_items(attributes(&slot_id))
            .map_err(|error| format!("unable to query desktop keyring: {error}"))?;
        let Some(item) = items.pop() else {
            return Ok(DeviceSecretRead::Missing);
        };
        let mut secret = item
            .get_secret()
            .map_err(|error| format!("unable to read device unlock key: {error}"))?;
        if secret.len() != SYSTEM_AUTH_UNLOCK_KEY_LENGTH {
            secret.zeroize();
            return Err("desktop secret service returned an invalid device unlock key".to_owned());
        }
        Ok(DeviceSecretRead::Secret(secret))
    }

    fn delete(&self, slot: &SystemAuthSlot) -> Result<(), String> {
        self.ensure_unlocked()?;
        let service = connect_secret_service()?;
        let collection = service
            .get_default_collection()
            .map_err(|error| format!("unable to open desktop default keyring: {error}"))?;
        let slot_id = slot.storage_id();
        for item in collection
            .search_items(attributes(&slot_id))
            .map_err(|error| format!("unable to query desktop keyring: {error}"))?
        {
            item.delete()
                .map_err(|error| format!("unable to remove device unlock key: {error}"))?;
        }
        Ok(())
    }
}

fn connect_secret_service() -> Result<SecretService<'static>, String> {
    SecretService::connect(EncryptionType::Dh)
        .map_err(|error| format!("desktop secret service is unavailable: {error}"))
}

fn attributes(slot_id: &str) -> HashMap<&str, &str> {
    HashMap::from([
        ("application", "fresnica"),
        ("purpose", "device-unlock"),
        ("slot", slot_id),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fprintd_status_mapping_is_fail_closed() {
        assert_eq!(
            classify_fingerprint_status("verify-match", true).unwrap(),
            FingerprintStatus::Complete(FingerprintVerificationOutcome::Verified)
        );
        assert_eq!(
            classify_fingerprint_status("verify-no-match", true).unwrap(),
            FingerprintStatus::Complete(FingerprintVerificationOutcome::Rejected)
        );
        assert!(matches!(
            classify_fingerprint_status("verify-retry-scan", false).unwrap(),
            FingerprintStatus::Continue(_)
        ));
        assert!(classify_fingerprint_status("unexpected", true).is_err());
        assert!(classify_fingerprint_status("unexpected", false).is_err());
    }
}
