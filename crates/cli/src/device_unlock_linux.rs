use std::collections::HashMap;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

use fresnica_client::{SystemAuthRelease, SystemAuthSlot, SYSTEM_AUTH_UNLOCK_KEY_LENGTH};
use secret_service::blocking::SecretService;
use secret_service::EncryptionType;
use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::OwnedValue;
use zeroize::Zeroize;

use crate::device_unlock::{
    DeviceAuthenticationOutcome, DeviceAuthenticator, DeviceSecretRead, DeviceSecretStore,
    DeviceUnlockBackend, DeviceUnlockState,
};

const PROVIDER_NAME: &str = "Linux system authentication";
const ITEM_LABEL: &str = "Fresnica Device Unlock";
const CONTENT_TYPE: &str = "application/octet-stream";
const POLKIT_SERVICE: &str = "org.freedesktop.PolicyKit1";
const POLKIT_PATH: &str = "/org/freedesktop/PolicyKit1/Authority";
const POLKIT_INTERFACE: &str = "org.freedesktop.PolicyKit1.Authority";
const POLKIT_ALLOW_USER_INTERACTION: u32 = 1;
const POLKIT_POLICY_DIR: &str = "/usr/share/polkit-1/actions";
const PKEXEC_PATH: &str = "/usr/bin/pkexec";
const SUDO_PATH: &str = "/usr/bin/sudo";
const INTERNAL_POLICY_COMMAND: &str = "__device-unlock-policy";
const POLKIT_POLICY_TEMPLATE: &str =
    include_str!("../../../packaging/linux/com.fresnica.device-unlock.policy.in");

pub(crate) fn backend() -> Arc<dyn DeviceUnlockBackend> {
    Arc::new(LinuxDeviceUnlockBackend {
        authenticator: LinuxPolkitAuthenticator {
            authenticated: Mutex::new(false),
        },
        store: LinuxSecretServiceStore,
    })
}

pub(crate) fn internal_policy_command(arguments: &[String]) -> Option<Result<(), String>> {
    if arguments.first().map(String::as_str) != Some(INTERNAL_POLICY_COMMAND) {
        return None;
    }
    Some(handle_internal_policy_command(&arguments[1..]))
}

struct LinuxDeviceUnlockBackend {
    authenticator: LinuxPolkitAuthenticator,
    store: LinuxSecretServiceStore,
}

struct LinuxPolkitAuthenticator {
    authenticated: Mutex<bool>,
}

struct LinuxSecretServiceStore;

