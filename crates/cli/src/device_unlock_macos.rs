use std::ffi::c_void;
use std::os::raw::{c_char, c_int, c_long};
use std::sync::{Arc, Mutex};

use core_foundation::base::TCFType;
use core_foundation::data::CFData;
use fresnica_client::{SystemAuthRelease, SystemAuthSlot, SYSTEM_AUTH_UNLOCK_KEY_LENGTH};
use security_framework::item::{
    ItemAddOptions, ItemAddValue, ItemClass, ItemSearchOptions, Location,
};
use security_framework::os::macos::keychain::SecKeychain;
use zeroize::{Zeroize, Zeroizing};

use crate::device_unlock::{
    complete_reauthorization, confirm_reauthorization, versioned_enrollment_state,
    DeviceAuthenticationOutcome, DeviceAuthenticator, DeviceSecretRead, DeviceSecretStore,
    DeviceUnlockBackend, DeviceUnlockState,
};

const PROVIDER_NAME: &str = "macOS Login Keychain";
const SERVICE: &str = "com.fresnica.device-unlock";
const MIGRATION_SERVICE: &str = "com.fresnica.device-unlock.migration";
const LEGACY_METADATA_SERVICE: &str = "com.fresnica.device-unlock.metadata";
const ENROLLMENT_LABEL_PREFIX: &str = "Fresnica Device Unlock v";
const ERR_SEC_USER_CANCELED: i32 = -128;
const ERR_SEC_ITEM_NOT_FOUND: i32 = -25300;
const K_SEC_UNLOCK_STATE_STATUS: u32 = 1;
const MAC_AUTH_AUTHENTICATED: c_int = 0;
const MAC_AUTH_CANCELLED: c_int = 1;
const MAC_AUTH_UNAVAILABLE: c_int = 2;
const MAC_AUTH_FAILED: c_int = 3;
const LOCAL_AUTH_REASON: &[u8] = b"authenticate this transaction\0";

#[link(name = "Security", kind = "framework")]
extern "C" {
    fn SecKeychainGetStatus(keychain: *mut c_void, status: *mut u32) -> i32;
    fn fresnica_macos_authenticate(reason: *const c_char, error_code: *mut c_long) -> c_int;
}

pub(crate) fn backend() -> Arc<dyn DeviceUnlockBackend> {
    Arc::new(MacDeviceUnlockBackend {
        authenticator: MacDeviceAuthenticator {
            authenticated: Mutex::new(false),
        },
        store: MacKeychainStore,
    })
}

struct MacDeviceUnlockBackend {
    authenticator: MacDeviceAuthenticator,
    store: MacKeychainStore,
}

struct MacDeviceAuthenticator {
    authenticated: Mutex<bool>,
}

struct MacKeychainStore;

