use std::sync::Arc;

use fresnica_client::{SystemAuthRelease, SystemAuthSlot, SYSTEM_AUTH_UNLOCK_KEY_LENGTH};
use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::ERROR_NOT_FOUND;
use windows::Win32::Security::Credentials::{
    CredDeleteW, CredFree, CredReadW, CredWriteW, CREDENTIALW, CRED_PERSIST_LOCAL_MACHINE,
    CRED_TYPE_GENERIC,
};
use zeroize::Zeroize;

use crate::device_unlock::{DeviceUnlockBackend, DeviceUnlockState};

const PROVIDER_NAME: &str = "Windows Credential Manager";
const TARGET_PREFIX: &str = "Fresnica:DeviceUnlock:";

pub(crate) fn backend() -> Arc<dyn DeviceUnlockBackend> {
    Arc::new(WindowsDeviceUnlockBackend)
}

struct WindowsDeviceUnlockBackend;

impl DeviceUnlockBackend for WindowsDeviceUnlockBackend {
    fn provider_name(&self) -> &'static str {
        PROVIDER_NAME
    }

    fn state(&self, slot: &SystemAuthSlot) -> Result<DeviceUnlockState, String> {
        if credential_exists(slot)? {
            Ok(DeviceUnlockState::Ready)
        } else {
            Ok(DeviceUnlockState::Disabled)
        }
    }

    fn enroll(&self, slot: &SystemAuthSlot, unlock_key: &[u8]) -> Result<(), String> {
        if unlock_key.len() != SYSTEM_AUTH_UNLOCK_KEY_LENGTH {
            return Err("device unlock requires exactly 32 key bytes".to_owned());
        }
        let mut target = wide_null(&target_name(slot));
        let credential = CREDENTIALW {
            Type: CRED_TYPE_GENERIC,
            TargetName: PWSTR(target.as_mut_ptr()),
            CredentialBlobSize: unlock_key.len() as u32,
            CredentialBlob: unlock_key.as_ptr() as *mut u8,
            Persist: CRED_PERSIST_LOCAL_MACHINE,
            ..Default::default()
        };
        unsafe { CredWriteW(&credential, 0) }
            .map_err(|error| format!("unable to store device unlock key: {error}"))
    }

    fn release(&self, slot: &SystemAuthSlot) -> SystemAuthRelease {
        let target = wide_null(&target_name(slot));
        let mut credential = std::ptr::null_mut();
        if let Err(error) = unsafe {
            CredReadW(
                PCWSTR(target.as_ptr()),
                CRED_TYPE_GENERIC,
                None,
                &mut credential,
            )
        } {
            return if is_not_found(&error) {
                SystemAuthRelease::PassphraseRequired
            } else {
                SystemAuthRelease::Failed(format!(
                    "unable to read Windows device unlock key: {error}"
                ))
            };
        }
        if credential.is_null() {
            return SystemAuthRelease::Failed(
                "Windows Credential Manager returned an empty credential".to_owned(),
            );
        }
        let stored = unsafe { &*credential };
        if stored.CredentialBlob.is_null()
            || stored.CredentialBlobSize as usize != SYSTEM_AUTH_UNLOCK_KEY_LENGTH
        {
            unsafe { CredFree(credential.cast()) };
            return SystemAuthRelease::Failed(
                "Windows Credential Manager returned an invalid device unlock key".to_owned(),
            );
        }
        let mut key = unsafe {
            std::slice::from_raw_parts(stored.CredentialBlob, stored.CredentialBlobSize as usize)
                .to_vec()
        };
        unsafe { CredFree(credential.cast()) };
        if key.len() != SYSTEM_AUTH_UNLOCK_KEY_LENGTH {
            key.zeroize();
            return SystemAuthRelease::Failed("invalid device unlock key length".to_owned());
        }
        SystemAuthRelease::UnlockKey(key)
    }

    fn delete(&self, slot: &SystemAuthSlot) -> Result<(), String> {
        let target = wide_null(&target_name(slot));
        match unsafe { CredDeleteW(PCWSTR(target.as_ptr()), CRED_TYPE_GENERIC, None) } {
            Ok(()) => Ok(()),
            Err(error) if is_not_found(&error) => Ok(()),
            Err(error) => Err(format!("unable to remove device unlock key: {error}")),
        }
    }
}

fn credential_exists(slot: &SystemAuthSlot) -> Result<bool, String> {
    let target = wide_null(&target_name(slot));
    let mut credential = std::ptr::null_mut();
    match unsafe {
        CredReadW(
            PCWSTR(target.as_ptr()),
            CRED_TYPE_GENERIC,
            None,
            &mut credential,
        )
    } {
        Ok(()) => {
            if !credential.is_null() {
                unsafe { CredFree(credential.cast()) };
            }
            Ok(true)
        }
        Err(error) if is_not_found(&error) => Ok(false),
        Err(error) => Err(format!(
            "unable to query Windows Credential Manager: {error}"
        )),
    }
}

fn target_name(slot: &SystemAuthSlot) -> String {
    format!("{TARGET_PREFIX}{}", slot.storage_id())
}

fn wide_null(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn is_not_found(error: &windows::core::Error) -> bool {
    error.code() == ERROR_NOT_FOUND.to_hresult()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credential_target_is_exact_slot_scoped() {
        let slot = SystemAuthSlot {
            signer_public_key: "GDLVVGABQKYQVN6VJP7NHSLEA45A5YLS6PNKMIZFV4BBU2HXA5IRVHUR"
                .to_owned(),
            envelope_fingerprint: "0123456789abcdef".to_owned(),
        };
        let target = target_name(&slot);
        assert!(target.starts_with(TARGET_PREFIX));
        assert!(target.ends_with(&slot.storage_id()));
    }
}