impl DeviceUnlockBackend for LinuxDeviceUnlockBackend {
    fn provider_name(&self) -> &'static str {
        PROVIDER_NAME
    }

    fn state(&self, slot: &SystemAuthSlot) -> Result<DeviceUnlockState, String> {
        self.store.state(slot)
    }

    fn prepare_system_support(&self) -> Result<(), String> {
        self.store.ensure_unlocked()?;
        ensure_polkit_policy_current()
    }

    fn authorize_enrollment(&self) -> Result<(), String> {
        self.store.ensure_unlocked()?;
        match self.authenticator.verify()? {
            PolkitAuthenticationOutcome::Verified => Ok(()),
            PolkitAuthenticationOutcome::Cancelled => Err(
                "Linux system authentication cancelled; Device Unlock was not enabled".to_owned(),
            ),
            PolkitAuthenticationOutcome::Unavailable(reason) => Err(format!(
                "Linux system authentication unavailable ({reason}); Device Unlock was not enabled"
            )),
        }
    }

    fn cleanup_system_support_if_unused(&self) -> Result<(), String> {
        if !self.store.has_any_enrollment()? {
            remove_polkit_policy()?;
        }
        Ok(())
    }

    fn enroll(&self, slot: &SystemAuthSlot, unlock_key: &[u8]) -> Result<(), String> {
        self.store.enroll(slot, unlock_key)
    }

    fn release(&self, slot: &SystemAuthSlot) -> SystemAuthRelease {
        match self.store.state(slot) {
            Ok(DeviceUnlockState::Ready) => {}
            Ok(DeviceUnlockState::Locked) => {
                eprintln!("Desktop keyring is locked; Fresnica Passphrase required.");
                return SystemAuthRelease::PassphraseRequired;
            }
            Ok(DeviceUnlockState::Disabled | DeviceUnlockState::Unavailable) => {
                return SystemAuthRelease::PassphraseRequired
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

#[derive(Debug, PartialEq, Eq)]
enum PolkitAuthenticationOutcome {
    Verified,
    Cancelled,
    Unavailable(String),
}

#[derive(Debug, PartialEq, Eq)]
struct PolkitAuthorization {
    authorized: bool,
    challenge: bool,
    dismissed: bool,
}

impl LinuxPolkitAuthenticator {
    fn verify(&self) -> Result<PolkitAuthenticationOutcome, String> {
        let mut authenticated = self
            .authenticated
            .lock()
            .map_err(|_| "Linux system authentication state is unavailable".to_owned())?;
        if *authenticated {
            return Ok(PolkitAuthenticationOutcome::Verified);
        }

        let outcome = verify_current_user_with_polkit()?;
        if outcome == PolkitAuthenticationOutcome::Verified {
            *authenticated = true;
        }
        Ok(outcome)
    }
}

impl DeviceAuthenticator for LinuxPolkitAuthenticator {
    fn authenticate(&self) -> Result<DeviceAuthenticationOutcome, String> {
        match self.verify()? {
            PolkitAuthenticationOutcome::Verified => Ok(DeviceAuthenticationOutcome::Authenticated),
            PolkitAuthenticationOutcome::Cancelled => Ok(DeviceAuthenticationOutcome::Cancelled),
            PolkitAuthenticationOutcome::Unavailable(reason) => {
                eprintln!(
                    "Linux system authentication unavailable ({reason}); Fresnica Passphrase required."
                );
                Ok(DeviceAuthenticationOutcome::PassphraseRequired)
            }
        }
    }
}

fn verify_current_user_with_polkit() -> Result<PolkitAuthenticationOutcome, String> {
    let uid = current_user_id()?;
    if !polkit_policy_current(uid)? {
        return Ok(PolkitAuthenticationOutcome::Unavailable(
            "Device Unlock system support is missing or outdated; run `fresnica wallet device-unlock enable <wallet>` to refresh it"
                .to_owned(),
        ));
    }

    let connection = match Connection::system() {
        Ok(connection) => connection,
        Err(error) => {
            return Ok(PolkitAuthenticationOutcome::Unavailable(format!(
                "unable to connect to the system bus: {error}"
            )))
        }
    };
    let authority = Proxy::new(&connection, POLKIT_SERVICE, POLKIT_PATH, POLKIT_INTERFACE)
        .map_err(|error| format!("unable to open Polkit authority: {error}"))?;
    let subject = current_process_subject()?;
    let action_id = polkit_action_id(uid);

    let preflight = check_polkit_authorization(&authority, &subject, &action_id, 0)?;
    if preflight.authorized {
        return Ok(PolkitAuthenticationOutcome::Unavailable(
            "Polkit action is already authorized without fresh authentication".to_owned(),
        ));
    }
    if !preflight.challenge {
        return Ok(PolkitAuthenticationOutcome::Unavailable(
            "current session is not eligible for Polkit user authentication".to_owned(),
        ));
    }

    let result = check_polkit_authorization(
        &authority,
        &subject,
        &action_id,
        POLKIT_ALLOW_USER_INTERACTION,
    )?;
    Ok(classify_interactive_polkit_result(result))
}

fn check_polkit_authorization(
    authority: &Proxy<'_>,
    subject: &HashMap<&'static str, OwnedValue>,
    action_id: &str,
    flags: u32,
) -> Result<PolkitAuthorization, String> {
    let subject = ("unix-process", subject.clone());
    let details = HashMap::<&str, &str>::new();
    let (authorized, challenge, result_details): (bool, bool, HashMap<String, String>) = authority
        .call(
            "CheckAuthorization",
            &(subject, action_id, details, flags, ""),
        )
        .map_err(|error| format!("Polkit authorization check failed: {error}"))?;
    Ok(PolkitAuthorization {
        authorized,
        challenge,
        dismissed: result_details
            .get("polkit.dismissed")
            .is_some_and(|value| !value.is_empty()),
    })
}

fn classify_interactive_polkit_result(result: PolkitAuthorization) -> PolkitAuthenticationOutcome {
    if result.authorized {
        PolkitAuthenticationOutcome::Verified
    } else if result.dismissed {
        PolkitAuthenticationOutcome::Cancelled
    } else {
        PolkitAuthenticationOutcome::Unavailable(
            "no suitable authentication agent was available or authorization was denied".to_owned(),
        )
    }
}

fn ensure_polkit_policy_current() -> Result<(), String> {
    let uid = current_user_id()?;
    if polkit_policy_current(uid)? {
        return Ok(());
    }
    eprintln!("Linux Device Unlock requires one-time system setup.");
    eprintln!("Administrator authentication may be requested.");
    run_privileged_policy_command("install", uid)?;
    if !polkit_policy_current(uid)? {
        return Err("Linux Device Unlock system policy was not installed correctly".to_owned());
    }
    Ok(())
}

fn remove_polkit_policy() -> Result<(), String> {
    let uid = current_user_id()?;
    if !polkit_policy_path(uid).exists() {
        return Ok(());
    }
    eprintln!("Removing Linux Device Unlock system support.");
    eprintln!("Administrator authentication may be requested.");
    run_privileged_policy_command("remove", uid)?;
    if polkit_policy_path(uid).exists() {
        return Err("Linux Device Unlock system policy could not be removed".to_owned());
    }
    Ok(())
}

fn run_privileged_policy_command(operation: &str, uid: u32) -> Result<(), String> {
    let executable = env::current_exe()
        .map_err(|error| format!("unable to locate the Fresnica executable: {error}"))?;
    let mut command = if Path::new(PKEXEC_PATH).is_file() {
        let mut command = Command::new(PKEXEC_PATH);
        command.arg(&executable);
        command
    } else if Path::new(SUDO_PATH).is_file() {
        let mut command = Command::new(SUDO_PATH);
        command.arg("--").arg(&executable);
        command
    } else {
        return Err(
            "Linux Device Unlock needs a one-time administrator setup, but neither pkexec nor sudo is available; install either tool and retry `device-unlock enable`"
                .to_owned(),
        );
    };
    let status = command
        .arg(INTERNAL_POLICY_COMMAND)
        .arg(operation)
        .arg(uid.to_string())
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|error| format!("unable to start Linux system setup: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(match status.code() {
            Some(126 | 127) => "administrator authorization was cancelled or unavailable; retry `device-unlock enable` when ready".to_owned(),
            Some(code) => format!("Linux system setup failed with exit code {code}"),
            None => "Linux system setup did not complete".to_owned(),
        })
    }
}

fn handle_internal_policy_command(arguments: &[String]) -> Result<(), String> {
    let [operation, uid_text] = arguments else {
        return Err("invalid internal Device Unlock policy command".to_owned());
    };
    let uid = uid_text
        .parse::<u32>()
        .map_err(|_| "invalid Device Unlock policy user id".to_owned())?;
    let invoking_uid = env::var("PKEXEC_UID")
        .or_else(|_| env::var("SUDO_UID"))
        .map_err(|_| {
            "Device Unlock policy setup must be launched through pkexec or sudo".to_owned()
        })?
        .parse::<u32>()
        .map_err(|_| "administrator setup reported an invalid invoking user id".to_owned())?;
    if uid != invoking_uid {
        return Err("Device Unlock policy user id does not match the pkexec caller".to_owned());
    }
    let (real_uid, effective_uid) = current_uids()?;
    if real_uid != 0 || effective_uid != 0 {
        return Err("Device Unlock policy setup did not receive root privileges".to_owned());
    }

    match operation.as_str() {
        "install" => write_polkit_policy(uid),
        "remove" => remove_polkit_policy_file(uid),
        _ => Err("invalid internal Device Unlock policy operation".to_owned()),
    }
}

fn write_polkit_policy(uid: u32) -> Result<(), String> {
    fs::create_dir_all(POLKIT_POLICY_DIR)
        .map_err(|error| format!("unable to create Polkit action directory: {error}"))?;
    let target = polkit_policy_path(uid);
    let temp = policy_temp_path(uid);
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|error| format!("unable to create temporary Polkit policy: {error}"))?;
        file.write_all(render_polkit_policy(uid).as_bytes())
            .map_err(|error| format!("unable to write Polkit policy: {error}"))?;
        file.sync_all()
            .map_err(|error| format!("unable to sync Polkit policy: {error}"))?;
        fs::set_permissions(&temp, fs::Permissions::from_mode(0o644))
            .map_err(|error| format!("unable to set Polkit policy permissions: {error}"))?;
        fs::rename(&temp, &target)
            .map_err(|error| format!("unable to install Polkit policy: {error}"))?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

fn remove_polkit_policy_file(uid: u32) -> Result<(), String> {
    match fs::remove_file(polkit_policy_path(uid)) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("unable to remove Polkit policy: {error}")),
    }
}

