#[cfg(not(windows))]
fn main() {
    eprintln!("Fresnica Windows System Auth provider is Windows-only");
    std::process::exit(20);
}

#[cfg(windows)]
fn main() {
    windows_provider::run();
}

#[cfg(windows)]
mod windows_provider {
    use fresnica_system_auth_windows::{
        read_frame, valid_slot, write_frame, Request, Response, PIPE_NAME, RP_ID, RP_NAME,
        UNLOCK_KEY_LEN,
    };
    use std::fs::{File, OpenOptions};
    use std::io::{self, Read, Write};
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Networking::WindowsWebServices::{
        WebAuthNAuthenticatorGetAssertion, WebAuthNAuthenticatorMakeCredential,
        WebAuthNFreeAssertion, WebAuthNFreeCredentialAttestation, WebAuthNGetErrorName,
        WebAuthNIsUserVerifyingPlatformAuthenticatorAvailable, WEBAUTHN_ASSERTION,
        WEBAUTHN_ATTESTATION_CONVEYANCE_PREFERENCE_NONE,
        WEBAUTHN_AUTHENTICATOR_ATTACHMENT_PLATFORM, WEBAUTHN_AUTHENTICATOR_GET_ASSERTION_OPTIONS,
        WEBAUTHN_AUTHENTICATOR_GET_ASSERTION_OPTIONS_VERSION_1,
        WEBAUTHN_AUTHENTICATOR_MAKE_CREDENTIAL_OPTIONS,
        WEBAUTHN_AUTHENTICATOR_MAKE_CREDENTIAL_OPTIONS_VERSION_1, WEBAUTHN_CLIENT_DATA,
        WEBAUTHN_CLIENT_DATA_CURRENT_VERSION,
        WEBAUTHN_COSE_ALGORITHM_RSASSA_PKCS1_V1_5_WITH_SHA256, WEBAUTHN_COSE_CREDENTIAL_PARAMETER,
        WEBAUTHN_COSE_CREDENTIAL_PARAMETERS, WEBAUTHN_COSE_CREDENTIAL_PARAMETER_CURRENT_VERSION,
        WEBAUTHN_CREDENTIAL, WEBAUTHN_CREDENTIALS, WEBAUTHN_CREDENTIAL_CURRENT_VERSION,
        WEBAUTHN_CREDENTIAL_TYPE_PUBLIC_KEY, WEBAUTHN_HASH_ALGORITHM_SHA_256,
        WEBAUTHN_RP_ENTITY_INFORMATION, WEBAUTHN_RP_ENTITY_INFORMATION_CURRENT_VERSION,
        WEBAUTHN_USER_ENTITY_INFORMATION, WEBAUTHN_USER_ENTITY_INFORMATION_CURRENT_VERSION,
        WEBAUTHN_USER_VERIFICATION_REQUIREMENT_REQUIRED,
    };
    use windows::Win32::System::Console::GetConsoleWindow;
    use windows::Win32::UI::WindowsAndMessaging::{GetAncestor, GA_ROOTOWNER};
    use zeroize::{Zeroize, Zeroizing};

    const EXIT_PASSPHRASE_REQUIRED: i32 = 10;
    const EXIT_CANCELLED: i32 = 11;
    const EXIT_FAILURE: i32 = 20;

    pub(super) fn run() {
        let arguments: Vec<String> = std::env::args().collect();
        let command = arguments.get(1).map(String::as_str).unwrap_or("");
        let result = match command {
            "probe" if arguments.len() == 2 => probe(),
            "has" if arguments.len() == 3 => has(require_slot(&arguments[2])),
            "enroll" if arguments.len() == 3 => enroll(require_slot(&arguments[2])),
            "release" if arguments.len() == 3 => release(require_slot(&arguments[2])),
            "delete" if arguments.len() == 3 => delete(require_slot(&arguments[2])),
            _ => Outcome::Failed(
                "usage: fresnica-system-auth-provider probe|has|enroll|release|delete [SLOT]"
                    .to_owned(),
            ),
        };
        finish(result)
    }

