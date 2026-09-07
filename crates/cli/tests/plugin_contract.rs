#[cfg(unix)]
mod unix {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Output};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir() -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "fresnica-cli-plugin-{}-{nonce}",
            std::process::id()
        ))
    }

    fn write_plugin(directory: &Path, name: &str, body: &str) {
        fs::create_dir_all(directory).unwrap();
        let path = directory.join(name);
        fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        let mut permissions = fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).unwrap();
    }

    fn run(directory: &Path, arguments: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_fresnica"))
            .args(arguments)
            .env("PATH", directory)
            .output()
            .unwrap()
    }

    #[test]
    fn fresnica_namespace_wins_and_plugin_exit_code_is_preserved() {
        let directory = temp_dir();
        write_plugin(
            &directory,
            "fresnica-hello",
            "printf 'fresnica:%s\\n' \"$*\"; printf 'plugin-stderr\\n' >&2; exit 23",
        );
        write_plugin(
            &directory,
            "stellar-hello",
            "printf 'stellar:%s\\n' \"$*\"; exit 24",
        );

        let output = run(&directory, &["hello", "alpha", "--flag", "beta"]);
        assert_eq!(output.status.code(), Some(23));
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            "fresnica:alpha --flag beta\n"
        );
        assert_eq!(String::from_utf8(output.stderr).unwrap(), "plugin-stderr\n");

        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn longest_command_chain_wins_before_shorter_plugins() {
        let directory = temp_dir();
        write_plugin(&directory, "stellar-tools", "printf 'short:%s\\n' \"$*\"");
        write_plugin(
            &directory,
            "stellar-tools-echo",
            "printf 'long:%s\\n' \"$*\"",
        );

        let output = run(&directory, &["tools", "echo", "value"]);
        assert!(output.status.success());
        assert_eq!(String::from_utf8(output.stdout).unwrap(), "long:value\n");

        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn legacy_soroban_plugin_is_a_fallback() {
        let directory = temp_dir();
        write_plugin(&directory, "soroban-legacy", "printf 'legacy:%s\\n' \"$*\"");

        let output = run(&directory, &["legacy", "value"]);
        assert!(output.status.success());
        assert_eq!(String::from_utf8(output.stdout).unwrap(), "legacy:value\n");

        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn built_in_commands_cannot_be_shadowed_by_plugins() {
        let directory = temp_dir();
        write_plugin(
            &directory,
            "fresnica-wallet",
            "printf 'PLUGIN-SHOULD-NOT-RUN\\n'",
        );
        let home = directory.join("home");

        let output = Command::new(env!("CARGO_BIN_EXE_fresnica"))
            .arg("--home")
            .arg(&home)
            .args(["wallet", "list"])
            .env("PATH", &directory)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "wallet list failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!String::from_utf8_lossy(&output.stdout).contains("PLUGIN-SHOULD-NOT-RUN"));

        let _ = fs::remove_dir_all(directory);
    }
}