fn polkit_action_id(uid: u32) -> String {
    format!("com.fresnica.device-unlock.authenticate.{uid}")
}

fn polkit_policy_path(uid: u32) -> PathBuf {
    Path::new(POLKIT_POLICY_DIR).join(format!("com.fresnica.device-unlock.{uid}.policy"))
}

fn policy_temp_path(uid: u32) -> PathBuf {
    Path::new(POLKIT_POLICY_DIR).join(format!(
        ".com.fresnica.device-unlock.{uid}.{}.tmp",
        std::process::id()
    ))
}

fn render_polkit_policy(uid: u32) -> String {
    POLKIT_POLICY_TEMPLATE.replace("@UID@", &uid.to_string())
}

fn polkit_policy_current(uid: u32) -> Result<bool, String> {
    let path = polkit_policy_path(uid);
    match fs::read_to_string(&path) {
        Ok(contents) => Ok(contents == render_polkit_policy(uid)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!(
            "unable to inspect Linux Device Unlock system policy: {error}"
        )),
    }
}

fn current_user_id() -> Result<u32, String> {
    let (real_uid, effective_uid) = current_uids()?;
    if real_uid != effective_uid {
        return Err("Linux Device Unlock does not run from a setuid/elevated process".to_owned());
    }
    if real_uid == 0 {
        return Err("Linux Device Unlock is unavailable for the root account".to_owned());
    }
    Ok(real_uid)
}