    fn probe() -> Outcome {
        match platform_authenticator_available() {
            Ok(true) => {}
            Ok(false) => return Outcome::PassphraseRequired,
            Err(error) => return Outcome::Failed(error),
        }
        let mut pipe = match connect() {
            Ok(pipe) => pipe,
            Err(_) => return Outcome::PassphraseRequired,
        };
        match exchange(&mut pipe, &Request::Probe) {
            Ok(Response::Ok) => Outcome::Success,
            Ok(Response::Missing) => Outcome::PassphraseRequired,
            Ok(Response::Error { message }) => Outcome::Failed(message),
            Ok(_) => Outcome::Failed(
                "Windows System Auth service returned an invalid probe response".to_owned(),
            ),
            Err(_) => Outcome::PassphraseRequired,
        }
    }

    fn has(slot: &str) -> Outcome {
        let mut pipe = match connect() {
            Ok(pipe) => pipe,
            Err(_) => return Outcome::PassphraseRequired,
        };
        match exchange(
            &mut pipe,
            &Request::Has {
                slot: slot.to_owned(),
            },
        ) {
            Ok(Response::Ok) => Outcome::Success,
            Ok(Response::Missing) => Outcome::PassphraseRequired,
            Ok(Response::Error { message }) => Outcome::Failed(message),
            Ok(_) => Outcome::Failed(
                "Windows System Auth service returned an invalid status response".to_owned(),
            ),
            Err(error) => Outcome::Failed(error),
        }
    }

    fn enroll(slot: &str) -> Outcome {
        match platform_authenticator_available() {
            Ok(true) => {}
            Ok(false) => return Outcome::PassphraseRequired,
            Err(error) => return Outcome::Failed(error),
        }
        let mut key = match read_unlock_key() {
            Ok(key) => key,
            Err(error) => return Outcome::Failed(error),
        };
        let mut pipe = match connect() {
            Ok(pipe) => pipe,
            Err(error) => return Outcome::Failed(error),
        };
        let mut request = Request::Enroll {
            slot: slot.to_owned(),
            unlock_key: key.to_vec(),
        };
        key.zeroize();
        let response = exchange(&mut pipe, &request);
        if let Request::Enroll { unlock_key, .. } = &mut request {
            unlock_key.zeroize();
        }
        match response {
            Ok(Response::Ok) => Outcome::Success,
            Ok(Response::CreateCredential {
                client_data_json,
                user_id,
            }) => {
                let (credential_id, authenticator_data) =
                    match make_credential(&client_data_json, &user_id) {
                        Ok(value) => value,
                        Err(outcome) => return outcome,
                    };
                match exchange(
                    &mut pipe,
                    &Request::Credential {
                        credential_id,
                        authenticator_data,
                    },
                ) {
                    Ok(Response::Ok) => Outcome::Success,
                    Ok(Response::Error { message }) => Outcome::Failed(message),
                    Ok(_) => Outcome::Failed(
                        "Windows System Auth service returned an invalid enrollment response"
                            .to_owned(),
                    ),
                    Err(error) => Outcome::Failed(error),
                }
            }
            Ok(Response::Error { message }) => Outcome::Failed(message),
            Ok(_) => Outcome::Failed(
                "Windows System Auth service returned an invalid enrollment response".to_owned(),
            ),
            Err(error) => Outcome::Failed(error),
        }
    }

    fn release(slot: &str) -> Outcome {
        match platform_authenticator_available() {
            Ok(true) => {}
            Ok(false) => return Outcome::PassphraseRequired,
            Err(error) => return Outcome::Failed(error),
        }
        let mut pipe = match connect() {
            Ok(pipe) => pipe,
            Err(_) => return Outcome::PassphraseRequired,
        };
        match exchange(
            &mut pipe,
            &Request::Release {
                slot: slot.to_owned(),
            },
        ) {
            Ok(Response::Missing) => Outcome::PassphraseRequired,
            Ok(Response::AssertionChallenge {
                client_data_json,
                credential_id,
            }) => {
                let assertion = match get_assertion(&client_data_json, &credential_id) {
                    Ok(assertion) => assertion,
                    Err(outcome) => return outcome,
                };
                match exchange(
                    &mut pipe,
                    &Request::Assertion {
                        credential_id: assertion.credential_id,
                        authenticator_data: assertion.authenticator_data,
                        signature: assertion.signature,
                    },
                ) {
                    Ok(Response::Key { mut unlock_key }) => {
                        if unlock_key.len() != UNLOCK_KEY_LEN {
                            unlock_key.zeroize();
                            return Outcome::Failed(
                                "Windows System Auth service returned an invalid unlock key"
                                    .to_owned(),
                            );
                        }
                        let result = io::stdout().write_all(&unlock_key);
                        unlock_key.zeroize();
                        match result {
                            Ok(()) => Outcome::Success,
                            Err(error) => Outcome::Failed(format!(
                                "unable to return Windows System Auth unlock key: {error}"
                            )),
                        }
                    }
                    Ok(Response::Missing) => Outcome::PassphraseRequired,
                    Ok(Response::Error { message }) => Outcome::Failed(message),
                    Ok(_) => Outcome::Failed(
                        "Windows System Auth service returned an invalid proof response".to_owned(),
                    ),
                    Err(error) => Outcome::Failed(error),
                }
            }
            Ok(Response::Error { message }) => Outcome::Failed(message),
            Ok(_) => Outcome::Failed(
                "Windows System Auth service returned an invalid release response".to_owned(),
            ),
            Err(_) => Outcome::PassphraseRequired,
        }
    }

