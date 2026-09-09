#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("Fresnica Linux System Auth provider is Linux-only");
    std::process::exit(20);
}

#[cfg(target_os = "linux")]
fn main() {
    linux::run();
}

#[cfg(target_os = "linux")]
mod linux {
    use sha2::{Digest, Sha256};
    use std::fs::{self, File, OpenOptions};
    use std::io::{self, Read, Write};
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
    use std::path::{Path, PathBuf};
    use std::process::{Command, Stdio};
    use zeroize::Zeroizing;

    const EXIT_PASSPHRASE_REQUIRED: i32 = 10;
    const EXIT_CANCELLED: i32 = 11;
    const EXIT_FAILURE: i32 = 20;
    const ACTION_ID: &str = "com.fresnica.system-auth.release";
    const PKCHECK: &str = "/usr/bin/pkcheck";
    const PKACTION: &str = "/usr/bin/pkaction";
    const STORAGE_ROOT: &str = "/var/lib/fresnica-system-auth";
    const RECORD_MAGIC: &[u8; 4] = b"FSA1";
    const UNLOCK_KEY_LEN: usize = 32;
    const RECORD_LEN: u64 = (RECORD_MAGIC.len() + UNLOCK_KEY_LEN) as u64;

    #[derive(Debug, PartialEq, Eq)]
    enum AuthOutcome {
        Authorized,
        PassphraseRequired,
        Cancelled,
        Failed,
    }

    #[derive(Debug, PartialEq, Eq)]
    struct Caller {
        pid: u32,
        start_time: u64,
        uid: u32,
    }

    struct Storage {
        root: PathBuf,
        owner_uid: u32,
        owner_gid: u32,
    }

    impl Storage {
        fn production() -> Self {
            Self {
                root: PathBuf::from(STORAGE_ROOT),
                owner_uid: 0,
                owner_gid: 0,
            }
        }

        fn user_dir(&self, uid: u32) -> PathBuf {
            self.root.join(uid.to_string())
        }

        fn record_path(&self, uid: u32, slot: &str) -> PathBuf {
            self.user_dir(uid)
                .join(hex(&Sha256::digest(slot.as_bytes())))
        }

        fn ensure_dirs(&self, uid: u32) -> Result<(), String> {
            self.ensure_dir(&self.root)?;
            self.ensure_dir(&self.user_dir(uid))
        }

        fn ensure_dir(&self, path: &Path) -> Result<(), String> {
            match fs::symlink_metadata(path) {
                Ok(metadata) => self.validate_dir(path, &metadata),
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    fs::create_dir(path).map_err(|error| {
                        format!(
                            "unable to create protected storage {}: {error}",
                            path.display()
                        )
                    })?;
                    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(
                        |error| {
                            format!(
                                "unable to protect storage permissions {}: {error}",
                                path.display()
                            )
                        },
                    )?;
                    let metadata = fs::symlink_metadata(path).map_err(|error| {
                        format!(
                            "unable to inspect protected storage {}: {error}",
                            path.display()
                        )
                    })?;
                    self.validate_dir(path, &metadata)
                }
                Err(error) => Err(format!(
                    "unable to inspect protected storage {}: {error}",
                    path.display()
                )),
            }
        }

