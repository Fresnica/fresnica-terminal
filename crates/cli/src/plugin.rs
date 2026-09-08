use std::collections::BTreeSet;
use std::env;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const PREFIXES: [&str; 3] = ["fresnica-", "stellar-", "soroban-"];
const RESERVED_NATIVE_EXECUTABLES: [&str; 1] = ["fresnica-tui"];

pub struct NativeHostContext<'a> {
    pub home: &'a Path,
    pub network: &'a str,
    pub horizon_url: Option<&'a str>,
    pub rpc_url: Option<&'a str>,
    pub tx_timeout_seconds: Option<u64>,
}

pub fn command_plugin(args: &[String]) -> Result<(), String> {
    match args {
        [command] if command == "ls" => {
            let plugins = list_plugins(env::var_os("PATH").as_deref());
            if plugins.is_empty() {
                println!("No Fresnica or Stellar CLI plugins found on PATH.");
                println!(
                    "Plugins are executable commands named fresnica-<name> or stellar-<name>."
                );
            } else {
                println!("Installed external CLI plugins:");
                for plugin in plugins {
                    println!("  {plugin}");
                }
            }
            Ok(())
        }
        [command] => Err(format!(
            "unknown plugin command: {command}; expected: plugin ls"
        )),
        _ => Err("usage: fresnica plugin ls".to_owned()),
    }
}

pub fn dispatch(args: &[String], context: &NativeHostContext<'_>) -> Result<Option<i32>, String> {
    let Some(invocation) = find_plugin(args, env::var_os("PATH").as_deref()) else {
        return Ok(None);
    };
    run_invocation(invocation, context).map(Some)
}

fn run_invocation(
    invocation: PluginInvocation,
    context: &NativeHostContext<'_>,
) -> Result<i32, String> {
    let mut command = Command::new(&invocation.executable);
    command.args(&invocation.args);
    if invocation.native {
        let host = env::current_exe().map_err(|error| {
            format!("unable to locate Fresnica plugin host executable: {error}")
        })?;
        command
            .env("FRESNICA_PLUGIN_API", "1")
            .env("FRESNICA_PLUGIN_HOST", host)
            .env("FRESNICA_PLUGIN_NETWORK", context.network)
            .env("FRESNICA_HOME", context.home);
        if let Some(url) = context.horizon_url {
            command.env("FRESNICA_HORIZON_URL", url);
        }
        if let Some(url) = context.rpc_url {
            command.env("FRESNICA_RPC_URL", url);
        }
        if let Some(timeout_seconds) = context.tx_timeout_seconds {
            command.env("FRESNICA_TX_TIMEOUT_SECONDS", timeout_seconds.to_string());
        }
    }
    let status = command.status().map_err(|error| {
        format!(
            "unable to run external CLI plugin {}: {error}",
            invocation.executable.display()
        )
    })?;
    Ok(status.code().unwrap_or(1))
}

#[derive(Debug, PartialEq, Eq)]
struct PluginInvocation {
    executable: PathBuf,
    args: Vec<String>,
    native: bool,
}

fn find_plugin(args: &[String], path: Option<&OsStr>) -> Option<PluginInvocation> {
    find_plugin_with_prefixes(args, path, &PREFIXES)
}

fn find_plugin_with_prefixes(
    args: &[String],
    path: Option<&OsStr>,
    prefixes: &[&str],
) -> Option<PluginInvocation> {
    let command_len = args
        .iter()
        .take_while(|argument| !argument.starts_with("--"))
        .count();

    for len in (1..=command_len).rev() {
        let command_name = args[..len].join("-");
        for prefix in prefixes {
            let executable_name = format!("{prefix}{command_name}");
            if RESERVED_NATIVE_EXECUTABLES.contains(&executable_name.as_str()) {
                continue;
            }
            if let Some(executable) = find_executable(&executable_name, path) {
                return Some(PluginInvocation {
                    executable,
                    args: args[len..].to_vec(),
                    native: *prefix == "fresnica-",
                });
            }
        }
    }

    None
}