impl DeviceUnlockBackend for MacDeviceUnlockBackend {
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
        match self.store.state(slot) {
            Ok(DeviceUnlockState::NeedsReauthorization) => {
                return complete_reauthorization(
                    confirm_reauthorization,
                    || self.store.migrate_and_release(slot),
                    || self.authenticator.remember_authenticated(),
                )
            }
            Ok(DeviceUnlockState::Ready | DeviceUnlockState::Locked) => {}
            Ok(DeviceUnlockState::Disabled) => return SystemAuthRelease::PassphraseRequired,
            Ok(DeviceUnlockState::Unavailable) => {
                return SystemAuthRelease::Failed("device unlock unavailable".to_owned())
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

impl MacDeviceAuthenticator {
    fn remember_authenticated(&self) -> Result<(), String> {
        *self
            .authenticated
            .lock()
            .map_err(|_| "macOS device authentication state is unavailable".to_owned())? = true;
        Ok(())
    }
}

impl DeviceAuthenticator for MacDeviceAuthenticator {
    fn authenticate(&self) -> Result<DeviceAuthenticationOutcome, String> {
        let mut authenticated = self
            .authenticated
            .lock()
            .map_err(|_| "macOS device authentication state is unavailable".to_owned())?;
        if *authenticated {
            return Ok(DeviceAuthenticationOutcome::Authenticated);
        }

        let mut keychain = default_keychain()?;
        let outcome = if keychain_unlocked(&keychain)? {
            local_authenticate()?
        } else {
            match keychain.unlock(None) {
                Ok(()) => DeviceAuthenticationOutcome::Authenticated,
                Err(error) if error.code() == ERR_SEC_USER_CANCELED => {
                    DeviceAuthenticationOutcome::Cancelled
                }
                Err(error) => {
                    return Err(format!("unable to unlock macOS Login Keychain: {error}"))
                }
            }
        };
        if outcome == DeviceAuthenticationOutcome::Authenticated {
            *authenticated = true;
        }
        Ok(outcome)
    }
}

impl DeviceSecretStore for MacKeychainStore {
    fn state(&self, slot: &SystemAuthSlot) -> Result<DeviceUnlockState, String> {
        let keychain = default_keychain()?;
        let enrollment_exists = item_exists(&keychain, SERVICE, slot)?;
        let current_version = current_enrollment_exists(&keychain, slot)?;
        let recovery_exists = item_exists(&keychain, MIGRATION_SERVICE, slot)?;
        Ok(versioned_enrollment_state(
            enrollment_exists,
            current_version,
            recovery_exists,
            keychain_unlocked(&keychain).ok(),
        ))
    }

    fn enroll(&self, slot: &SystemAuthSlot, unlock_key: &[u8]) -> Result<(), String> {
        if unlock_key.len() != SYSTEM_AUTH_UNLOCK_KEY_LENGTH {
            return Err("device unlock requires exactly 32 key bytes".to_owned());
        }
        let mut keychain = default_keychain()?;
        ensure_keychain_unlocked(&mut keychain)?;
        if item_exists(&keychain, SERVICE, slot)? {
            return Err("device unlock is already enabled for this signer".to_owned());
        }
        delete_if_authorized_without_prompt(&keychain, MIGRATION_SERVICE, slot);
        delete_if_authorized_without_prompt(&keychain, LEGACY_METADATA_SERVICE, slot);
        add_current_enrollment(&keychain, slot, unlock_key)
    }

    fn read(&self, slot: &SystemAuthSlot) -> Result<DeviceSecretRead, String> {
        let keychain = default_keychain()?;
        let (password, _) = match keychain.find_generic_password(SERVICE, &slot.storage_id()) {
            Ok(value) => value,
            Err(error) if error.code() == ERR_SEC_ITEM_NOT_FOUND => {
                return Ok(DeviceSecretRead::Missing)
            }
            Err(error) if error.code() == ERR_SEC_USER_CANCELED => {
                return Ok(DeviceSecretRead::Cancelled)
            }
            Err(error) => return Err(format!("unable to read device unlock key: {error}")),
        };
        let mut key = password.as_ref().to_vec();
        if key.len() != SYSTEM_AUTH_UNLOCK_KEY_LENGTH {
            key.zeroize();
            return Err("macOS Keychain returned an invalid device unlock key".to_owned());
        }
        Ok(DeviceSecretRead::Secret(key))
    }

    fn delete(&self, slot: &SystemAuthSlot) -> Result<(), String> {
        let mut keychain = default_keychain()?;
        ensure_keychain_unlocked(&mut keychain)?;
        // Remove auxiliary items first. If an old binary owns one of them and
        // macOS denies access, keep the canonical unlock key recoverable.
        delete_password_if_exists(&keychain, LEGACY_METADATA_SERVICE, slot)?;
        delete_password_if_exists(&keychain, MIGRATION_SERVICE, slot)?;
        delete_password_if_exists(&keychain, SERVICE, slot)
    }
}

impl MacKeychainStore {
    fn migrate_and_release(&self, slot: &SystemAuthSlot) -> Result<DeviceSecretRead, String> {
        let keychain = default_keychain()?;
        let account = slot.storage_id();
        let (password, source_item, source_is_primary) =
            match keychain.find_generic_password(SERVICE, &account) {
                Ok((password, item)) => (password, item, true),
                Err(error) if error.code() == ERR_SEC_ITEM_NOT_FOUND => {
                    match keychain.find_generic_password(MIGRATION_SERVICE, &account) {
                        Ok((password, item)) => (password, item, false),
                        Err(error) if error.code() == ERR_SEC_ITEM_NOT_FOUND => {
                            return Ok(DeviceSecretRead::Missing)
                        }
                        Err(error) if error.code() == ERR_SEC_USER_CANCELED => {
                            return Ok(DeviceSecretRead::Cancelled)
                        }
                        Err(error) => {
                            return Err(format!(
                                "unable to read Device Unlock migration recovery: {error}"
                            ))
                        }
                    }
                }
                Err(error) if error.code() == ERR_SEC_USER_CANCELED => {
                    return Ok(DeviceSecretRead::Cancelled)
                }
                Err(error) => {
                    return Err(format!(
                        "unable to read Device Unlock enrollment for migration: {error}"
                    ))
                }
            };

        let key = Zeroizing::new(password.as_ref().to_vec());
        if key.len() != SYSTEM_AUTH_UNLOCK_KEY_LENGTH {
            return Err("macOS Keychain returned an invalid device unlock key".to_owned());
        }

        let mut source_item = Some(source_item);
        let mut stage_item = None;
        if source_is_primary {
            if !item_exists(&keychain, MIGRATION_SERVICE, slot)? {
                keychain
                    .add_generic_password(MIGRATION_SERVICE, &account, key.as_slice())
                    .map_err(|error| format!("unable to stage Device Unlock migration: {error}"))?;
                let (staged, item) = keychain
                    .find_generic_password(MIGRATION_SERVICE, &account)
                    .map_err(|error| {
                        format!("unable to verify Device Unlock migration staging: {error}")
                    })?;
                if staged.as_ref() != key.as_slice() {
                    return Err("Device Unlock migration staging verification failed".to_owned());
                }
                stage_item = Some(item);
            }
            source_item.take().expect("source item").delete();
            if item_exists(&keychain, SERVICE, slot)? {
                return Err("unable to replace the old Device Unlock enrollment".to_owned());
            }
        } else {
            stage_item = source_item.take();
        }

        add_current_enrollment(&keychain, slot, key.as_slice())?;
        let (current, current_item) = keychain
            .find_generic_password(SERVICE, &account)
            .map_err(|error| format!("unable to verify Device Unlock migration: {error}"))?;
        if current.as_ref() != key.as_slice() {
            current_item.delete();
            return Err("Device Unlock migration verification failed".to_owned());
        }

        if let Some(item) = stage_item {
            item.delete();
        }
        delete_if_authorized_without_prompt(&keychain, MIGRATION_SERVICE, slot);
        delete_if_authorized_without_prompt(&keychain, LEGACY_METADATA_SERVICE, slot);
        Ok(DeviceSecretRead::Secret(key.to_vec()))
    }
}

fn enrollment_label() -> String {
    format!("{ENROLLMENT_LABEL_PREFIX}{}", env!("CARGO_PKG_VERSION"))
}

fn add_current_enrollment(
    keychain: &SecKeychain,
    slot: &SystemAuthSlot,
    unlock_key: &[u8],
) -> Result<(), String> {
    let label = enrollment_label();
    let mut item = ItemAddOptions::new(ItemAddValue::Data {
        class: ItemClass::generic_password(),
        data: CFData::from_buffer(unlock_key),
    });
    item.set_location(Location::FileKeychain(keychain.clone()))
        .set_service(SERVICE)
        .set_account_name(slot.storage_id())
        .set_label(&label)
        .set_description("Fresnica Device Unlock key");
    item.add()
        .map_err(|error| format!("unable to store Device Unlock key: {error}"))
}

fn current_enrollment_exists(
    keychain: &SecKeychain,
    slot: &SystemAuthSlot,
) -> Result<bool, String> {
    let label = enrollment_label();
    let mut search = ItemSearchOptions::new();
    search
        .keychains(std::slice::from_ref(keychain))
        .class(ItemClass::generic_password())
        .service(SERVICE)
        .account(&slot.storage_id())
        .label(&label)
        .load_attributes(true)
        .skip_authenticated_items(true);
    match search.search() {
        Ok(results) => Ok(!results.is_empty()),
        Err(error) if error.code() == ERR_SEC_ITEM_NOT_FOUND => Ok(false),
        Err(error) => Err(format!(
            "unable to query Device Unlock enrollment version: {error}"
        )),
    }
}

fn local_authenticate() -> Result<DeviceAuthenticationOutcome, String> {
    let mut error_code = 0 as c_long;
    let result = unsafe {
        fresnica_macos_authenticate(LOCAL_AUTH_REASON.as_ptr().cast::<c_char>(), &mut error_code)
    };
    match result {
        MAC_AUTH_AUTHENTICATED => Ok(DeviceAuthenticationOutcome::Authenticated),
        MAC_AUTH_CANCELLED => Ok(DeviceAuthenticationOutcome::Cancelled),
        MAC_AUTH_UNAVAILABLE => Ok(DeviceAuthenticationOutcome::PassphraseRequired),
        MAC_AUTH_FAILED => Err(format!(
            "macOS device authentication failed (LAError {error_code})"
        )),
        other => Err(format!(
            "macOS device authentication returned invalid status {other}"
        )),
    }
}

fn default_keychain() -> Result<SecKeychain, String> {
    SecKeychain::default().map_err(|error| format!("unable to open macOS Login Keychain: {error}"))
}

fn item_exists(
    keychain: &SecKeychain,
    service: &str,
    slot: &SystemAuthSlot,
) -> Result<bool, String> {
    let mut search = ItemSearchOptions::new();
    search
        .keychains(std::slice::from_ref(keychain))
        .class(ItemClass::generic_password())
        .service(service)
        .account(&slot.storage_id())
        .load_attributes(true)
        .skip_authenticated_items(true);
    match search.search() {
        Ok(results) => Ok(!results.is_empty()),
        Err(error) if error.code() == ERR_SEC_ITEM_NOT_FOUND => Ok(false),
        Err(error) => Err(format!("unable to query macOS Login Keychain: {error}")),
    }
}

fn delete_password_if_exists(
    keychain: &SecKeychain,
    service: &str,
    slot: &SystemAuthSlot,
) -> Result<(), String> {
    match keychain.find_generic_password(service, &slot.storage_id()) {
        Ok((_, item)) => {
            item.delete();
            if item_exists(keychain, service, slot)? {
                Err("unable to remove Device Unlock Keychain item".to_owned())
            } else {
                Ok(())
            }
        }
        Err(error) if error.code() == ERR_SEC_ITEM_NOT_FOUND => Ok(()),
        Err(error) if error.code() == ERR_SEC_USER_CANCELED => {
            Err("Device Unlock removal cancelled".to_owned())
        }
        Err(error) => Err(format!(
            "unable to remove Device Unlock Keychain item: {error}"
        )),
    }
}

fn delete_if_authorized_without_prompt(
    keychain: &SecKeychain,
    service: &str,
    slot: &SystemAuthSlot,
) {
    let mut search = ItemSearchOptions::new();
    search
        .keychains(std::slice::from_ref(keychain))
        .class(ItemClass::generic_password())
        .service(service)
        .account(&slot.storage_id())
        .skip_authenticated_items(true);
    let _ = search.delete();
}

fn keychain_unlocked(keychain: &SecKeychain) -> Result<bool, String> {
    let mut status = 0u32;
    let result = unsafe {
        SecKeychainGetStatus(keychain.as_concrete_TypeRef().cast::<c_void>(), &mut status)
    };
    if result != 0 {
        return Err(format!(
            "unable to query macOS Login Keychain status: {result}"
        ));
    }
    Ok(status & K_SEC_UNLOCK_STATE_STATUS != 0)
}

fn ensure_keychain_unlocked(keychain: &mut SecKeychain) -> Result<(), String> {
    if keychain_unlocked(keychain)? {
        return Ok(());
    }
    keychain.unlock(None).map_err(|error| {
        if error.code() == ERR_SEC_USER_CANCELED {
            "device unlock cancelled".to_owned()
        } else {
            format!("unable to unlock macOS Login Keychain: {error}")
        }
    })
}
