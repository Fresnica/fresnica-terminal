use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

const WATCH_ADDRESS: &str = "GDLVVGABQKYQVN6VJP7NHSLEA45A5YLS6PNKMIZFV4BBU2HXA5IRVHUR";
const OTHER_WATCH_ADDRESS: &str = "GAXUGZINCMWFE5WPBMF4H75RYIH522TEGLZHGI7QXRDNGLEUFZJ4RWNY";

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
fn capabilities_are_machine_readable_without_home_or_network_configuration() {
    let output = Command::new(env!("CARGO_BIN_EXE_fresnica"))
        .args(["--network", "testnet", "capabilities", "--json"])
        .env_remove("HOME")
        .env_remove("USERPROFILE")
        .env_remove("FRESNICA_HOME")
        .env("FRESNICA_HORIZON_URL", "not-a-url")
        .env("FRESNICA_RPC_URL", "not-a-url")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "capabilities must not initialize wallet or network state: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["schema"], "fresnica-capabilities-v1");
    assert!(value["fresnica_revision"].as_str().is_some());
    let operations = value["operations"].as_array().unwrap();
    assert!(operations
        .iter()
        .any(|item| item["id"] == "contract.invoke"));
    assert!(operations.iter().any(|item| item["id"] == "token.transfer"));
}

#[test]
fn local_identity_commands_are_machine_readable() {
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
    assert!(import.status.success());

    let wallets = run(&home, &["wallet", "list", "--json"]);
    assert!(
        wallets.status.success(),
        "{}",
        String::from_utf8_lossy(&wallets.stderr)
    );
    let wallets: serde_json::Value = serde_json::from_slice(&wallets.stdout).unwrap();
    assert_eq!(wallets["kind"], "wallet_list");
    assert_eq!(wallets["wallets"][0]["name"], "observer");
    assert_eq!(wallets["wallets"][0]["address"], WATCH_ADDRESS);
    assert_eq!(wallets["wallets"][0]["watch_only"], true);
    assert!(wallets["wallets"][0].get("secret").is_none());

    let info = run(&home, &["info", "--wallet", "observer", "--json"]);
    assert!(
        info.status.success(),
        "{}",
        String::from_utf8_lossy(&info.stderr)
    );
    let info: serde_json::Value = serde_json::from_slice(&info.stdout).unwrap();
    assert_eq!(info["kind"], "wallet_info");
    assert_eq!(info["wallet"]["name"], "observer");
    assert_eq!(info["wallet"]["protection"], "none");
    assert!(info["fresnica_revision"].as_str().is_some());

    let import_second = run(
        &home,
        &[
            "--network",
            "testnet",
            "wallet",
            "import-watch",
            "secondary",
            OTHER_WATCH_ADDRESS,
        ],
    );
    assert!(import_second.status.success());
    let use_wallet = run(&home, &["wallet", "use", "secondary", "--json"]);
    assert!(
        use_wallet.status.success(),
        "{}",
        String::from_utf8_lossy(&use_wallet.stderr)
    );
    let use_wallet: serde_json::Value = serde_json::from_slice(&use_wallet.stdout).unwrap();
    assert_eq!(use_wallet["kind"], "wallet_default_changed");
    assert_eq!(use_wallet["wallet"]["name"], "secondary");
    assert_eq!(use_wallet["wallet"]["default"], true);
    assert!(use_wallet["wallet"].get("secret").is_none());

    let wallets_after_use = run(&home, &["wallet", "list", "--json"]);
    let wallets_after_use: serde_json::Value =
        serde_json::from_slice(&wallets_after_use.stdout).unwrap();
    let secondary = wallets_after_use["wallets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|wallet| wallet["name"] == "secondary")
        .unwrap();
    assert_eq!(secondary["default"], true);

    let add = run(
        &home,
        &[
            "contact",
            "add",
            "alice",
            WATCH_ADDRESS,
            "--memo",
            "42",
            "--json",
        ],
    );
    assert!(
        add.status.success(),
        "{}",
        String::from_utf8_lossy(&add.stderr)
    );
    let add: serde_json::Value = serde_json::from_slice(&add.stdout).unwrap();
    assert_eq!(add["kind"], "contact_added");
    assert_eq!(add["contact"]["memo"], "42");

    let list = run(&home, &["contact", "list", "--json"]);
    assert!(list.status.success());
    let list: serde_json::Value = serde_json::from_slice(&list.stdout).unwrap();
    assert_eq!(list["contacts"][0]["name"], "alice");

    let remove = run(&home, &["contact", "remove", "alice", "--json"]);
    assert!(remove.status.success());
    let remove: serde_json::Value = serde_json::from_slice(&remove.stdout).unwrap();
    assert_eq!(remove["kind"], "contact_removed");
    assert_eq!(remove["contact"]["name"], "alice");

    let _ = fs::remove_dir_all(home);
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

    let mut stored_with_observation = stored.clone();
    stored_with_observation["contracts"][0]["observed"] = serde_json::json!({
        "executable": "wasm",
        "wasm_hash": "11".repeat(32),
        "wasm_meta": [
            {"key": "sep", "value": "41"},
            {"key": "home_domain", "value": "example.org"}
        ],
        "derived": {
            "sep41": {
                "interface_version": "0.5.1",
                "native_sac": false,
                "sep47_declared": true,
                "interface_compatible": true
            }
        }
    });
    fs::write(
        home.join("contracts-testnet.json"),
        serde_json::to_string_pretty(&stored_with_observation).unwrap(),
    )
    .unwrap();

    let list = run(
        &home,
        &["--network", "testnet", "contract", "list", "--json"],
    );
    assert!(list.status.success());
    let listed: serde_json::Value = serde_json::from_slice(&list.stdout).unwrap();
    assert_eq!(listed["schema"], "fresnica-contract-list-v1");
    assert_eq!(listed["network"], "testnet");
    assert_eq!(listed["contracts"][0]["name"], "aqua");
    assert_eq!(
        listed["contracts"][0]["observed"]["wasm_meta"][0]["key"],
        "sep"
    );
    assert_eq!(
        listed["contracts"][0]["observed"]["wasm_meta"][1]["value"],
        "example.org"
    );
    assert_eq!(
        listed["contracts"][0]["observed"]["derived"]["sep41"]["interface_version"],
        "0.5.1"
    );
    assert_eq!(
        listed["contracts"][0]["observed"]["derived"]["sep41"]["sep47_declared"],
        true
    );
    assert_eq!(
        listed["contracts"][0]["observed"]["derived"]["sep41"]["interface_compatible"],
        true
    );
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
