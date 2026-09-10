use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use fresnica_client::{SystemAuthRelease, SystemAuthSlot, SYSTEM_AUTH_UNLOCK_KEY_LENGTH};
use secret_service::blocking::{Collection, SecretService};
use secret_service::{EncryptionType, Error as SecretServiceError};
use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::{OwnedObjectPath, OwnedValue};
use zeroize::Zeroize;

use crate::device_unlock::{
    DeviceAuthenticationOutcome, DeviceAuthenticator, DeviceSecretRead, DeviceSecretStore,
    DeviceUnlockBackend, DeviceUnlockState,
};

const PROVIDER_NAME: &str = "Fresnica Secret Service collection";
const COLLECTION_LABEL: &str = "Fresnica Device Unlock";
const COLLECTION_ALIAS: &str = "fresnica-device-unlock";
const ITEM_LABEL: &str = "Fresnica Device Unlock";
const CONTENT_TYPE: &str = "application/octet-stream";
const SECRET_SERVICE_NAME: &str = "org.freedesktop.secrets";
const SECRET_SERVICE_PATH: &str = "/org/freedesktop/secrets";
const SECRET_SERVICE_INTERFACE: &str = "org.freedesktop.Secret.Service";
const SECRET_PROMPT_INTERFACE: &str = "org.freedesktop.Secret.Prompt";

pub(crate) fn backend() -> Arc<dyn DeviceUnlockBackend> {
    Arc::new(LinuxDeviceUnlockBackend {
        authenticator: LinuxSecretServiceAuthenticator {
            authenticated: Mutex::new(false),
        },
        store: LinuxSecretServiceStore,
    })
}

struct LinuxDeviceUnlockBackend {
    authenticator: LinuxSecretServiceAuthenticator,
    store: LinuxSecretServiceStore,
}

struct LinuxSecretServiceAuthenticator {
    authenticated: Mutex<bool>,
}

struct LinuxSecretServiceStore;

impl Drop for LinuxDeviceUnlockBackend {
    fn drop(&mut self) {
        let _ = relock_collection();
    }
}

