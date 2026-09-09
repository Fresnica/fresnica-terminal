use std::sync::{Arc, Mutex};

use fresnica_client::{SystemAuthRelease, SystemAuthSlot, SYSTEM_AUTH_UNLOCK_KEY_LENGTH};
use windows::core::{factory, HSTRING, PCWSTR, PWSTR};
use windows::Security::Credentials::UI::{
    UserConsentVerificationResult, UserConsentVerifier, UserConsentVerifierAvailability,
};
use windows::Win32::Foundation::ERROR_NOT_FOUND;
use windows::Win32::Security::Credentials::{
    CredDeleteW, CredFree, CredReadW, CredWriteW, CREDENTIALW, CRED_PERSIST_LOCAL_MACHINE,
    CRED_TYPE_GENERIC,
};
use windows::Win32::System::Console::GetConsoleWindow;
use windows::Win32::System::WinRT::{
    IUserConsentVerifierInterop, RoInitialize, RoUninitialize, RO_INIT_MULTITHREADED,
};
use windows_future::IAsyncOperation;
use zeroize::Zeroize;

use crate::device_unlock::{
    DeviceAuthenticationOutcome, DeviceAuthenticator, DeviceSecretRead, DeviceSecretStore,
    DeviceUnlockBackend, DeviceUnlockState,
};

const PROVIDER_NAME: &str = "Windows Hello";
const TARGET_PREFIX: &str = "Fresnica:DeviceUnlock:";
const AUTH_MESSAGE: &str = "Authenticate this Fresnica transaction";

pub(crate) fn backend() -> Arc<dyn DeviceUnlockBackend> {
    Arc::new(WindowsDeviceUnlockBackend {
        authenticator: WindowsHelloAuthenticator {
            authenticated: Mutex::new(false),
        },
        store: WindowsCredentialStore,
    })
}

struct WindowsDeviceUnlockBackend {
    authenticator: WindowsHelloAuthenticator,
    store: WindowsCredentialStore,
}

struct WindowsHelloAuthenticator {
    authenticated: Mutex<bool>,
}

struct WindowsCredentialStore;

impl DeviceUnlockBackend for WindowsDeviceUnlockBackend {
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

impl DeviceAuthenticator for WindowsHelloAuthenticator {
    fn authenticate(&self) -> Result<DeviceAuthenticationOutcome, String> {
        let mut authenticated = self
            .authenticated
            .lock()
            .map_err(|_| "Windows device authentication state is unavailable".to_owned())?;
        if *authenticated {
            return Ok(DeviceAuthenticationOutcome::Authenticated);
        }

        let _runtime = match WindowsRuntime::initialize() {
            Ok(runtime) => runtime,
            Err(_) => return windows_hello_unavailable(),
        };
        let availability = match UserConsentVerifier::CheckAvailabilityAsync()
            .and_then(|operation| operation.join())
        {
            Ok(availability) => availability,
            Err(_) => return windows_hello_unavailable(),
        };
        if availability != UserConsentVerifierAvailability::Available {
            return windows_hello_unavailable();
        }

        let window = unsafe { GetConsoleWindow() };
        if window.0.is_null() {
            return windows_hello_unavailable();
        }
        let interop: IUserConsentVerifierInterop =
            match factory::<UserConsentVerifier, IUserConsentVerifierInterop>() {
                Ok(interop) => interop,
                Err(_) => return windows_hello_unavailable(),
            };
        let operation: IAsyncOperation<UserConsentVerificationResult> = match unsafe {
            interop.RequestVerificationForWindowAsync(window, &HSTRING::from(AUTH_MESSAGE))
        } {
            Ok(operation) => operation,
            Err(_) => return windows_hello_unavailable(),
        };
        let result = match operation.join() {
            Ok(result) => result,
            Err(_) => return windows_hello_unavailable(),
        };
        let outcome = match result {
            UserConsentVerificationResult::Verified => DeviceAuthenticationOutcome::Authenticated,
            UserConsentVerificationResult::Canceled => DeviceAuthenticationOutcome::Cancelled,
            UserConsentVerificationResult::DeviceNotPresent
            | UserConsentVerificationResult::NotConfiguredForUser
            | UserConsentVerificationResult::DisabledByPolicy
            | UserConsentVerificationResult::DeviceBusy
            | UserConsentVerificationResult::RetriesExhausted => {
                DeviceAuthenticationOutcome::PassphraseRequired
            }
            other => {
                return Err(format!(
                    "Windows Hello returned unknown verification result {}",
                    other.0
                ))
            }
        };
        if outcome == DeviceAuthenticationOutcome::Authenticated {
            *authenticated = true;
        }
        Ok(outcome)
    }
}

fn windows_hello_unavailable() -> Result<DeviceAuthenticationOutcome, String> {
    eprintln!("Windows Hello unavailable; Fresnica Passphrase required.");
    Ok(DeviceAuthenticationOutcome::PassphraseRequired)
}

struct WindowsRuntime;

impl WindowsRuntime {
    fn initialize() -> Result<Self, String> {
        unsafe { RoInitialize(RO_INIT_MULTITHREADED) }
            .map_err(|error| format!("unable to initialize Windows Runtime: {error}"))?;
        Ok(Self)
    }
}

impl Drop for WindowsRuntime {
    fn drop(&mut self) {
        unsafe { RoUninitialize() };
    }
}

impl DeviceSecretStore for WindowsCredentialStore {
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

    fn read(&self, slot: &SystemAuthSlot) -> Result<DeviceSecretRead, String> {
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
                Ok(DeviceSecretRead::Missing)
            } else {
                Err(format!("unable to read Windows device unlock key: {error}"))
            };
        }
        if credential.is_null() {
            return Err("Windows Credential Manager returned an empty credential".to_owned());
        }
        let stored = unsafe { &*credential };
        if stored.CredentialBlob.is_null()
            || stored.CredentialBlobSize as usize != SYSTEM_AUTH_UNLOCK_KEY_LENGTH
        {
            unsafe { CredFree(credential.cast()) };
            return Err(
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
            return Err("invalid device unlock key length".to_owned());
        }
        Ok(DeviceSecretRead::Secret(key))
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