    fn delete(slot: &str) -> Outcome {
        let mut pipe = match connect() {
            Ok(pipe) => pipe,
            Err(error) => return Outcome::Failed(error),
        };
        match exchange(
            &mut pipe,
            &Request::Delete {
                slot: slot.to_owned(),
            },
        ) {
            Ok(Response::Ok | Response::Missing) => Outcome::Success,
            Ok(Response::Error { message }) => Outcome::Failed(message),
            Ok(_) => Outcome::Failed(
                "Windows System Auth service returned an invalid delete response".to_owned(),
            ),
            Err(error) => Outcome::Failed(error),
        }
    }

    fn platform_authenticator_available() -> Result<bool, String> {
        unsafe { WebAuthNIsUserVerifyingPlatformAuthenticatorAvailable() }
            .map(|available| available.as_bool())
            .map_err(|error| format!("unable to query Windows Hello WebAuthn support: {error}"))
    }

    fn make_credential(
        client_data_json: &[u8],
        user_id: &[u8],
    ) -> Result<(Vec<u8>, Vec<u8>), Outcome> {
        if user_id.is_empty() || user_id.len() > 64 {
            return Err(Outcome::Failed(
                "Windows System Auth service returned an invalid WebAuthn user id".to_owned(),
            ));
        }
        let hwnd = webauthn_window()?;
        let rp_id = wide_null(RP_ID);
        let rp_name = wide_null(RP_NAME);
        let user_name = wide_null("Fresnica");
        let rp = WEBAUTHN_RP_ENTITY_INFORMATION {
            dwVersion: WEBAUTHN_RP_ENTITY_INFORMATION_CURRENT_VERSION,
            pwszId: PCWSTR(rp_id.as_ptr()),
            pwszName: PCWSTR(rp_name.as_ptr()),
            pwszIcon: PCWSTR::null(),
        };
        let user = WEBAUTHN_USER_ENTITY_INFORMATION {
            dwVersion: WEBAUTHN_USER_ENTITY_INFORMATION_CURRENT_VERSION,
            cbId: user_id.len() as u32,
            pbId: user_id.as_ptr() as *mut u8,
            pwszName: PCWSTR(user_name.as_ptr()),
            pwszIcon: PCWSTR::null(),
            pwszDisplayName: PCWSTR(user_name.as_ptr()),
        };
        let mut parameter = WEBAUTHN_COSE_CREDENTIAL_PARAMETER {
            dwVersion: WEBAUTHN_COSE_CREDENTIAL_PARAMETER_CURRENT_VERSION,
            pwszCredentialType: WEBAUTHN_CREDENTIAL_TYPE_PUBLIC_KEY,
            lAlg: WEBAUTHN_COSE_ALGORITHM_RSASSA_PKCS1_V1_5_WITH_SHA256,
        };
        let parameters = WEBAUTHN_COSE_CREDENTIAL_PARAMETERS {
            cCredentialParameters: 1,
            pCredentialParameters: &mut parameter,
        };
        let client_data = client_data(client_data_json)?;
        let options = WEBAUTHN_AUTHENTICATOR_MAKE_CREDENTIAL_OPTIONS {
            dwVersion: WEBAUTHN_AUTHENTICATOR_MAKE_CREDENTIAL_OPTIONS_VERSION_1,
            dwTimeoutMilliseconds: 120_000,
            dwAuthenticatorAttachment: WEBAUTHN_AUTHENTICATOR_ATTACHMENT_PLATFORM,
            dwUserVerificationRequirement: WEBAUTHN_USER_VERIFICATION_REQUIREMENT_REQUIRED,
            dwAttestationConveyancePreference: WEBAUTHN_ATTESTATION_CONVEYANCE_PREFERENCE_NONE,
            ..Default::default()
        };
        let pointer = unsafe {
            WebAuthNAuthenticatorMakeCredential(
                hwnd,
                &rp,
                &user,
                &parameters,
                &client_data,
                Some(&options),
            )
        }
        .map_err(web_authn_outcome)?;
        let guard = CredentialAttestationGuard(pointer);
        let attestation = unsafe { guard.0.as_ref() }.ok_or_else(|| {
            Outcome::Failed("Windows WebAuthn returned an empty credential attestation".to_owned())
        })?;
        let credential_id = copy_bytes(
            attestation.pbCredentialId,
            attestation.cbCredentialId,
            "credential id",
        )?;
        let authenticator_data = copy_bytes(
            attestation.pbAuthenticatorData,
            attestation.cbAuthenticatorData,
            "registration authenticator data",
        )?;
        Ok((credential_id, authenticator_data))
    }