impl DeviceUnlockBackend for LinuxDeviceUnlockBackend {
    fn provider_name(&self) -> &'static str {
        PROVIDER_NAME
    }

    fn state(&self, slot: &SystemAuthSlot) -> Result<DeviceUnlockState, String> {
        self.store.state(slot)
    }

    fn authorize_enrollment(&self) -> Result<(), String> {
        ensure_dedicated_collection()?;
        match self.authenticator.authenticate()? {
            DeviceAuthenticationOutcome::Authenticated => Ok(()),
            DeviceAuthenticationOutcome::Cancelled => {
                Err("desktop authentication cancelled; Device Unlock was not enabled".to_owned())
            }
            DeviceAuthenticationOutcome::PassphraseRequired => Err(
                "desktop authentication did not provide a usable prompt; Device Unlock was not enabled"
                    .to_owned(),
            ),
        }
    }

    fn enroll(&self, slot: &SystemAuthSlot, unlock_key: &[u8]) -> Result<(), String> {
        self.store.enroll(slot, unlock_key)
    }

    fn release(&self, slot: &SystemAuthSlot) -> SystemAuthRelease {
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

impl DeviceAuthenticator for LinuxSecretServiceAuthenticator {
    fn authenticate(&self) -> Result<DeviceAuthenticationOutcome, String> {
        let mut authenticated = self
            .authenticated
            .lock()
            .map_err(|_| "desktop device authentication state is unavailable".to_owned())?;
        if *authenticated {
            return Ok(DeviceAuthenticationOutcome::Authenticated);
        }

        let service = match connect() {
            Ok(service) => service,
            Err(_) => return Ok(DeviceAuthenticationOutcome::PassphraseRequired),
        };
        let collection = match service.get_collection_by_alias(COLLECTION_ALIAS) {
            Ok(collection) => collection,
            Err(SecretServiceError::NoResult) => {
                return Ok(DeviceAuthenticationOutcome::PassphraseRequired)
            }
            Err(error) => {
                return Err(format!(
                    "unable to open Fresnica device-unlock collection: {error}"
                ))
            }
        };

        if !collection
            .is_locked()
            .map_err(|error| format!("unable to query Fresnica keyring state: {error}"))?
            && !lock_without_prompt(&collection.collection_path)?
        {
            eprintln!(
                "Desktop authentication cannot relock the Fresnica keyring without a prompt; Fresnica Passphrase required."
            );
            return Ok(DeviceAuthenticationOutcome::PassphraseRequired);
        }
        if !collection
            .is_locked()
            .map_err(|error| format!("unable to verify Fresnica keyring state: {error}"))?
        {
            return Err("desktop secret service did not lock the Fresnica keyring".to_owned());
        }

        let outcome = unlock_with_required_prompt(&collection.collection_path)?;
        if outcome == DeviceAuthenticationOutcome::PassphraseRequired {
            eprintln!(
                "Desktop authentication did not provide a user prompt; Fresnica Passphrase required."
            );
        }
        if outcome != DeviceAuthenticationOutcome::Authenticated {
            if !collection
                .is_locked()
                .map_err(|error| format!("unable to query Fresnica keyring state: {error}"))?
                && !lock_without_prompt(&collection.collection_path)?
            {
                return Err(
                    "desktop secret service could not relock the Fresnica keyring without a prompt"
                        .to_owned(),
                );
            }
            return Ok(outcome);
        }
        if collection
            .is_locked()
            .map_err(|error| format!("unable to verify Fresnica keyring state: {error}"))?
        {
            return Err("desktop secret service left the Fresnica keyring locked".to_owned());
        }

        *authenticated = true;
        Ok(DeviceAuthenticationOutcome::Authenticated)
    }
}

impl DeviceSecretStore for LinuxSecretServiceStore {
    fn state(&self, slot: &SystemAuthSlot) -> Result<DeviceUnlockState, String> {
        let service = match connect() {
            Ok(service) => service,
            Err(_) => return Ok(DeviceUnlockState::Unavailable),
        };
        let collection = match service.get_collection_by_alias(COLLECTION_ALIAS) {
            Ok(collection) => collection,
            Err(SecretServiceError::NoResult) => return Ok(DeviceUnlockState::Disabled),
            Err(_) => return Ok(DeviceUnlockState::Unavailable),
        };
        let slot_id = slot.storage_id();
        let items = collection
            .search_items(attributes(&slot_id))
            .map_err(|error| format!("unable to query Fresnica keyring: {error}"))?;
        if items.is_empty() {
            return Ok(DeviceUnlockState::Disabled);
        }
        if collection
            .is_locked()
            .map_err(|error| format!("unable to query Fresnica keyring state: {error}"))?
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
        let service = connect()?;
        let collection = match service.get_collection_by_alias(COLLECTION_ALIAS) {
            Ok(collection) => collection,
            Err(SecretServiceError::NoResult) => service
                .create_collection(COLLECTION_LABEL, COLLECTION_ALIAS)
                .map_err(map_collection_create_error)?,
            Err(error) => {
                return Err(format!(
                    "unable to open Fresnica device-unlock collection: {error}"
                ))
            }
        };
        if collection
            .is_locked()
            .map_err(|error| format!("unable to query Fresnica keyring state: {error}"))?
        {
            collection.unlock().map_err(map_interactive_error)?;
        }
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
        delete_legacy_default_items(&service, &collection, &slot_id)?;
        if !lock_without_prompt(&collection.collection_path)? {
            return Err(
                "desktop secret service could not lock the Fresnica keyring without a prompt"
                    .to_owned(),
            );
        }
        Ok(())
    }

    fn read(&self, slot: &SystemAuthSlot) -> Result<DeviceSecretRead, String> {
        let service = connect()?;
        let collection = match service.get_collection_by_alias(COLLECTION_ALIAS) {
            Ok(collection) => collection,
            Err(SecretServiceError::NoResult) => return Ok(DeviceSecretRead::Missing),
            Err(error) => {
                return Err(format!(
                    "unable to open Fresnica device-unlock collection: {error}"
                ))
            }
        };
        if collection
            .is_locked()
            .map_err(|error| format!("unable to query Fresnica keyring state: {error}"))?
        {
            return Err("Fresnica keyring is locked after device authentication".to_owned());
        }
        let slot_id = slot.storage_id();
        let mut items = collection
            .search_items(attributes(&slot_id))
            .map_err(|error| format!("unable to query Fresnica keyring: {error}"))?;
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
        let service = connect()?;
        let slot_id = slot.storage_id();
        if let Ok(collection) = service.get_collection_by_alias(COLLECTION_ALIAS) {
            if collection
                .is_locked()
                .map_err(|error| format!("unable to query Fresnica keyring state: {error}"))?
            {
                collection.unlock().map_err(map_interactive_error)?;
            }
            for item in collection
                .search_items(attributes(&slot_id))
                .map_err(|error| format!("unable to query Fresnica keyring: {error}"))?
            {
                item.delete()
                    .map_err(|error| format!("unable to remove device unlock key: {error}"))?;
            }
            collection
                .lock()
                .map_err(|error| format!("unable to lock Fresnica keyring: {error}"))?;
        }
        delete_legacy_default_items_without_dedicated(&service, &slot_id)
    }
}

fn lock_without_prompt(collection_path: &OwnedObjectPath) -> Result<bool, String> {
    let connection = Connection::session()
        .map_err(|error| format!("unable to connect to desktop session bus: {error}"))?;
    let service = Proxy::new(
        &connection,
        SECRET_SERVICE_NAME,
        SECRET_SERVICE_PATH,
        SECRET_SERVICE_INTERFACE,
    )
    .map_err(|error| format!("unable to open Secret Service interface: {error}"))?;
    let objects = vec![collection_path.clone()];
    let (locked, prompt): (Vec<OwnedObjectPath>, OwnedObjectPath) = service
        .call("Lock", &objects)
        .map_err(|error| format!("unable to request Fresnica keyring lock: {error}"))?;
    Ok(!locked.is_empty() && prompt.as_str() == "/")
}

fn unlock_with_required_prompt(
    collection_path: &OwnedObjectPath,
) -> Result<DeviceAuthenticationOutcome, String> {
    let connection = Connection::session()
        .map_err(|error| format!("unable to connect to desktop session bus: {error}"))?;
    let service = Proxy::new(
        &connection,
        SECRET_SERVICE_NAME,
        SECRET_SERVICE_PATH,
        SECRET_SERVICE_INTERFACE,
    )
    .map_err(|error| format!("unable to open Secret Service interface: {error}"))?;
    let objects = vec![collection_path.clone()];
    let (unlocked, prompt): (Vec<OwnedObjectPath>, OwnedObjectPath) = service
        .call("Unlock", &objects)
        .map_err(|error| format!("unable to request Fresnica keyring unlock: {error}"))?;

    if !unlocked.is_empty() {
        return Ok(DeviceAuthenticationOutcome::PassphraseRequired);
    }
    if prompt.as_str() == "/" {
        return Err(
            "desktop secret service neither prompted nor unlocked the Fresnica keyring".to_owned(),
        );
    }

    let prompt_proxy = Proxy::new(
        &connection,
        SECRET_SERVICE_NAME,
        prompt.as_str(),
        SECRET_PROMPT_INTERFACE,
    )
    .map_err(|error| format!("unable to open Secret Service prompt: {error}"))?;
    let mut completed = prompt_proxy
        .receive_signal("Completed")
        .map_err(|error| format!("unable to receive Secret Service prompt result: {error}"))?;
    let _: () = prompt_proxy
        .call("Prompt", &"")
        .map_err(|error| format!("unable to display Secret Service prompt: {error}"))?;
    let message = completed.next().ok_or_else(|| {
        "desktop secret service prompt disconnected before authentication completed".to_owned()
    })?;
    let (dismissed, _result): (bool, OwnedValue) = message
        .body()
        .deserialize()
        .map_err(|error| format!("unable to decode Secret Service prompt result: {error}"))?;
    if dismissed {
        Ok(DeviceAuthenticationOutcome::Cancelled)
    } else {
        Ok(DeviceAuthenticationOutcome::Authenticated)
    }
}

fn connect() -> Result<SecretService<'static>, String> {
    SecretService::connect(EncryptionType::Dh)
        .map_err(|error| format!("desktop secret service is unavailable: {error}"))
}

