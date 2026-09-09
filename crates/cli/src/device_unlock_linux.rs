use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use fresnica_client::{SystemAuthRelease, SystemAuthSlot, SYSTEM_AUTH_UNLOCK_KEY_LENGTH};
use secret_service::blocking::{Collection, SecretService};
use secret_service::{EncryptionType, Error as SecretServiceError};
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
        {
            collection
                .lock()
                .map_err(|error| format!("unable to lock Fresnica keyring: {error}"))?;
        }
        if !collection
            .is_locked()
            .map_err(|error| format!("unable to verify Fresnica keyring state: {error}"))?
        {
            return Err("desktop secret service did not lock the Fresnica keyring".to_owned());
        }
        match collection.unlock() {
            Ok(()) => {}
            Err(SecretServiceError::Prompt) => {
                return Ok(DeviceAuthenticationOutcome::Cancelled)
            }
            Err(error) => {
                return Err(format!("unable to unlock Fresnica keyring: {error}"))
            }
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
        collection
            .lock()
            .map_err(|error| format!("unable to lock Fresnica keyring after enrollment: {error}"))?;
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
            return Err(
                "desktop secret service returned an invalid device unlock key".to_owned(),
            );
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

fn connect() -> Result<SecretService<'static>, String> {
    SecretService::connect(EncryptionType::Dh)
        .map_err(|error| format!("desktop secret service is unavailable: {error}"))
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
    {
        collection
            .lock()
            .map_err(|error| format!("unable to lock Fresnica keyring: {error}"))?;
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