    fn get_assertion(
        client_data_json: &[u8],
        credential_id: &[u8],
    ) -> Result<WebAuthnAssertion, Outcome> {
        if credential_id.is_empty() || credential_id.len() > 1024 {
            return Err(Outcome::Failed(
                "Windows System Auth service returned an invalid credential id".to_owned(),
            ));
        }
        let hwnd = webauthn_window()?;
        let rp_id = wide_null(RP_ID);
        let client_data = client_data(client_data_json)?;
        let mut credential = WEBAUTHN_CREDENTIAL {
            dwVersion: WEBAUTHN_CREDENTIAL_CURRENT_VERSION,
            cbId: credential_id.len() as u32,
            pbId: credential_id.as_ptr() as *mut u8,
            pwszCredentialType: WEBAUTHN_CREDENTIAL_TYPE_PUBLIC_KEY,
        };
        let credentials = WEBAUTHN_CREDENTIALS {
            cCredentials: 1,
            pCredentials: &mut credential,
        };
        let options = WEBAUTHN_AUTHENTICATOR_GET_ASSERTION_OPTIONS {
            dwVersion: WEBAUTHN_AUTHENTICATOR_GET_ASSERTION_OPTIONS_VERSION_1,
            dwTimeoutMilliseconds: 120_000,
            CredentialList: credentials,
            dwAuthenticatorAttachment: WEBAUTHN_AUTHENTICATOR_ATTACHMENT_PLATFORM,
            dwUserVerificationRequirement: WEBAUTHN_USER_VERIFICATION_REQUIREMENT_REQUIRED,
            ..Default::default()
        };
        let pointer = unsafe {
            WebAuthNAuthenticatorGetAssertion(
                hwnd,
                PCWSTR(rp_id.as_ptr()),
                &client_data,
                Some(&options),
            )
        }
        .map_err(web_authn_outcome)?;
        let guard = AssertionGuard(pointer);
        let assertion = unsafe { guard.0.as_ref() }.ok_or_else(|| {
            Outcome::Failed("Windows WebAuthn returned an empty assertion".to_owned())
        })?;
        Ok(WebAuthnAssertion {
            credential_id: copy_credential_id(assertion)?,
            authenticator_data: copy_bytes(
                assertion.pbAuthenticatorData,
                assertion.cbAuthenticatorData,
                "assertion authenticator data",
            )?,
            signature: copy_bytes(assertion.pbSignature, assertion.cbSignature, "signature")?,
        })
    }

    fn client_data(bytes: &[u8]) -> Result<WEBAUTHN_CLIENT_DATA, Outcome> {
        if bytes.is_empty() || bytes.len() > 4096 {
            return Err(Outcome::Failed(
                "Windows System Auth service returned invalid WebAuthn client data".to_owned(),
            ));
        }
        Ok(WEBAUTHN_CLIENT_DATA {
            dwVersion: WEBAUTHN_CLIENT_DATA_CURRENT_VERSION,
            cbClientDataJSON: bytes.len() as u32,
            pbClientDataJSON: bytes.as_ptr() as *mut u8,
            pwszHashAlgId: WEBAUTHN_HASH_ALGORITHM_SHA_256,
        })
    }