fn current_process_subject() -> Result<HashMap<&'static str, OwnedValue>, String> {
    let uid = i32::try_from(current_user_id()?)
        .map_err(|_| "current Linux user id is outside the Polkit range".to_owned())?;
    Ok(HashMap::from([
        ("pid", OwnedValue::from(std::process::id())),
        (
            "start-time",
            OwnedValue::from(current_process_start_time()?),
        ),
        ("uid", OwnedValue::from(uid)),
    ]))
}

fn current_uids() -> Result<(u32, u32), String> {
    let status = fs::read_to_string("/proc/self/status")
        .map_err(|error| format!("unable to inspect current process credentials: {error}"))?;
    let line = status
        .lines()
        .find(|line| line.starts_with("Uid:"))
        .ok_or_else(|| "current process credentials have no Uid field".to_owned())?;
    let values = line
        .split_whitespace()
        .skip(1)
        .take(2)
        .map(str::parse::<u32>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| "current process user id is invalid".to_owned())?;
    match values.as_slice() {
        [real, effective] => Ok((*real, *effective)),
        _ => Err("current process credentials are incomplete".to_owned()),
    }
}

fn current_process_start_time() -> Result<u64, String> {
    let stat = fs::read_to_string("/proc/self/stat")
        .map_err(|error| format!("unable to inspect current process start time: {error}"))?;
    let close = stat
        .rfind(')')
        .ok_or_else(|| "current process stat has no command terminator".to_owned())?;
    let fields = stat[close + 1..].split_whitespace().collect::<Vec<_>>();
    fields
        .get(19)
        .ok_or_else(|| "current process stat has no start time".to_owned())?
        .parse::<u64>()
        .map_err(|_| "current process start time is invalid".to_owned())
}

impl LinuxSecretServiceStore {
    fn ensure_unlocked(&self) -> Result<(), String> {
        let service = connect_secret_service()?;
        let collection = service
            .get_default_collection()
            .map_err(|error| format!("unable to open desktop default keyring: {error}"))?;
        if collection
            .is_locked()
            .map_err(|error| format!("unable to query desktop keyring state: {error}"))?
        {
            return Err(
                "desktop default keyring is locked; unlock it in the desktop session and retry"
                    .to_owned(),
            );
        }
        Ok(())
    }

    fn has_any_enrollment(&self) -> Result<bool, String> {
        let service = connect_secret_service()?;
        let collection = service
            .get_default_collection()
            .map_err(|error| format!("unable to open desktop default keyring: {error}"))?;
        let items = collection
            .search_items(base_attributes())
            .map_err(|error| format!("unable to query desktop keyring: {error}"))?;
        Ok(!items.is_empty())
    }
}