fn list_plugins(path: Option<&OsStr>) -> Vec<String> {
    let mut plugins = BTreeSet::new();
    let Some(path) = path else {
        return Vec::new();
    };

    for directory in env::split_paths(path) {
        let Ok(entries) = fs::read_dir(directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !is_executable(&path) {
                continue;
            }
            let Some(file_name) = path.file_name().and_then(OsStr::to_str) else {
                continue;
            };
            let Some(file_name) = plugin_file_name(file_name) else {
                continue;
            };
            if RESERVED_NATIVE_EXECUTABLES.contains(&file_name) {
                continue;
            }
            for prefix in PREFIXES {
                if let Some(name) = file_name.strip_prefix(prefix) {
                    if !name.is_empty() {
                        plugins.insert(name.to_owned());
                    }
                    break;
                }
            }
        }
    }

    plugins.into_iter().collect()
}

fn find_executable(name: &str, path: Option<&OsStr>) -> Option<PathBuf> {
    let path = path?;
    for directory in env::split_paths(path) {
        for file_name in executable_names(name) {
            let candidate = directory.join(file_name);
            if is_executable(&candidate) {
                return Some(candidate);
            }
        }
    }
    None
}

#[cfg(not(windows))]
fn executable_names(name: &str) -> Vec<OsString> {
    vec![OsString::from(name)]
}