    fn webauthn_window() -> Result<HWND, Outcome> {
        let console = unsafe { GetConsoleWindow() };
        if console.0.is_null() {
            return Err(Outcome::PassphraseRequired);
        }
        let owner = unsafe { GetAncestor(console, GA_ROOTOWNER) };
        if owner.0.is_null() {
            Ok(console)
        } else {
            Ok(owner)
        }
    }

    fn copy_credential_id(assertion: &WEBAUTHN_ASSERTION) -> Result<Vec<u8>, Outcome> {
        copy_bytes(
            assertion.Credential.pbId,
            assertion.Credential.cbId,
            "assertion credential id",
        )
    }

    fn copy_bytes(pointer: *const u8, length: u32, label: &str) -> Result<Vec<u8>, Outcome> {
        if pointer.is_null() || length == 0 || length as usize > 8192 {
            return Err(Outcome::Failed(format!(
                "Windows WebAuthn returned invalid {label}"
            )));
        }
        Ok(unsafe { std::slice::from_raw_parts(pointer, length as usize).to_vec() })
    }

    fn web_authn_outcome(error: windows::core::Error) -> Outcome {
        let name = unsafe { WebAuthNGetErrorName(error.code()) };
        let name = if name.is_null() {
            String::new()
        } else {
            unsafe { name.to_string() }.unwrap_or_default()
        };
        match name.as_str() {
            "NotAllowedError" => Outcome::Cancelled,
            "NotSupportedError" => Outcome::PassphraseRequired,
            _ => Outcome::Failed(if name.is_empty() {
                format!("Windows WebAuthn operation failed: {error}")
            } else {
                format!("Windows WebAuthn operation failed ({name}): {error}")
            }),
        }
    }

    fn connect() -> Result<File, String> {
        OpenOptions::new()
            .read(true)
            .write(true)
            .open(PIPE_NAME)
            .map_err(|error| {
                format!("unable to connect to Fresnica Windows System Auth service: {error}")
            })
    }

    fn exchange(pipe: &mut File, request: &Request) -> Result<Response, String> {
        write_frame(pipe, request)?;
        read_frame(pipe)
    }

    fn read_unlock_key() -> Result<Zeroizing<Vec<u8>>, String> {
        let mut key = Zeroizing::new(Vec::with_capacity(UNLOCK_KEY_LEN + 1));
        io::stdin()
            .take((UNLOCK_KEY_LEN + 1) as u64)
            .read_to_end(&mut key)
            .map_err(|error| {
                format!("unable to read Windows System Auth enrollment key: {error}")
            })?;
        if key.len() != UNLOCK_KEY_LEN {
            return Err("Windows System Auth enrollment requires exactly 32 key bytes".to_owned());
        }
        Ok(key)
    }

    fn require_slot(slot: &str) -> &str {
        if !valid_slot(slot) {
            finish(Outcome::Failed(
                "Windows System Auth provider requires an exact valid slot id".to_owned(),
            ));
        }
        slot
    }

    fn wide_null(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }

    struct WebAuthnAssertion {
        credential_id: Vec<u8>,
        authenticator_data: Vec<u8>,
        signature: Vec<u8>,
    }

    struct CredentialAttestationGuard(
        *mut windows::Win32::Networking::WindowsWebServices::WEBAUTHN_CREDENTIAL_ATTESTATION,
    );

    impl Drop for CredentialAttestationGuard {
        fn drop(&mut self) {
            unsafe { WebAuthNFreeCredentialAttestation(Some(self.0)) };
        }
    }

    struct AssertionGuard(*mut WEBAUTHN_ASSERTION);

    impl Drop for AssertionGuard {
        fn drop(&mut self) {
            unsafe { WebAuthNFreeAssertion(self.0) };
        }
    }

    enum Outcome {
        Success,
        PassphraseRequired,
        Cancelled,
        Failed(String),
    }

    fn finish(outcome: Outcome) -> ! {
        match outcome {
            Outcome::Success => std::process::exit(0),
            Outcome::PassphraseRequired => std::process::exit(EXIT_PASSPHRASE_REQUIRED),
            Outcome::Cancelled => std::process::exit(EXIT_CANCELLED),
            Outcome::Failed(message) => {
                eprintln!("{message}");
                std::process::exit(EXIT_FAILURE)
            }
        }
    }
}
