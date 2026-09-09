use std::ffi::c_void;
use std::os::raw::{c_char, c_int, c_long};
use std::sync::{Arc, Mutex};

use core_foundation::base::TCFType;
use fresnica_client::{SystemAuthRelease, SystemAuthSlot, SYSTEM_AUTH_UNLOCK_KEY_LENGTH};
use security_framework::item::{ItemClass, ItemSearchOptions};
use security_framework::os::macos::keychain::SecKeychain;
use zeroize::Zeroize;

use crate::device_unlock::{DeviceUnlockBackend, DeviceUnlockState};

const PROVIDER_NAME: &str = "macOS Login Keychain";
const SERVICE: &str = "com.fresnica.device-unlock";
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
        authenticated: Mutex::new(false),
    })
}

struct MacDeviceUnlockBackend {
    authenticated: Mutex<bool>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MacAuthenticationOutcome {
    Authenticated,
    Cancelled,
    PassphraseRequired,
}

impl DeviceUnlockBackend for MacDeviceUnlockBackend {
    fn provider_name(&self) -> &'static str {
        PROVIDER_NAME
    }

    fn state(&self, slot: &SystemAuthSlot) -> Result<DeviceUnlockState, String> {
        let keychain = default_keychain()?;
        if !item_exists(&keychain, slot)? {
            return Ok(DeviceUnlockState::Disabled);
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
            .map_err(|error| format!("unable to store device unlock key: {error}"))
    }

    fn release(&self, slot: &SystemAuthSlot) -> SystemAuthRelease {
        match self.authenticate_once() {
            Ok(MacAuthenticationOutcome::Authenticated) => {}
            Ok(MacAuthenticationOutcome::Cancelled) => return SystemAuthRelease::Cancelled,
            Ok(MacAuthenticationOutcome::PassphraseRequired) => {
                return SystemAuthRelease::PassphraseRequired
            }
            Err(error) => return SystemAuthRelease::Failed(error),
        }

        let keychain = match default_keychain() {
            Ok(keychain) => keychain,
            Err(error) => return SystemAuthRelease::Failed(error),
        };
        let (password, _) = match keychain.find_generic_password(SERVICE, &slot.storage_id()) {
            Ok(value) => value,
            Err(error) if error.code() == ERR_SEC_USER_CANCELED => {
                return SystemAuthRelease::Cancelled
            }
            Err(error) if error.code() == ERR_SEC_ITEM_NOT_FOUND => {
                return SystemAuthRelease::PassphraseRequired
            }
            Err(error) => {
                return SystemAuthRelease::Failed(format!(
                    "unable to read device unlock key: {error}"
                ))
            }
        };
        let mut key = password.as_ref().to_vec();
        if key.len() != SYSTEM_AUTH_UNLOCK_KEY_LENGTH {
            key.zeroize();
            return SystemAuthRelease::Failed(
                "macOS Keychain returned an invalid device unlock key".to_owned(),
            );
        }
        SystemAuthRelease::UnlockKey(key)
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
                Ok(())
            }
            Err(error) if error.code() == ERR_SEC_ITEM_NOT_FOUND => Ok(()),
            Err(error) => Err(format!("unable to remove device unlock key: {error}")),
        }
    }
}

impl MacDeviceUnlockBackend {
    fn authenticate_once(&self) -> Result<MacAuthenticationOutcome, String> {
        let mut authenticated = self
            .authenticated
            .lock()
            .map_err(|_| "macOS device authentication state is unavailable".to_owned())?;
        if *authenticated {
            return Ok(MacAuthenticationOutcome::Authenticated);
        }

        let mut keychain = default_keychain()?;
        let outcome = if keychain_unlocked(&keychain)? {
            local_authenticate()?
        } else {
            match keychain.unlock(None) {
                Ok(()) => MacAuthenticationOutcome::Authenticated,
                Err(error) if error.code() == ERR_SEC_USER_CANCELED => {
                    MacAuthenticationOutcome::Cancelled
                }
                Err(error) => {
                    return Err(format!("unable to unlock macOS Login Keychain: {error}"))
                }
            }
        };
        if outcome == MacAuthenticationOutcome::Authenticated {
            *authenticated = true;
        }
        Ok(outcome)
    }
}

fn local_authenticate() -> Result<MacAuthenticationOutcome, String> {
    let mut error_code = 0 as c_long;
    let result = unsafe {
        fresnica_macos_authenticate(
            LOCAL_AUTH_REASON.as_ptr().cast::<c_char>(),
            &mut error_code,
        )
    };
    match result {
        MAC_AUTH_AUTHENTICATED => Ok(MacAuthenticationOutcome::Authenticated),
        MAC_AUTH_CANCELLED => Ok(MacAuthenticationOutcome::Cancelled),
        MAC_AUTH_UNAVAILABLE => Ok(MacAuthenticationOutcome::PassphraseRequired),
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