impl DeviceSecretStore for LinuxSecretServiceStore {
    fn state(&self, slot: &SystemAuthSlot) -> Result<DeviceUnlockState, String> {
        let service = match connect_secret_service() {
            Ok(service) => service,
            Err(_) => return Ok(DeviceUnlockState::Unavailable),
        };
        let collection = match service.get_default_collection() {
            Ok(collection) => collection,
            Err(_) => return Ok(DeviceUnlockState::Unavailable),
        };
        let slot_id = slot.storage_id();
        let items = collection
            .search_items(attributes(&slot_id))
            .map_err(|error| format!("unable to query desktop keyring: {error}"))?;
        if items.is_empty() {
            return Ok(DeviceUnlockState::Disabled);
        }
        if collection
            .is_locked()
            .map_err(|error| format!("unable to query desktop keyring state: {error}"))?
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
        self.ensure_unlocked()?;
        let service = connect_secret_service()?;
        let collection = service
            .get_default_collection()
            .map_err(|error| format!("unable to open desktop default keyring: {error}"))?;
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

    fn read(&self, slot: &SystemAuthSlot) -> Result<DeviceSecretRead, String> {
        let service = connect_secret_service()?;
        let collection = service
            .get_default_collection()
            .map_err(|error| format!("unable to open desktop default keyring: {error}"))?;
        if collection
            .is_locked()
            .map_err(|error| format!("unable to query desktop keyring state: {error}"))?
        {
            return Err("desktop default keyring became locked after authentication".to_owned());
        }
        let slot_id = slot.storage_id();
        let mut items = collection
            .search_items(attributes(&slot_id))
            .map_err(|error| format!("unable to query desktop keyring: {error}"))?;
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
        self.ensure_unlocked()?;
        let service = connect_secret_service()?;
        let collection = service
            .get_default_collection()
            .map_err(|error| format!("unable to open desktop default keyring: {error}"))?;
        let slot_id = slot.storage_id();
        for item in collection
            .search_items(attributes(&slot_id))
            .map_err(|error| format!("unable to query desktop keyring: {error}"))?
        {
            item.delete()
                .map_err(|error| format!("unable to remove device unlock key: {error}"))?;
        }
        Ok(())
    }
}

fn connect_secret_service() -> Result<SecretService<'static>, String> {
    SecretService::connect(EncryptionType::Dh)
        .map_err(|error| format!("desktop secret service is unavailable: {error}"))
}

fn base_attributes() -> HashMap<&'static str, &'static str> {
    HashMap::from([("application", "fresnica"), ("purpose", "device-unlock")])
}

fn attributes(slot_id: &str) -> HashMap<&str, &str> {
    let mut attributes = base_attributes();
    attributes.insert("slot", slot_id);
    attributes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_is_scoped_to_linux_user_without_keep_authorization() {
        let uid = 1000;
        let policy = render_polkit_policy(uid);
        assert!(policy.contains("com.fresnica.device-unlock.authenticate.1000"));
        assert!(policy.contains("<allow_active>auth_self</allow_active>"));
        assert!(!policy.contains("auth_self_keep"));
        assert_eq!(
            polkit_policy_path(uid),
            Path::new(POLKIT_POLICY_DIR).join("com.fresnica.device-unlock.1000.policy")
        );
    }

    #[test]
    fn interactive_polkit_results_preserve_success_cancel_and_failure() {
        assert_eq!(
            classify_interactive_polkit_result(PolkitAuthorization {
                authorized: true,
                challenge: false,
                dismissed: false,
            }),
            PolkitAuthenticationOutcome::Verified
        );
        assert_eq!(
            classify_interactive_polkit_result(PolkitAuthorization {
                authorized: false,
                challenge: false,
                dismissed: true,
            }),
            PolkitAuthenticationOutcome::Cancelled
        );
        assert!(matches!(
            classify_interactive_polkit_result(PolkitAuthorization {
                authorized: false,
                challenge: false,
                dismissed: false,
            }),
            PolkitAuthenticationOutcome::Unavailable(_)
        ));
    }

    #[test]
    fn process_start_time_is_available_for_polkit_subject() {
        assert!(current_process_start_time().unwrap() > 0);
    }
}
