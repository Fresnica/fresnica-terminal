use std::ffi::c_void;
use std::os::raw::{c_char, c_int, c_long};
use std::sync::{Arc, Mutex};

use core_foundation::base::TCFType;
use fresnica_client::{SystemAuthRelease, SystemAuthSlot, SYSTEM_AUTH_UNLOCK_KEY_LENGTH};
use security_framework::item::{ItemClass, ItemSearchOptions};
use security_framework::os::macos::keychain::SecKeychain;
use zeroize::Zeroize;

use crate::device_unlock::{
    DeviceAuthenticationOutcome, DeviceAuthenticator, DeviceSecretRead, DeviceSecretStore,
    DeviceUnlockBackend, DeviceUnlockState,
};

const PROVIDER_NAME: &str = "macOS Login Keychain";
const SERVICE: &str = "com.fresnica.device-unlock";
const METADATA_SERVICE: &str = "com.fresnica.device-unlock.metadata";
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
        match self.authenticator.authenticate() {
            Ok(DeviceAuthenticationOutcome::Authenticated) => {}
            Ok(DeviceAuthenticationOutcome::Cancelled) => return SystemAuthRelease::Cancelled,
            Ok(DeviceAuthenticationOutcome::PassphraseRequired) => {
                return SystemAuthRelease::PassphraseRequired
            }
            Err(error) => return SystemAuthRelease::Failed(error),
        }
        match self.store.state(slot) {
            Ok(DeviceUnlockState::NeedsReauthorization) => {
                if let Err(error) = self.store.update_enrollment(slot) {
                    return SystemAuthRelease::Failed(error);
                }
            }
            Ok(DeviceUnlockState::Ready | DeviceUnlockState::Locked) => {}
            Ok(DeviceUnlockState::Disabled) => return SystemAuthRelease::PassphraseRequired,
            Ok(DeviceUnlockState::Unavailable) => {
                return SystemAuthRelease::Failed("device unlock unavailable".to_owned())
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
        if !item_exists(&keychain, slot)? {
            return Ok(DeviceUnlockState::Disabled);
        }
        if !metadata_matches(&keychain, slot)? {
            return Ok(DeviceUnlockState::NeedsReauthorization);
        }
        match keychain_unlocked(&keychain) {
            Ok(true) => Ok(DeviceUnlockState::Ready),
            Ok(false) => Ok(DeviceUnlockState::Locked),
            Err(_) => Ok(DeviceUnlockState::Unavailable),
        }
    }

    fn enroll(&self, slot: &SystemAuthSlot, unlock_key: &[u8]) -> Result<(), String> {
        if unlock_key.len() != SYSTEM_AUTH_UNLOCK_KEY_LENGTH {
            return Err("device unlock requires exactly 32 key bytes".to_owned());
        }
        let mut keychain = default_keychain()?;
        ensure_keychain_unlocked(&mut keychain)?;
        keychain
            .set_generic_password(SERVICE, &slot.storage_id(), unlock_key)
            .map_err(|error| format!("unable to store device unlock key: {error}"))?;
        keychain
            .set_generic_password(
                METADATA_SERVICE,
                &slot.storage_id(),
                env!("CARGO_PKG_VERSION").as_bytes(),
            )
            .map_err(|error| format!("unable to store device unlock metadata: {error}"))
    }

    fn update_enrollment(&self, slot: &SystemAuthSlot) -> Result<(), String> {
        let mut keychain = default_keychain()?;
        ensure_keychain_unlocked(&mut keychain)?;
        let (password, _) = keychain
            .find_generic_password(SERVICE, &slot.storage_id())
            .map_err(|error| {
                format!("unable to read device unlock enrollment for migration: {error}")
            })?;
        let key = password.as_ref();
        if key.len() != SYSTEM_AUTH_UNLOCK_KEY_LENGTH {
            return Err("macOS Keychain returned an invalid device unlock key".to_owned());
        }
        write_enrollment_items(&mut keychain, slot, key)
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
        if !item_exists(&keychain, slot)? {
            return Ok(());
        }
        ensure_keychain_unlocked(&mut keychain)?;
        match keychain.find_generic_password(SERVICE, &slot.storage_id()) {
            Ok((_, item)) => {
                item.delete();
                match keychain.find_generic_password(METADATA_SERVICE, &slot.storage_id()) {
                    Ok((_, metadata)) => {
                        metadata.delete();
                        Ok(())
                    }
                    Err(error) if error.code() == ERR_SEC_ITEM_NOT_FOUND => Ok(()),
                    Err(error) => Err(format!("unable to remove device unlock metadata: {error}")),
                }
            }
            Err(error) if error.code() == ERR_SEC_ITEM_NOT_FOUND => Ok(()),
            Err(error) => Err(format!("unable to remove device unlock key: {error}")),
        }
    }
}

fn write_enrollment_items(
    keychain: &mut SecKeychain,
    slot: &SystemAuthSlot,
    unlock_key: &[u8],
) -> Result<(), String> {
    keychain
        .set_generic_password(SERVICE, &slot.storage_id(), unlock_key)
        .map_err(|error| format!("unable to migrate device unlock key: {error}"))?;
    keychain
        .set_generic_password(
            METADATA_SERVICE,
            &slot.storage_id(),
            env!("CARGO_PKG_VERSION").as_bytes(),
        )
        .map_err(|error| format!("unable to migrate device unlock metadata: {error}"))
}

fn metadata_matches(keychain: &SecKeychain, slot: &SystemAuthSlot) -> Result<bool, String> {
    match keychain.find_generic_password(METADATA_SERVICE, &slot.storage_id()) {
        Ok((version, _)) => Ok(version.as_ref() == env!("CARGO_PKG_VERSION").as_bytes()),
        Err(error) if error.code() == ERR_SEC_ITEM_NOT_FOUND => Ok(false),
        Err(error) => Err(format!("unable to read device unlock metadata: {error}")),
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

fn item_exists(keychain: &SecKeychain, slot: &SystemAuthSlot) -> Result<bool, String> {
    let mut search = ItemSearchOptions::new();
    search
        .keychains(std::slice::from_ref(keychain))
        .class(ItemClass::generic_password())
        .service(SERVICE)
        .account(&slot.storage_id())
        .load_attributes(true)
        .skip_authenticated_items(true);
    match search.search() {
        Ok(results) => Ok(!results.is_empty()),
        Err(error) if error.code() == ERR_SEC_ITEM_NOT_FOUND => Ok(false),
        Err(error) => Err(format!("unable to query macOS Login Keychain: {error}")),
    }
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