fn ensure_dedicated_collection() -> Result<(), String> {
    let service = connect()?;
    let result = match service.get_collection_by_alias(COLLECTION_ALIAS) {
        Ok(_) => Ok(()),
        Err(SecretServiceError::NoResult) => service
            .create_collection(COLLECTION_LABEL, COLLECTION_ALIAS)
            .map(|_| ())
            .map_err(map_collection_create_error),
        Err(error) => Err(format!(
            "unable to open Fresnica device-unlock collection: {error}"
        )),
    };
    result
}

fn relock_collection() -> Result<(), String> {
    let service = connect()?;
    let collection = match service.get_collection_by_alias(COLLECTION_ALIAS) {
        Ok(collection) => collection,
        Err(SecretServiceError::NoResult) => return Ok(()),
        Err(error) => return Err(format!("unable to open Fresnica keyring: {error}")),
    };
    if !collection
        .is_locked()
        .map_err(|error| format!("unable to query Fresnica keyring state: {error}"))?
        && !lock_without_prompt(&collection.collection_path)?
    {
        return Err(
            "desktop secret service could not relock the Fresnica keyring without a prompt"
                .to_owned(),
        );
    }
    Ok(())
}

fn delete_legacy_default_items(
    service: &SecretService<'_>,
    dedicated: &Collection<'_>,
    slot_id: &str,
) -> Result<(), String> {
    let default = match service.get_default_collection() {
        Ok(collection) => collection,
        Err(_) => return Ok(()),
    };
    if default.collection_path == dedicated.collection_path {
        return Ok(());
    }
    delete_items_from_collection(default, slot_id)
}