#[cfg(windows)]
fn executable_names(name: &str) -> Vec<OsString> {
    vec![OsString::from(format!("{name}.exe"))]
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    fs::metadata(path)
        .map(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(windows)]
fn is_executable(path: &Path) -> bool {
    fs::metadata(path)
        .map(|metadata| metadata.is_file())
        .unwrap_or(false)
}

#[cfg(not(any(unix, windows)))]
fn is_executable(path: &Path) -> bool {
    fs::metadata(path)
        .map(|metadata| metadata.is_file())
        .unwrap_or(false)
}

#[cfg(windows)]
fn plugin_file_name(file_name: &str) -> Option<&str> {
    strip_windows_executable_suffix(file_name)
}

#[cfg(not(windows))]
fn plugin_file_name(file_name: &str) -> Option<&str> {
    Some(file_name)
}

#[cfg(any(windows, test))]
fn strip_windows_executable_suffix(file_name: &str) -> Option<&str> {
    let extension = ".exe";
    if file_name.len() > extension.len()
        && file_name[file_name.len() - extension.len()..].eq_ignore_ascii_case(extension)
    {
        return Some(&file_name[..file_name.len() - extension.len()]);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_lookup_prefers_longest_command_chain() {
        let root = temporary_directory("longest");
        create_test_plugin(&root, "stellar-saint-account");
        create_test_plugin(&root, "stellar-saint");
        let path = env::join_paths([&root]).unwrap();
        let args = ["saint", "account", "GABC", "--json"].map(str::to_owned);

        let invocation = find_plugin(&args, Some(&path)).unwrap();

        assert_eq!(
            invocation.executable.file_name().and_then(OsStr::to_str),
            Some(test_executable_name("stellar-saint-account").as_str())
        );
        assert_eq!(invocation.args, ["GABC", "--json"]);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn plugin_lookup_falls_back_to_shorter_command_chain() {
        let root = temporary_directory("shorter");
        create_test_plugin(&root, "stellar-saint");
        let path = env::join_paths([&root]).unwrap();
        let args = ["saint", "account", "GABC", "--json"].map(str::to_owned);

        let invocation = find_plugin(&args, Some(&path)).unwrap();

        assert_eq!(
            invocation.executable.file_name().and_then(OsStr::to_str),
            Some(test_executable_name("stellar-saint").as_str())
        );
        assert_eq!(invocation.args, ["account", "GABC", "--json"]);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn plugin_lookup_prefers_longer_chain_before_namespace_priority() {
        let root = temporary_directory("longer-before-prefix");
        create_test_plugin(&root, "fresnica-aqua");
        create_test_plugin(&root, "stellar-aqua-contract");
        let path = env::join_paths([&root]).unwrap();
        let args = ["aqua", "contract", "quote"].map(str::to_owned);

        let invocation = find_plugin(&args, Some(&path)).unwrap();

        assert_eq!(
            invocation.executable.file_name().and_then(OsStr::to_str),
            Some(test_executable_name("stellar-aqua-contract").as_str())
        );
        assert_eq!(invocation.args, ["quote"]);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn plugin_lookup_prefers_fresnica_then_stellar_then_legacy_soroban() {
        let root = temporary_directory("prefix");
        create_test_plugin(&root, "fresnica-hello");
        create_test_plugin(&root, "stellar-hello");
        create_test_plugin(&root, "soroban-hello");
        let path = env::join_paths([&root]).unwrap();
        let args = ["hello", "world"].map(str::to_owned);

        let invocation = find_plugin(&args, Some(&path)).unwrap();

        assert_eq!(
            invocation.executable.file_name().and_then(OsStr::to_str),
            Some(test_executable_name("fresnica-hello").as_str())
        );
        assert_eq!(invocation.args, ["world"]);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn plugin_lookup_prefers_stellar_over_legacy_soroban_without_fresnica_plugin() {
        let root = temporary_directory("stellar-prefix");
        create_test_plugin(&root, "stellar-hello");
        create_test_plugin(&root, "soroban-hello");
        let path = env::join_paths([&root]).unwrap();
        let args = ["hello", "world"].map(str::to_owned);

        let invocation = find_plugin(&args, Some(&path)).unwrap();

        assert_eq!(
            invocation.executable.file_name().and_then(OsStr::to_str),
            Some(test_executable_name("stellar-hello").as_str())
        );
        assert_eq!(invocation.args, ["world"]);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn plugin_lookup_supports_legacy_soroban_prefix() {
        let root = temporary_directory("legacy");
        create_test_plugin(&root, "soroban-hello");
        let path = env::join_paths([&root]).unwrap();
        let args = ["hello", "world"].map(str::to_owned);

        let invocation = find_plugin(&args, Some(&path)).unwrap();

        assert_eq!(
            invocation.executable.file_name().and_then(OsStr::to_str),
            Some(test_executable_name("soroban-hello").as_str())
        );
        assert_eq!(invocation.args, ["world"]);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn companion_tui_binary_is_not_a_plugin() {
        let root = temporary_directory("companion-tui");
        create_test_plugin(&root, "fresnica-anchor");
        create_test_plugin(&root, "fresnica-tui");
        let path = env::join_paths([&root]).unwrap();

        assert_eq!(list_plugins(Some(&path)), vec!["anchor"]);
        let args = ["tui"].map(str::to_owned);
        assert!(find_plugin(&args, Some(&path)).is_none());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn windows_plugin_listing_accepts_only_exe_names() {
        assert_eq!(
            strip_windows_executable_suffix("stellar-alpha.exe"),
            Some("stellar-alpha")
        );
        assert_eq!(
            strip_windows_executable_suffix("stellar-alpha.EXE"),
            Some("stellar-alpha")
        );
        assert_eq!(strip_windows_executable_suffix("stellar-alpha"), None);
        assert_eq!(strip_windows_executable_suffix("stellar-alpha.cmd"), None);
    }

    #[test]
    fn plugin_list_deduplicates_all_supported_prefixes() {
        let root = temporary_directory("list");
        create_test_plugin(&root, "fresnica-alpha");
        create_test_plugin(&root, "stellar-alpha");
        create_test_plugin(&root, "soroban-alpha");
        create_test_plugin(&root, "fresnica-beta");
        let path = env::join_paths([&root]).unwrap();

        assert_eq!(list_plugins(Some(&path)), ["alpha", "beta"]);
        fs::remove_dir_all(root).unwrap();
    }

    fn temporary_directory(label: &str) -> PathBuf {
        let path = env::temp_dir().join(format!(
            "fresnica-plugin-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[cfg(unix)]
    fn create_test_plugin(directory: &Path, name: &str) {
        use std::os::unix::fs::PermissionsExt;

        let path = directory.join(name);
        fs::write(&path, "#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    }

    #[cfg(windows)]
    fn create_test_plugin(directory: &Path, name: &str) {
        fs::write(directory.join(format!("{name}.exe")), b"test").unwrap();
    }

    fn test_executable_name(name: &str) -> String {
        if cfg!(windows) {
            format!("{name}.exe")
        } else {
            name.to_owned()
        }
    }
}
