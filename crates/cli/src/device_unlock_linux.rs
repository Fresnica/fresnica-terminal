use std::collections::HashMap;
use std::sync::Arc;

use fresnica_client::{SystemAuthRelease, SystemAuthSlot, SYSTEM_AUTH_UNLOCK_KEY_LENGTH};
use secret_service::blocking::SecretService;
use secret_service::{EncryptionType, Error as SecretServiceError};
use zeroize::Zeroize;

use crate::device_unlock::{DeviceUnlockBackend, DeviceUnlockState};

const PROVIDER_NAME: &str = "Desktop Secret Service";
const ITEM_LABEL: &str = "Fresnica Device Unlock";
const CONTENT_TYPE: &str = "application/octet-stream";

pub(crate) fn backend() -> Arc<dyn DeviceUnlockBackend> {
    Arc::new(LinuxDeviceUnlockBackend)
}

struct LinuxDeviceUnlockBackend;

impl DeviceUnlockBackend for LinuxDeviceUnlockBackend {
    fn provider_name(&self) -> &'static str {
        PROVIDER_NAME
    }

    fn state(&self, slot: &SystemAuthSlot) -> Result<DeviceUnlockState, String> {
        let service = match connect() {
            Ok(service) => service,
            Err(_) => return Ok(DeviceUnlockState::Unavailable),
        };
        let slot_id = slot.storage_id();
        let items = service
            .search_items(attributes(&slot_id))
            .map_err(|error| format!("unable to query desktop secret service: {error}"))?;
        if !items.unlocked.is_empty() {
            Ok(DeviceUnlockState::Ready)
        } else if !items.locked.is_empty() {
            Ok(DeviceUnlockState::Locked)
        } else {
            Ok(DeviceUnlockState::Disabled)
        }
    }

    fn enroll(&self, slot: &SystemAuthSlot, unlock_key: &[u8]) -> Result<(), String> {
        if unlock_key.len() != SYSTEM_AUTH_UNLOCK_KEY_LENGTH {
            return Err("device unlock requires exactly 32 key bytes".to_owned());
        }
        let service = connect()?;
        let collection = service
            .get_default_collection()
            .map_err(|error| format!("unable to open desktop keyring: {error}"))?;
        if collection
            .is_locked()
            .map_err(|error| format!("unable to query desktop keyring state: {error}"))?
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
        Ok(())
    }

    fn release(&self, slot: &SystemAuthSlot) -> SystemAuthRelease {
        let service = match connect() {
            Ok(service) => service,
            Err(_) => return SystemAuthRelease::PassphraseRequired,
        };
        let slot_id = slot.storage_id();
        let mut items = match service.search_items(attributes(&slot_id)) {
            Ok(items) => items,
            Err(error) => {
                return SystemAuthRelease::Failed(format!(
                    "unable to query desktop secret service: {error}"
                ))
            }
        };
        if let Some(item) = items.unlocked.pop() {
            return release_item(&item);
        }
        let Some(item) = items.locked.pop() else {
            return SystemAuthRelease::PassphraseRequired;
        };
        if let Err(error) = item.unlock() {
            return match error {
                SecretServiceError::Prompt => SystemAuthRelease::Cancelled,
                other => {
                    SystemAuthRelease::Failed(format!("unable to unlock desktop keyring: {other}"))
                }
            };
        }
        release_item(&item)
    }

    fn delete(&self, slot: &SystemAuthSlot) -> Result<(), String> {
        let service = connect()?;
        let slot_id = slot.storage_id();
        let mut items = service
            .search_items(attributes(&slot_id))
            .map_err(|error| format!("unable to query desktop secret service: {error}"))?;
        for item in &items.locked {
            item.unlock().map_err(map_interactive_error)?;
        }
        items.unlocked.append(&mut items.locked);
        for item in items.unlocked {
            item.delete()
                .map_err(|error| format!("unable to remove device unlock key: {error}"))?;
        }
        Ok(())
    }
}

fn connect() -> Result<SecretService<'static>, String> {
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

fn release_item(item: &secret_service::blocking::Item<'_>) -> SystemAuthRelease {
    let mut secret = match item.get_secret() {
        Ok(secret) => secret,
        Err(error) => {
            return SystemAuthRelease::Failed(format!("unable to read device unlock key: {error}"))
        }
    };
    if secret.len() != SYSTEM_AUTH_UNLOCK_KEY_LENGTH {
        secret.zeroize();
        return SystemAuthRelease::Failed(
            "desktop secret service returned an invalid device unlock key".to_owned(),
        );
    }
    SystemAuthRelease::UnlockKey(secret)
}

fn map_interactive_error(error: SecretServiceError) -> String {
    match error {
        SecretServiceError::Prompt => "device unlock cancelled".to_owned(),
        other => format!("unable to unlock desktop keyring: {other}"),
    }
}
