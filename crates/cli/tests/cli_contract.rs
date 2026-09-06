use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

const WATCH_ADDRESS: &str = "GDLVVGABQKYQVN6VJP7NHSLEA45A5YLS6PNKMIZFV4BBU2HXA5IRVHUR";

fn temp_home() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "fresnica-cli-contract-{}-{nonce}",
        std::process::id()
    ))
}

fn run(home: &PathBuf, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_fresnica"))
        .arg("--home")
        .arg(home)
        .args(arguments)
        .output()
        .unwrap()
}

#[test]
fn info_prints_sdk_core_compatibility_line_once() {
    let home = temp_home();
    let import = run(
        &home,
        &[
            "--network",
            "testnet",
            "wallet",
            "import-watch",
            "observer",
            WATCH_ADDRESS,
        ],
    );
    assert!(
        import.status.success(),
        "import-watch failed: {}",
        String::from_utf8_lossy(&import.stderr)
    );

    let info = run(
        &home,
        &["--network", "testnet", "info", "--wallet", "observer"],
    );
    assert!(
        info.status.success(),
        "info failed: {}",
        String::from_utf8_lossy(&info.stderr)
    );
    let stdout = String::from_utf8(info.stdout).unwrap();
    assert_eq!(stdout.matches("SDK/Core:   Rust (direct link)").count(), 1);

    let _ = fs::remove_dir_all(home);
}