fn delete_legacy_default_items_without_dedicated(
    service: &SecretService<'_>,
    slot_id: &str,
) -> Result<(), String> {
    let default = match service.get_default_collection() {
        Ok(collection) => collection,
        Err(_) => return Ok(()),
    };
    delete_items_from_collection(default, slot_id)
}

fn delete_items_from_collection(collection: Collection<'_>, slot_id: &str) -> Result<(), String> {
    if collection
        .is_locked()
        .map_err(|error| format!("unable to query desktop keyring state: {error}"))?
    {
        collection.unlock().map_err(map_interactive_error)?;
    }
    for item in collection
        .search_items(attributes(slot_id))
        .map_err(|error| format!("unable to query desktop keyring: {error}"))?
    {
        item.delete()
            .map_err(|error| format!("unable to remove legacy device unlock key: {error}"))?;
    }
    Ok(())
}

fn attributes(slot_id: &str) -> HashMap<&str, &str> {
    HashMap::from([
        ("application", "fresnica"),
        ("purpose", "device-unlock"),
        ("slot", slot_id),
    ])
}

fn map_collection_create_error(error: SecretServiceError) -> String {
    match error {
        SecretServiceError::Prompt => "device unlock enrollment cancelled".to_owned(),
        other => format!("unable to create Fresnica keyring: {other}"),
    }
}

fn map_interactive_error(error: SecretServiceError) -> String {
    match error {
        SecretServiceError::Prompt => "device unlock cancelled".to_owned(),
        other => format!("unable to unlock Fresnica keyring: {other}"),
    }
}
