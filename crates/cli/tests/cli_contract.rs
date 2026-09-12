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

fn command(home: &PathBuf, arguments: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_fresnica"));
    command
        .arg("--home")
        .arg(home)
        .args(arguments)
        .env_remove("FRESNICA_HORIZON_URL");
    command
}

fn run(home: &PathBuf, arguments: &[&str]) -> Output {
    command(home, arguments).output().unwrap()
}

fn run_with_horizon_url(home: &PathBuf, arguments: &[&str], horizon_url: &str) -> Output {
    command(home, arguments)
        .env("FRESNICA_HORIZON_URL", horizon_url)
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

#[test]
fn local_commands_do_not_require_a_valid_horizon_endpoint() {
    let home = temp_home();
    let invalid_horizon = "not-a-url";

    let import = run_with_horizon_url(
        &home,
        &[
            "--network",
            "testnet",
            "wallet",
            "import-watch",
            "observer",
            WATCH_ADDRESS,
        ],
        invalid_horizon,
    );
    assert!(
        import.status.success(),
        "local wallet command should ignore Horizon config: {}",
        String::from_utf8_lossy(&import.stderr)
    );

    let info = run_with_horizon_url(
        &home,
        &["--network", "testnet", "info", "--wallet", "observer"],
        invalid_horizon,
    );
    assert!(
        info.status.success(),
        "local info command should ignore Horizon config: {}",
        String::from_utf8_lossy(&info.stderr)
    );

    let contacts = run_with_horizon_url(&home, &["contact", "list"], invalid_horizon);
    assert!(
        contacts.status.success(),
        "local contact command should ignore Horizon config: {}",
        String::from_utf8_lossy(&contacts.stderr)
    );

    let contracts = command(
        &home,
        &["--network", "testnet", "contract", "list", "--json"],
    )
    .env("FRESNICA_HORIZON_URL", invalid_horizon)
    .env("FRESNICA_RPC_URL", "not-a-url")
    .output()
    .unwrap();
    assert!(
        contracts.status.success(),
        "local contract store command should ignore provider config: {}",
        String::from_utf8_lossy(&contracts.stderr)
    );

    let account = run_with_horizon_url(
        &home,
        &["--network", "testnet", "account", "--wallet", "observer"],
        invalid_horizon,
    );
    assert!(!account.status.success());
    assert!(String::from_utf8_lossy(&account.stderr)
        .contains("Horizon URL must start with http:// or https://"));

    let _ = fs::remove_dir_all(home);
}

#[test]
fn contract_store_commands_are_versioned_and_machine_readable() {
    const CONTRACT: &str = "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAD2KM";
    let home = temp_home();

    let add = run(
        &home,
        &[
            "--network",
            "testnet",
            "contract",
            "add",
            "aqua",
            CONTRACT,
            "--json",
        ],
    );
    assert!(
        add.status.success(),
        "contract add failed: {}",
        String::from_utf8_lossy(&add.stderr)
    );
    let added: serde_json::Value = serde_json::from_slice(&add.stdout).unwrap();
    assert_eq!(added["kind"], "contract_saved");
    assert_eq!(added["contract"]["name"], "aqua");
    assert_eq!(added["contract"]["contract_id"], CONTRACT);

    let stored: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(home.join("contracts-testnet.json")).unwrap())
            .unwrap();
    assert_eq!(stored["schema"], "fresnica-contract-store-v1");
    assert_eq!(stored["contracts"][0]["contract_id"], CONTRACT);

    let list = run(
        &home,
        &["--network", "testnet", "contract", "list", "--json"],
    );
    assert!(list.status.success());
    let listed: serde_json::Value = serde_json::from_slice(&list.stdout).unwrap();
    assert_eq!(listed["schema"], "fresnica-contract-list-v1");
    assert_eq!(listed["network"], "testnet");
    assert_eq!(listed["contracts"][0]["name"], "aqua");
    assert_ne!(listed["schema"], stored["schema"]);

    let remove = run(
        &home,
        &[
            "--network",
            "testnet",
            "contract",
            "remove",
            "aqua",
            "--json",
        ],
    );
    assert!(remove.status.success());
    let removed: serde_json::Value = serde_json::from_slice(&remove.stdout).unwrap();
    assert_eq!(removed["kind"], "contract_removed");
    assert_eq!(removed["contract"]["contract_id"], CONTRACT);

    let _ = fs::remove_dir_all(home);
}
