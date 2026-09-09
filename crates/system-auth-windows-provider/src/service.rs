#[cfg(not(windows))]
fn main() {
    eprintln!("Fresnica Windows System Auth service is Windows-only");
    std::process::exit(20);
}

#[cfg(windows)]
fn main() -> windows_service::Result<()> {
    windows_service_impl::run()
}

#[cfg(windows)]
mod windows_service_impl {
    use fresnica_system_auth_windows::{
        make_client_data, read_frame, sid_storage_id, slot_storage_id, valid_slot,
        validate_registration_authenticator_data, verify_webauthn_assertion, write_frame,
        DomainRecord, Request, Response, SignerRecord, CHALLENGE_LEN, PIPE_NAME, SERVICE_NAME,
        UNLOCK_KEY_LEN,
    };
    use sha2::{Digest, Sha256};
    use std::ffi::{c_void, OsString};
    use std::fs::{self, File, OpenOptions};
    use std::io;
    use std::os::windows::io::FromRawHandle;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::Duration;
    use windows::core::{PCWSTR, PWSTR};
    use windows::Win32::Foundation::{
        CloseHandle, GetLastError, LocalFree, ERROR_INSUFFICIENT_BUFFER, ERROR_PIPE_CONNECTED,
        HANDLE, HLOCAL, INVALID_HANDLE_VALUE,
    };
    use windows::Win32::Security::Authorization::{
        ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
        SDDL_REVISION_1,
    };
    use windows::Win32::Security::Cryptography::{
        BCryptGenRandom, CryptProtectData, CryptUnprotectData, BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };
    use windows::Win32::Security::{
        GetTokenInformation, TokenUser, PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES, TOKEN_QUERY,
        TOKEN_USER,
    };
    use windows::Win32::Storage::FileSystem::PIPE_ACCESS_DUPLEX;
    use windows::Win32::System::Pipes::{
        ConnectNamedPipe, CreateNamedPipeW, GetNamedPipeClientProcessId, PIPE_READMODE_BYTE,
        PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE, PIPE_WAIT,
    };
    use windows::Win32::System::Threading::{
        OpenProcess, OpenProcessToken, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows_service::define_windows_service;
    use windows_service::service::{
        ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
        ServiceType,
    };
    use windows_service::service_control_handler::{self, ServiceControlHandlerResult};
    use windows_service::service_dispatcher;
    use zeroize::{Zeroize, Zeroizing};

    const PIPE_SDDL: &str = "D:P(A;;GA;;;SY)(A;;GA;;;BA)(A;;GA;;;AU)";
    const PIPE_BUFFER: u32 = 32 * 1024;
    const STATE_SUBDIR: &str = r"Fresnica\SystemAuth";
    const SERVICE_TYPE: ServiceType = ServiceType::OWN_PROCESS;

    define_windows_service!(ffi_service_main, service_main);

    pub(super) fn run() -> windows_service::Result<()> {
        service_dispatcher::start(SERVICE_NAME, ffi_service_main)
    }

    pub fn service_main(_arguments: Vec<OsString>) {
        let _ = run_service();
    }

    fn run_service() -> windows_service::Result<()> {
        let stopping = Arc::new(AtomicBool::new(false));
        let handler_stop = Arc::clone(&stopping);
        let event_handler = move |event| -> ServiceControlHandlerResult {
            match event {
                ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
                ServiceControl::Stop => {
                    handler_stop.store(true, Ordering::SeqCst);
                    let _ = wake_pipe();
                    ServiceControlHandlerResult::NoError
                }
                _ => ServiceControlHandlerResult::NotImplemented,
            }
        };
        let status_handle = service_control_handler::register(SERVICE_NAME, event_handler)?;
        status_handle.set_service_status(ServiceStatus {
            service_type: SERVICE_TYPE,
            current_state: ServiceState::Running,
            controls_accepted: ServiceControlAccept::STOP,
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::default(),
            process_id: None,
        })?;

        let storage = Storage::production();
        while !stopping.load(Ordering::SeqCst) {
            match accept_client() {
                Ok((mut pipe, sid)) => {
                    if stopping.load(Ordering::SeqCst) {
                        break;
                    }
                    if let Err(error) = handle_client(&mut pipe, &storage, &sid) {
                        let _ = write_frame(&mut pipe, &Response::Error { message: error });
                    }
                }
                Err(_) if stopping.load(Ordering::SeqCst) => break,
                Err(_) => continue,
            }
        }

        status_handle.set_service_status(ServiceStatus {
            service_type: SERVICE_TYPE,
            current_state: ServiceState::Stopped,
            controls_accepted: ServiceControlAccept::empty(),
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::default(),
            process_id: None,
        })?;
        Ok(())
    }

    fn handle_client(pipe: &mut File, storage: &Storage, sid: &str) -> Result<(), String> {
        let request: Request = read_frame(pipe)?;
        match request {
            Request::Probe => write_frame(pipe, &Response::Ok),
            Request::Has { slot } => handle_has(pipe, storage, sid, &slot),
            Request::Enroll {
                slot,
                mut unlock_key,
            } => {
                let result = handle_enroll(pipe, storage, sid, &slot, &unlock_key);
                unlock_key.zeroize();
                result
            }
            Request::Release { slot } => handle_release(pipe, storage, sid, &slot),
            Request::Delete { slot } => handle_delete(pipe, storage, sid, &slot),
            Request::Credential { .. } | Request::Assertion { .. } => {
                Err("unexpected Windows System Auth proof frame".to_owned())
            }
        }
    }

    fn handle_has(pipe: &mut File, storage: &Storage, sid: &str, slot: &str) -> Result<(), String> {
        require_slot(slot)?;
        let Some(domain) = storage.read_domain(sid)? else {
            return write_frame(pipe, &Response::Missing);
        };
        domain.validate(sid)?;
        let Some(mut signer) = storage.read_signer(sid, slot)? else {
            return write_frame(pipe, &Response::Missing);
        };
        let validation = signer.validate(sid, slot, &domain);
        signer.unlock_key.zeroize();
        validation?;
        write_frame(pipe, &Response::Ok)
    }

    fn handle_enroll(
        pipe: &mut File,
        storage: &Storage,
        sid: &str,
        slot: &str,
        unlock_key: &[u8],
    ) -> Result<(), String> {
        require_slot(slot)?;
        require_unlock_key(unlock_key)?;
        let domain = match storage.read_domain(sid)? {
            Some(domain) => {
                domain.validate(sid)?;
                domain
            }
            None => register_domain(pipe, storage, sid)?,
        };
        let mut signer = SignerRecord::new(sid, slot, &domain.fingerprint, unlock_key.to_vec());
        let stored = storage.write_signer(sid, slot, &signer);
        signer.unlock_key.zeroize();
        stored?;
        write_frame(pipe, &Response::Ok)
    }

    fn register_domain(
        pipe: &mut File,
        storage: &Storage,
        sid: &str,
    ) -> Result<DomainRecord, String> {
        let mut challenge = random_challenge()?;
        let client_data_json = make_client_data("webauthn.create", &challenge)?;
        challenge.zeroize();
        let user_id = Sha256::digest(sid.as_bytes()).to_vec();
        write_frame(
            pipe,
            &Response::CreateCredential {
                client_data_json,
                user_id,
            },
        )?;
        let registration: Request = read_frame(pipe)?;
        let Request::Credential {
            credential_id,
            authenticator_data,
        } = registration
        else {
            return Err("Windows System Auth service expected a credential frame".to_owned());
        };
        let public_key_cose =
            validate_registration_authenticator_data(&authenticator_data, &credential_id)?;
        let domain = DomainRecord::new(sid, credential_id, public_key_cose);
        domain.validate(sid)?;
        storage.replace_domain(sid, &domain)?;
        Ok(domain)
    }

    fn handle_release(
        pipe: &mut File,
        storage: &Storage,
        sid: &str,
        slot: &str,
    ) -> Result<(), String> {
        require_slot(slot)?;
        let Some(domain) = storage.read_domain(sid)? else {
            return write_frame(pipe, &Response::Missing);
        };
        domain.validate(sid)?;
        let Some(mut signer) = storage.read_signer(sid, slot)? else {
            return write_frame(pipe, &Response::Missing);
        };
        if let Err(error) = signer.validate(sid, slot, &domain) {
            signer.unlock_key.zeroize();
            return Err(error);
        }

        let mut challenge = random_challenge()?;
        let client_data_json = make_client_data("webauthn.get", &challenge)?;
        challenge.zeroize();
        write_frame(
            pipe,
            &Response::AssertionChallenge {
                client_data_json: client_data_json.clone(),
                credential_id: domain.credential_id.clone(),
            },
        )?;
        let proof: Request = read_frame(pipe)?;
        let Request::Assertion {
            credential_id,
            authenticator_data,
            signature,
        } = proof
        else {
            signer.unlock_key.zeroize();
            return Err("Windows System Auth service expected an assertion frame".to_owned());
        };
        if let Err(error) = verify_webauthn_assertion(
            &domain.public_key_cose,
            &domain.credential_id,
            &credential_id,
            &client_data_json,
            &authenticator_data,
            &signature,
        ) {
            signer.unlock_key.zeroize();
            return Err(error);
        }
        let mut response = Response::Key {
            unlock_key: std::mem::take(&mut signer.unlock_key),
        };
        let result = write_frame(pipe, &response);
        if let Response::Key { unlock_key } = &mut response {
            unlock_key.zeroize();
        }
        result
    }

    fn handle_delete(
        pipe: &mut File,
        storage: &Storage,
        sid: &str,
        slot: &str,
    ) -> Result<(), String> {
        require_slot(slot)?;
        let removed = storage.delete_signer(sid, slot)?;
        write_frame(
            pipe,
            if removed {
                &Response::Ok
            } else {
                &Response::Missing
            },
        )
    }

    fn random_challenge() -> Result<Zeroizing<Vec<u8>>, String> {
        let mut challenge = Zeroizing::new(vec![0u8; CHALLENGE_LEN]);
        let status = unsafe {
            BCryptGenRandom(
                None,
                challenge.as_mut_slice(),
                BCRYPT_USE_SYSTEM_PREFERRED_RNG,
            )
        };
        if status.0 < 0 {
            return Err(format!(
                "Windows System Auth challenge generation failed with status 0x{:08x}",
                status.0 as u32
            ));
        }
        Ok(challenge)
    }

    fn require_slot(slot: &str) -> Result<(), String> {
        if valid_slot(slot) {
            Ok(())
        } else {
            Err("Windows System Auth service requires an exact valid slot id".to_owned())
        }
    }

    fn require_unlock_key(key: &[u8]) -> Result<(), String> {
        if key.len() == UNLOCK_KEY_LEN {
            Ok(())
        } else {
            Err("Windows System Auth enrollment requires exactly 32 key bytes".to_owned())
        }
    }

    struct Storage {
        root: PathBuf,
    }

    impl Storage {
        fn production() -> Self {
            let base = std::env::var_os("ProgramData")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(r"C:\ProgramData"));
            Self {
                root: base.join(STATE_SUBDIR),
            }
        }

        fn sid_dir(&self, sid: &str) -> PathBuf {
            self.root.join(sid_storage_id(sid))
        }

        fn domain_path(&self, sid: &str) -> PathBuf {
            self.sid_dir(sid).join("domain.bin")
        }

        fn signer_path(&self, sid: &str, slot: &str) -> PathBuf {
            self.sid_dir(sid)
                .join(format!("signer-{}.bin", slot_storage_id(slot)))
        }

        fn read_domain(&self, sid: &str) -> Result<Option<DomainRecord>, String> {
            self.read_record(&self.domain_path(sid))
        }

        fn read_signer(&self, sid: &str, slot: &str) -> Result<Option<SignerRecord>, String> {
            self.read_record(&self.signer_path(sid, slot))
        }

        fn read_record<T: serde::de::DeserializeOwned>(
            &self,
            path: &Path,
        ) -> Result<Option<T>, String> {
            let protected = match fs::read(path) {
                Ok(value) => value,
                Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
                Err(error) => {
                    return Err(format!(
                        "unable to read Windows System Auth state {}: {error}",
                        path.display()
                    ))
                }
            };
            let clear = dpapi_unprotect(&protected)?;
            serde_json::from_slice(&clear)
                .map(Some)
                .map_err(|_| format!("Windows System Auth state is malformed: {}", path.display()))
        }

        fn write_record<T: serde::Serialize>(&self, path: &Path, value: &T) -> Result<(), String> {
            let parent = path
                .parent()
                .ok_or_else(|| "Windows System Auth state path has no parent".to_owned())?;
            fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "unable to create Windows System Auth state directory {}: {error}",
                    parent.display()
                )
            })?;
            let clear =
                Zeroizing::new(serde_json::to_vec(value).map_err(|error| {
                    format!("unable to encode Windows System Auth state: {error}")
                })?);
            let protected = dpapi_protect(&clear)?;
            fs::write(path, protected).map_err(|error| {
                format!(
                    "unable to write Windows System Auth state {}: {error}",
                    path.display()
                )
            })
        }

        fn replace_domain(&self, sid: &str, domain: &DomainRecord) -> Result<(), String> {
            let dir = self.sid_dir(sid);
            fs::create_dir_all(&dir).map_err(|error| {
                format!(
                    "unable to create Windows System Auth SID directory {}: {error}",
                    dir.display()
                )
            })?;
            for entry in fs::read_dir(&dir)
                .map_err(|error| format!("unable to inspect Windows System Auth state: {error}"))?
            {
                let entry = entry.map_err(|error| {
                    format!("unable to inspect Windows System Auth state: {error}")
                })?;
                if entry.file_name().to_string_lossy().starts_with("signer-") {
                    fs::remove_file(entry.path()).map_err(|error| {
                        format!("unable to clear stale Windows System Auth signer state: {error}")
                    })?;
                }
            }
            self.write_record(&self.domain_path(sid), domain)
        }

        fn write_signer(&self, sid: &str, slot: &str, signer: &SignerRecord) -> Result<(), String> {
            self.write_record(&self.signer_path(sid, slot), signer)
        }

        fn delete_signer(&self, sid: &str, slot: &str) -> Result<bool, String> {
            let path = self.signer_path(sid, slot);
            match fs::remove_file(&path) {
                Ok(()) => Ok(true),
                Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
                Err(error) => Err(format!(
                    "unable to remove Windows System Auth signer state {}: {error}",
                    path.display()
                )),
            }
        }
    }

    fn dpapi_protect(clear: &[u8]) -> Result<Vec<u8>, String> {
        let input = CRYPT_INTEGER_BLOB {
            cbData: clear
                .len()
                .try_into()
                .map_err(|_| "Windows System Auth state is too large".to_owned())?,
            pbData: clear.as_ptr() as *mut u8,
        };
        let mut output = CRYPT_INTEGER_BLOB::default();
        let description = wide_null("Fresnica System Auth");
        unsafe {
            CryptProtectData(
                &input,
                PCWSTR(description.as_ptr()),
                None,
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        }
        .map_err(|error| format!("unable to protect Windows System Auth state: {error}"))?;
        copy_local_blob(&output, false)
    }

    fn dpapi_unprotect(protected: &[u8]) -> Result<Zeroizing<Vec<u8>>, String> {
        let input = CRYPT_INTEGER_BLOB {
            cbData: protected
                .len()
                .try_into()
                .map_err(|_| "Windows System Auth protected state is too large".to_owned())?,
            pbData: protected.as_ptr() as *mut u8,
        };
        let mut output = CRYPT_INTEGER_BLOB::default();
        unsafe {
            CryptUnprotectData(
                &input,
                None,
                None,
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        }
        .map_err(|error| format!("unable to unprotect Windows System Auth state: {error}"))?;
        copy_local_blob(&output, true).map(Zeroizing::new)
    }

    fn copy_local_blob(
        blob: &CRYPT_INTEGER_BLOB,
        zero_before_free: bool,
    ) -> Result<Vec<u8>, String> {
        if blob.pbData.is_null() || blob.cbData == 0 {
            return Err("Windows DPAPI returned an empty state blob".to_owned());
        }
        let result =
            unsafe { std::slice::from_raw_parts(blob.pbData, blob.cbData as usize).to_vec() };
        unsafe {
            if zero_before_free {
                std::ptr::write_bytes(blob.pbData, 0, blob.cbData as usize);
            }
            let _ = LocalFree(Some(HLOCAL(blob.pbData.cast::<c_void>())));
        }
        Ok(result)
    }

    fn accept_client() -> Result<(File, String), String> {
        let security = PipeSecurity::new()?;
        let name = wide_null(PIPE_NAME);
        let mode = PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS;
        let handle = unsafe {
            CreateNamedPipeW(
                PCWSTR(name.as_ptr()),
                PIPE_ACCESS_DUPLEX,
                mode,
                1,
                PIPE_BUFFER,
                PIPE_BUFFER,
                0,
                Some(&security.attributes),
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            return Err(format!(
                "unable to create Windows System Auth named pipe: {}",
                windows::core::Error::from_thread()
            ));
        }
        let owned = OwnedHandle(handle);
        match unsafe { ConnectNamedPipe(handle, None) } {
            Ok(()) => {}
            Err(_error) if unsafe { GetLastError() } == ERROR_PIPE_CONNECTED => {}
            Err(error) => {
                return Err(format!(
                    "unable to accept Windows System Auth client: {error}"
                ))
            }
        }
        let mut client_pid = 0u32;
        unsafe { GetNamedPipeClientProcessId(handle, &mut client_pid) }
            .map_err(|error| format!("unable to identify Windows System Auth client: {error}"))?;
        let (sid, image_path) = process_identity(client_pid)?;
        require_trusted_provider_path(&image_path)?;
        let raw = owned.into_raw();
        let file = unsafe { File::from_raw_handle(raw.0) };
        Ok((file, sid))
    }

    fn process_identity(pid: u32) -> Result<(String, PathBuf), String> {
        let process = OwnedHandle(
            unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.map_err(
                |error| format!("unable to open Windows System Auth client process: {error}"),
            )?,
        );
        let mut image = vec![0u16; 32768];
        let mut image_len = image.len() as u32;
        unsafe {
            QueryFullProcessImageNameW(
                process.0,
                PROCESS_NAME_WIN32,
                PWSTR(image.as_mut_ptr()),
                &mut image_len,
            )
        }
        .map_err(|error| {
            format!("unable to identify Windows System Auth client executable: {error}")
        })?;
        image.truncate(image_len as usize);
        let image_path = PathBuf::from(String::from_utf16(&image).map_err(|_| {
            "Windows System Auth client executable path is invalid UTF-16".to_owned()
        })?);

        let mut token_raw = HANDLE::default();
        unsafe { OpenProcessToken(process.0, TOKEN_QUERY, &mut token_raw) }
            .map_err(|error| format!("unable to open Windows System Auth client token: {error}"))?;
        let token = OwnedHandle(token_raw);
        let mut required = 0u32;
        let first = unsafe { GetTokenInformation(token.0, TokenUser, None, 0, &mut required) };
        if first.is_ok() || unsafe { GetLastError() } != ERROR_INSUFFICIENT_BUFFER || required == 0
        {
            return Err("unable to size Windows System Auth client token".to_owned());
        }
        let mut buffer = vec![0u8; required as usize];
        unsafe {
            GetTokenInformation(
                token.0,
                TokenUser,
                Some(buffer.as_mut_ptr().cast()),
                required,
                &mut required,
            )
        }
        .map_err(|error| format!("unable to read Windows System Auth client token: {error}"))?;
        let token_user = unsafe { &*(buffer.as_ptr() as *const TOKEN_USER) };
        let mut string_sid = PWSTR::null();
        unsafe { ConvertSidToStringSidW(token_user.User.Sid, &mut string_sid) }
            .map_err(|error| format!("unable to format Windows System Auth client SID: {error}"))?;
        let text = unsafe { string_sid.to_string() }
            .map_err(|error| format!("unable to decode Windows System Auth client SID: {error}"))?;
        unsafe {
            let _ = LocalFree(Some(HLOCAL(string_sid.0.cast::<c_void>())));
        }
        Ok((text, image_path))
    }

    fn require_trusted_provider_path(client: &Path) -> Result<(), String> {
        let base = std::env::var_os("ProgramFiles")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\Program Files"));
        let expected = base
            .join("Fresnica")
            .join("SystemAuth")
            .join("fresnica-system-auth-provider.exe");
        let expected = fs::canonicalize(&expected).map_err(|error| {
            format!(
                "unable to resolve trusted Windows System Auth provider {}: {error}",
                expected.display()
            )
        })?;
        let client = fs::canonicalize(client).map_err(|error| {
            format!(
                "unable to resolve Windows System Auth client executable {}: {error}",
                client.display()
            )
        })?;
        if expected
            .to_string_lossy()
            .eq_ignore_ascii_case(&client.to_string_lossy())
        {
            Ok(())
        } else {
            Err("Windows System Auth rejected an untrusted client executable".to_owned())
        }
    }

    fn wake_pipe() -> io::Result<()> {
        OpenOptions::new()
            .read(true)
            .write(true)
            .open(PIPE_NAME)
            .map(|_| ())
    }

    struct PipeSecurity {
        descriptor: PSECURITY_DESCRIPTOR,
        attributes: SECURITY_ATTRIBUTES,
    }

    impl PipeSecurity {
        fn new() -> Result<Self, String> {
            let sddl = wide_null(PIPE_SDDL);
            let mut descriptor = PSECURITY_DESCRIPTOR::default();
            unsafe {
                ConvertStringSecurityDescriptorToSecurityDescriptorW(
                    PCWSTR(sddl.as_ptr()),
                    SDDL_REVISION_1,
                    &mut descriptor,
                    None,
                )
            }
            .map_err(|error| format!("unable to create Windows System Auth pipe ACL: {error}"))?;
            let attributes = SECURITY_ATTRIBUTES {
                nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
                lpSecurityDescriptor: descriptor.0,
                bInheritHandle: false.into(),
            };
            Ok(Self {
                descriptor,
                attributes,
            })
        }
    }

    impl Drop for PipeSecurity {
        fn drop(&mut self) {
            unsafe {
                let _ = LocalFree(Some(HLOCAL(self.descriptor.0)));
            }
        }
    }

    struct OwnedHandle(HANDLE);

    impl OwnedHandle {
        fn into_raw(self) -> HANDLE {
            let handle = self.0;
            std::mem::forget(self);
            handle
        }
    }

    impl Drop for OwnedHandle {
        fn drop(&mut self) {
            if !self.0.is_invalid() {
                unsafe {
                    let _ = CloseHandle(self.0);
                }
            }
        }
    }

    fn wide_null(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }
}