        fn validate_dir(&self, path: &Path, metadata: &fs::Metadata) -> Result<(), String> {
            if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
                return Err(format!(
                    "protected storage is not a directory: {}",
                    path.display()
                ));
            }
            if metadata.uid() != self.owner_uid || metadata.gid() != self.owner_gid {
                return Err(format!(
                    "protected storage has unexpected ownership: {}",
                    path.display()
                ));
            }
            if metadata.mode() & 0o777 != 0o700 {
                return Err(format!(
                    "protected storage has unsafe permissions: {}",
                    path.display()
                ));
            }
            Ok(())
        }

        fn validate_record(&self, uid: u32, slot: &str) -> Result<bool, String> {
            let path = self.record_path(uid, slot);
            let metadata = match fs::symlink_metadata(&path) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
                Err(error) => {
                    return Err(format!("unable to inspect System Auth enrollment: {error}"))
                }
            };
            if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
                return Err("system-auth enrollment storage is not a regular file".to_owned());
            }
            if metadata.uid() != self.owner_uid || metadata.gid() != self.owner_gid {
                return Err("system-auth enrollment storage has unexpected ownership".to_owned());
            }
            if metadata.mode() & 0o777 != 0o600 || metadata.len() != RECORD_LEN {
                return Err("system-auth enrollment storage is invalid".to_owned());
            }
            let mut file = File::open(&path)
                .map_err(|error| format!("unable to open System Auth enrollment: {error}"))?;
            let mut magic = [0u8; RECORD_MAGIC.len()];
            file.read_exact(&mut magic)
                .map_err(|error| format!("unable to read System Auth enrollment: {error}"))?;
            if &magic != RECORD_MAGIC {
                return Err("system-auth enrollment storage has an invalid format".to_owned());
            }
            Ok(true)
        }

        fn write_key(&self, uid: u32, slot: &str, key: &[u8]) -> Result<(), String> {
            if key.len() != UNLOCK_KEY_LEN {
                return Err("system-auth enrollment requires exactly 32 key bytes".to_owned());
            }
            self.ensure_dirs(uid)?;
            let path = self.record_path(uid, slot);
            let temp = path.with_extension(format!("{}.tmp", std::process::id()));
            let _ = fs::remove_file(&temp);
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&temp)
                .map_err(|error| format!("unable to create System Auth enrollment: {error}"))?;
            let write_result = (|| -> Result<(), String> {
                file.write_all(RECORD_MAGIC)
                    .and_then(|_| file.write_all(key))
                    .and_then(|_| file.sync_all())
                    .map_err(|error| {
                        format!("unable to persist System Auth enrollment: {error}")
                    })?;
                fs::rename(&temp, &path)
                    .map_err(|error| format!("unable to commit System Auth enrollment: {error}"))?;
                File::open(self.user_dir(uid))
                    .and_then(|directory| directory.sync_all())
                    .map_err(|error| format!("unable to sync System Auth enrollment: {error}"))?;
                Ok(())
            })();
            if write_result.is_err() {
                let _ = fs::remove_file(&temp);
            }
            write_result
        }

        fn read_key(&self, uid: u32, slot: &str) -> Result<Zeroizing<Vec<u8>>, String> {
            if !self.validate_record(uid, slot)? {
                return Err("system-auth enrollment is missing".to_owned());
            }
            let mut file = File::open(self.record_path(uid, slot))
                .map_err(|error| format!("unable to open System Auth enrollment: {error}"))?;
            let mut magic = [0u8; RECORD_MAGIC.len()];
            file.read_exact(&mut magic)
                .map_err(|error| format!("unable to read System Auth enrollment: {error}"))?;
            let mut key = Zeroizing::new(vec![0u8; UNLOCK_KEY_LEN]);
            file.read_exact(key.as_mut_slice())
                .map_err(|error| format!("unable to read System Auth unlock key: {error}"))?;
            Ok(key)
        }

        fn delete(&self, uid: u32, slot: &str) -> Result<(), String> {
            let path = self.record_path(uid, slot);
            let metadata = match fs::symlink_metadata(&path) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
                Err(error) => {
                    return Err(format!("unable to inspect System Auth enrollment: {error}"))
                }
            };
            if !metadata.file_type().is_file()
                || metadata.file_type().is_symlink()
                || metadata.uid() != self.owner_uid
                || metadata.gid() != self.owner_gid
            {
                return Err("refusing to remove unsafe System Auth enrollment storage".to_owned());
            }
            fs::remove_file(path)
                .map_err(|error| format!("unable to remove System Auth enrollment: {error}"))
        }
    }

    pub(super) fn run() {
        let arguments: Vec<String> = std::env::args().collect();
        let command = arguments.get(1).map(String::as_str).unwrap_or("");
        let caller = match caller() {
            Ok(caller) => caller,
            Err(error) => {
                if command == "probe" {
                    finish(EXIT_PASSPHRASE_REQUIRED, None);
                }
                finish(EXIT_FAILURE, Some(&error));
            }
        };
        let storage = Storage::production();

        match command {
            "probe" if arguments.len() == 2 => {
                if policy_available() {
                    finish(0, None)
                } else {
                    finish(EXIT_PASSPHRASE_REQUIRED, None)
                }
            }
            "has" => {
                let slot = require_slot(&arguments);
                match storage.validate_record(caller.uid, slot) {
                    Ok(true) => finish(0, None),
                    Ok(false) => finish(EXIT_PASSPHRASE_REQUIRED, None),
                    Err(error) => finish(EXIT_FAILURE, Some(&error)),
                }
            }
            "enroll" => {
                let slot = require_slot(&arguments);
                let key = match read_unlock_key() {
                    Ok(key) => key,
                    Err(error) => finish(EXIT_FAILURE, Some(&error)),
                };
                match storage.write_key(caller.uid, slot, key.as_slice()) {
                    Ok(()) => finish(0, None),
                    Err(error) => finish(EXIT_FAILURE, Some(&error)),
                }
            }
            "release" => {
                let slot = require_slot(&arguments);
                match storage.validate_record(caller.uid, slot) {
                    Ok(true) => {}
                    Ok(false) => finish(EXIT_PASSPHRASE_REQUIRED, None),
                    Err(error) => finish(EXIT_FAILURE, Some(&error)),
                }
                match authorize(&caller) {
                    AuthOutcome::Authorized => {}
                    AuthOutcome::PassphraseRequired => finish(EXIT_PASSPHRASE_REQUIRED, None),
                    AuthOutcome::Cancelled => finish(EXIT_CANCELLED, None),
                    AuthOutcome::Failed => {
                        finish(EXIT_FAILURE, Some("polkit authorization failed"))
                    }
                }
                let key = match storage.read_key(caller.uid, slot) {
                    Ok(key) => key,
                    Err(error) => finish(EXIT_FAILURE, Some(&error)),
                };
                if let Err(error) = io::stdout().write_all(key.as_slice()) {
                    finish(
                        EXIT_FAILURE,
                        Some(&format!("unable to return System Auth unlock key: {error}")),
                    );
                }
                finish(0, None)
            }
            "delete" => {
                let slot = require_slot(&arguments);
                match storage.delete(caller.uid, slot) {
                    Ok(()) => finish(0, None),
                    Err(error) => finish(EXIT_FAILURE, Some(&error)),
                }
            }
            _ => finish(
                EXIT_FAILURE,
                Some("usage: fresnica-system-auth-provider probe|has|enroll|release|delete [SLOT]"),
            ),
        }
    }

    fn require_slot(arguments: &[String]) -> &str {
        if arguments.len() != 3 || !valid_slot(&arguments[2]) {
            finish(
                EXIT_FAILURE,
                Some("system-auth provider requires an exact valid slot id"),
            );
        }
        &arguments[2]
    }

    fn valid_slot(slot: &str) -> bool {
        let Some((public_key, fingerprint)) = slot.split_once(':') else {
            return false;
        };
        public_key.len() == 56
            && public_key.starts_with('G')
            && public_key
                .bytes()
                .all(|byte| byte.is_ascii_uppercase() || (b'2'..=b'7').contains(&byte))
            && fingerprint.len() == 64
            && fingerprint
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    }

    fn read_unlock_key() -> Result<Zeroizing<Vec<u8>>, String> {
        let mut key = Zeroizing::new(Vec::with_capacity(UNLOCK_KEY_LEN + 1));
        io::stdin()
            .take((UNLOCK_KEY_LEN + 1) as u64)
            .read_to_end(&mut key)
            .map_err(|error| format!("unable to read System Auth enrollment key: {error}"))?;
        if key.len() != UNLOCK_KEY_LEN {
            return Err("system-auth enrollment requires exactly 32 key bytes".to_owned());
        }
        Ok(key)
    }

    fn caller() -> Result<Caller, String> {
        let status = fs::read_to_string("/proc/self/status")
            .map_err(|error| format!("unable to inspect provider credentials: {error}"))?;
        let (real_uid, effective_uid) = parse_uids(&status)?;
        if effective_uid != 0 {
            return Err("Linux System Auth provider is not installed setuid-root".to_owned());
        }
        if real_uid == 0 {
            return Err("Linux System Auth is unavailable for the root account".to_owned());
        }
        let pid = parse_ppid(&status)?;
        let stat = fs::read_to_string(format!("/proc/{pid}/stat"))
            .map_err(|error| format!("unable to inspect caller process: {error}"))?;
        let start_time = parse_start_time(&stat)?;
        Ok(Caller {
            pid,
            start_time,
            uid: real_uid,
        })
    }

    fn parse_uids(status: &str) -> Result<(u32, u32), String> {
        let line = status
            .lines()
            .find(|line| line.starts_with("Uid:"))
            .ok_or_else(|| "provider credential record has no Uid field".to_owned())?;
        let values: Vec<&str> = line.split_whitespace().skip(1).collect();
        let real = values
            .first()
            .ok_or_else(|| "provider credential record has no real uid".to_owned())?
            .parse::<u32>()
            .map_err(|_| "provider real uid is invalid".to_owned())?;
        let effective = values
            .get(1)
            .ok_or_else(|| "provider credential record has no effective uid".to_owned())?
            .parse::<u32>()
            .map_err(|_| "provider effective uid is invalid".to_owned())?;
        Ok((real, effective))
    }

    fn parse_ppid(status: &str) -> Result<u32, String> {
        let line = status
            .lines()
            .find(|line| line.starts_with("PPid:"))
            .ok_or_else(|| "provider credential record has no PPid field".to_owned())?;
        let pid = line
            .split_whitespace()
            .nth(1)
            .ok_or_else(|| "provider parent pid is missing".to_owned())?
            .parse::<u32>()
            .map_err(|_| "provider parent pid is invalid".to_owned())?;
        if pid == 0 {
            return Err("provider parent pid is unavailable".to_owned());
        }
        Ok(pid)
    }

    fn parse_start_time(stat: &str) -> Result<u64, String> {
        let close = stat
            .rfind(')')
            .ok_or_else(|| "caller proc stat has no command terminator".to_owned())?;
        let fields: Vec<&str> = stat[close + 1..].split_whitespace().collect();
        fields
            .get(19)
            .ok_or_else(|| "caller proc stat has no start time".to_owned())?
            .parse::<u64>()
            .map_err(|_| "caller proc start time is invalid".to_owned())
    }

    fn policy_available() -> bool {
        Command::new(PKACTION)
            .env_clear()
            .args(["--action-id", ACTION_ID])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }

    fn authorize(caller: &Caller) -> AuthOutcome {
        let subject = format!("{},{},{}", caller.pid, caller.start_time, caller.uid);
        let output = match Command::new(PKCHECK)
            .env_clear()
            .args([
                "--action-id",
                ACTION_ID,
                "--process",
                &subject,
                "--allow-user-interaction",
            ])
            .stdin(Stdio::null())
            .output()
        {
            Ok(output) => output,
            Err(_) => return AuthOutcome::Failed,
        };
        map_pkcheck_exit(output.status.code())
    }

    fn map_pkcheck_exit(code: Option<i32>) -> AuthOutcome {
        match code {
            Some(0) => AuthOutcome::Authorized,
            Some(1 | 2) => AuthOutcome::PassphraseRequired,
            Some(3) => AuthOutcome::Cancelled,
            _ => AuthOutcome::Failed,
        }
    }

    fn hex(bytes: &[u8]) -> String {
        let mut output = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            use std::fmt::Write as _;
            let _ = write!(output, "{byte:02x}");
        }
        output
    }

    fn finish(code: i32, message: Option<&str>) -> ! {
        if let Some(message) = message {
            eprintln!("{message}");
        }
        std::process::exit(code)
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::sync::atomic::{AtomicU64, Ordering};

        static NEXT_ROOT: AtomicU64 = AtomicU64::new(0);

        fn test_storage() -> (Storage, PathBuf) {
            let root = std::env::temp_dir().join(format!(
                "fresnica-linux-system-auth-{}-{}",
                std::process::id(),
                NEXT_ROOT.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = fs::remove_dir_all(&root);
            fs::create_dir(&root).unwrap();
            fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
            let metadata = fs::metadata(&root).unwrap();
            let storage = Storage {
                root: root.clone(),
                owner_uid: metadata.uid(),
                owner_gid: metadata.gid(),
            };
            (storage, root)
        }

        fn slot() -> String {
            format!("G{}:{}", "A".repeat(55), "a".repeat(64))
        }

        #[test]
        fn slot_shape_is_strict_and_path_safe() {
            let valid = slot();
            assert!(valid_slot(&valid));
            assert!(!valid_slot("../../wallet"));
            assert!(!valid_slot(&valid.replace(':', "/")));
            assert!(!valid_slot(&valid.to_uppercase()));
        }

        #[test]
        fn storage_round_trip_is_uid_scoped_and_hashed() {
            let (storage, root) = test_storage();
            let key = [7u8; UNLOCK_KEY_LEN];
            let slot = slot();
            storage.write_key(1000, &slot, &key).unwrap();
            assert!(storage.validate_record(1000, &slot).unwrap());
            assert!(!storage.validate_record(1001, &slot).unwrap());
            assert_eq!(storage.read_key(1000, &slot).unwrap().as_slice(), key);
            let path = storage.record_path(1000, &slot);
            assert!(!path.to_string_lossy().contains(&slot));
            storage.delete(1000, &slot).unwrap();
            assert!(!storage.validate_record(1000, &slot).unwrap());
            fs::remove_dir_all(root).unwrap();
        }

        #[test]
        fn unsafe_record_permissions_fail_closed() {
            let (storage, root) = test_storage();
            let slot = slot();
            storage
                .write_key(1000, &slot, &[9u8; UNLOCK_KEY_LEN])
                .unwrap();
            let path = storage.record_path(1000, &slot);
            fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
            assert!(storage.validate_record(1000, &slot).is_err());
            storage.delete(1000, &slot).unwrap();
            fs::remove_dir_all(root).unwrap();
        }

        #[test]
        fn proc_parsing_uses_parent_start_time_and_real_uid() {
            let status = "Name:\tprovider\nUid:\t1000\t0\t0\t0\nPPid:\t4242\n";
            assert_eq!(parse_uids(status).unwrap(), (1000, 0));
            assert_eq!(parse_ppid(status).unwrap(), 4242);
            let stat = "4242 (Fresnica Worker) S 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 987654 20 21";
            assert_eq!(parse_start_time(stat).unwrap(), 987654);
        }

        #[test]
        fn polkit_outcomes_preserve_cancel_and_fallback() {
            assert_eq!(map_pkcheck_exit(Some(0)), AuthOutcome::Authorized);
            assert_eq!(map_pkcheck_exit(Some(1)), AuthOutcome::PassphraseRequired);
            assert_eq!(map_pkcheck_exit(Some(2)), AuthOutcome::PassphraseRequired);
            assert_eq!(map_pkcheck_exit(Some(3)), AuthOutcome::Cancelled);
            assert_eq!(map_pkcheck_exit(Some(127)), AuthOutcome::Failed);
            assert_eq!(map_pkcheck_exit(None), AuthOutcome::Failed);
        }
    }
}
