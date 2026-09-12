mod anchor_auth;
mod asset_discovery;
mod contacts;
mod contract;
mod contract_alias;
mod device_unlock;
#[cfg(target_os = "linux")]
mod device_unlock_linux;
#[cfg(target_os = "macos")]
mod device_unlock_macos;
#[cfg(target_os = "windows")]
mod device_unlock_windows;
mod dex;
mod diagnostics;
mod friendbot;
mod ledger;
mod plugin;
mod plugin_host;
mod read_commands;
mod send;
mod transaction_flow;
mod trust;
mod wallet;

use std::env;
use std::path::{Path, PathBuf};
use std::process;

use fresnica_client::{FresnicaClient, NetworkProfile, WalletStorage};
use zeroize::Zeroizing;

const HELP: &str = r#"Fresnica CLI

Usage:
  fresnica [GLOBAL OPTIONS] <COMMAND> [ARGS]

Commands:
  info       Show local wallet information
  account    Show current ledger account state
  balance    Show account balances and liabilities
  history    Show recent account operations
  asset      Discover issued assets
  send       Send a payment
  trust      Manage issued-asset trustlines
  dex        Read and trade on the Stellar DEX
  contract   Use Soroban contracts
  wallet     Manage wallets and signing material
  contact    Manage contacts
  plugin     Manage CLI plugins
  anchor     Stellar Anchor SEP flows (bundled plugin)

Global options:
  --home PATH                   Override Fresnica home for this invocation
  --network mainnet|testnet     Select Stellar network
  -v, --verbose                Show safe execution stages and failure context
  -vv                          Also show CLI version, network, and pinned Fresnica source
  --horizon-url URL            Override the Horizon endpoint for this invocation
  --rpc-url URL                Override the Stellar RPC endpoint for this invocation
  --tx-timeout SECONDS         Override the Classic transaction validity window
  -h, --help                   Show this help
  -V, --version                Show version information

Environment:
  FRESNICA_HOME                Default Fresnica home; CLI flag wins
  FRESNICA_HORIZON_URL         Default Horizon endpoint override; CLI flag wins
  FRESNICA_RPC_URL             Default Stellar RPC endpoint override; CLI flag wins
  FRESNICA_TX_TIMEOUT_SECONDS  Default Classic transaction validity window; CLI flag wins

More help:
  fresnica wallet --help       Wallet and signer commands
  fresnica anchor --help       Anchor plugin commands
"#;

fn main() {
    let arguments: Vec<String> = env::args().skip(1).collect();
    #[cfg(target_os = "linux")]
    if let Some(result) = device_unlock_linux::internal_policy_command(&arguments) {
        if let Err(error) = result {
            eprintln!("Error: {error}");
            process::exit(2);
        }
        return;
    }
    diagnostics::set_verbosity(diagnostics::leading_verbosity(&arguments));
    let global = match GlobalOptions::parse(&arguments) {
        Ok(global) => global,
        Err(error) => {
            diagnostics::render_error(&error);
            process::exit(2);
        }
    };
    diagnostics::set_verbosity(global.verbosity);
    diagnostics::startup(&global.network);
    if let Err(error) = run(global) {
        diagnostics::render_error(&error);
        process::exit(2);
    }
}

fn run(global: GlobalOptions) -> Result<(), String> {
    if global.command.is_empty() {
        print!("{HELP}");
        return Ok(());
    }
    if global.command == ["--help"] || global.command == ["-h"] {
        print!("{HELP}");
        return Ok(());
    }
    if global.command == ["--version"] || global.command == ["-V"] {
        println!(
            "fresnica {} · Fresnica source {}",
            env!("CARGO_PKG_VERSION"),
            diagnostics::short_fresnica_revision()
        );
        return Ok(());
    }

    diagnostics::stage(command_stage(&global.command));
    let plugin_context = plugin::NativeHostContext {
        home: &global.home,
        network: &global.network,
        horizon_url: global.horizon_url.as_deref(),
        rpc_url: global.rpc_url.as_deref(),
        tx_timeout_seconds: global.tx_timeout_seconds,
    };
    match global.command[0].as_str() {
        "info" | "contact" | "wallet" => run_local_command(&global),
        "plugin" => plugin::command_plugin(&global.command[1..]),
        "account" | "balance" | "assets" | "history" | "asset" | "send" | "trust" | "dex"
        | "contract" | "__plugin-host" => run_network_command(&global),
        other => match plugin::dispatch(&global.command, &plugin_context)? {
            Some(exit_code) => process::exit(exit_code),
            None => Err(format!("unknown command: {other}\n\n{HELP}")),
        },
    }
}

fn run_local_command(global: &GlobalOptions) -> Result<(), String> {
    diagnostics::stage("initialize local wallet storage");
    let storage = WalletStorage::new(&global.home)?;
    match global.command[0].as_str() {
        "info" => wallet::command_info(&storage, &global.command[1..]),
        "contact" => contacts::command_contact(&storage, &global.command[1..]),
        "wallet" => wallet::command_wallet(&storage, &global.network, &global.command[1..]),
        _ => unreachable!("local command was classified before dispatch"),
    }
}

fn run_network_command(global: &GlobalOptions) -> Result<(), String> {
    diagnostics::stage("initialize Fresnica network client");
    let mut profile = NetworkProfile::for_network(&global.network)?;
    if let Some(horizon_url) = horizon_url_override(global.horizon_url.as_deref()) {
        profile = profile.with_horizon_url(&horizon_url)?;
    }
    if let Some(rpc_url) = rpc_url_override(global.rpc_url.as_deref()) {
        profile = profile.with_rpc_url(&rpc_url)?;
    }
    let mut client = FresnicaClient::from_profile(&global.home, profile)?;
    if let Some(timeout_seconds) = tx_timeout_override(global.tx_timeout_seconds)? {
        client = client.with_classic_transaction_timeout_seconds(timeout_seconds)?;
    }
    match global.command[0].as_str() {
        "account" => read_commands::command_account(&client, &global.command[1..]),
        "balance" | "assets" => read_commands::command_balance(&client, &global.command[1..]),
        "history" => read_commands::command_history(&client, &global.command[1..]),
        "asset" => asset_discovery::command_asset(&client, &global.command[1..]),
        "send" => send::command_send(&client, &global.command[1..]),
        "trust" => trust::command_trust(&client, &global.command[1..]),
        "dex" => dex::command_dex(&client, &global.command[1..]),
        "contract" => contract::command_contract(&client, &global.command[1..]),
        "__plugin-host" => plugin_host::command(&client, &global.network, &global.command[1..]),
        _ => unreachable!("network command was classified before dispatch"),
    }
}

struct GlobalOptions {
    home: PathBuf,
    network: String,
    horizon_url: Option<String>,
    rpc_url: Option<String>,
    tx_timeout_seconds: Option<u64>,
    verbosity: u8,
    command: Vec<String>,
}

impl GlobalOptions {
    fn parse(arguments: &[String]) -> Result<Self, String> {
        let mut home = None;
        let mut network = "mainnet".to_owned();
        let mut horizon_url = None;
        let mut rpc_url = None;
        let mut tx_timeout_seconds = None;
        let mut verbosity = 0u8;
        let mut index = 0;
        while index < arguments.len() {
            match arguments[index].as_str() {
                "-v" | "--verbose" => {
                    verbosity = (verbosity + 1).min(2);
                    index += 1;
                }
                "-vv" => {
                    verbosity = 2;
                    index += 1;
                }
                "--home" => {
                    index += 1;
                    let value = arguments
                        .get(index)
                        .ok_or_else(|| "--home requires a path".to_owned())?;
                    home = Some(expand_path(value)?);
                    index += 1;
                }
                "--network" => {
                    index += 1;
                    network = arguments
                        .get(index)
                        .ok_or_else(|| "--network requires mainnet or testnet".to_owned())?
                        .to_owned();
                    validate_network(&network)?;
                    index += 1;
                }
                "--horizon-url" => {
                    index += 1;
                    horizon_url = Some(
                        arguments
                            .get(index)
                            .ok_or_else(|| "--horizon-url requires a URL".to_owned())?
                            .to_owned(),
                    );
                    index += 1;
                }
                "--rpc-url" => {
                    index += 1;
                    rpc_url = Some(
                        arguments
                            .get(index)
                            .ok_or_else(|| "--rpc-url requires a URL".to_owned())?
                            .to_owned(),
                    );
                    index += 1;
                }
                "--tx-timeout" => {
                    index += 1;
                    let value = arguments
                        .get(index)
                        .ok_or_else(|| "--tx-timeout requires seconds".to_owned())?;
                    tx_timeout_seconds = Some(parse_tx_timeout(value)?);
                    index += 1;
                }
                _ => break,
            }
        }
        let home = match home {
            Some(home) => home,
            None => default_home()?,
        };
        Ok(Self {
            home,
            network,
            horizon_url,
            rpc_url,
            tx_timeout_seconds,
            verbosity,
            command: arguments[index..].to_vec(),
        })
    }
}

fn horizon_url_override(cli_value: Option<&str>) -> Option<String> {
    cli_value
        .map(str::to_owned)
        .or_else(|| env::var("FRESNICA_HORIZON_URL").ok())
}

fn rpc_url_override(cli_value: Option<&str>) -> Option<String> {
    cli_value
        .map(str::to_owned)
        .or_else(|| env::var("FRESNICA_RPC_URL").ok())
}

fn parse_tx_timeout(value: &str) -> Result<u64, String> {
    value
        .parse::<u64>()
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| {
            "Classic transaction timeout must be a positive integer number of seconds".to_owned()
        })
}

fn tx_timeout_override(cli_value: Option<u64>) -> Result<Option<u64>, String> {
    match cli_value {
        Some(value) => Ok(Some(value)),
        None => env::var("FRESNICA_TX_TIMEOUT_SECONDS")
            .ok()
            .map(|value| parse_tx_timeout(&value))
            .transpose(),
    }
}

fn command_stage(command: &[String]) -> &'static str {
    match command.first().map(String::as_str) {
        Some("info") => "CLI command: info",
        Some("account") => "CLI command: account",
        Some("balance" | "assets") => "CLI command: balance",
        Some("history") => "CLI command: history",
        Some("asset") => "CLI command: asset",
        Some("send") => "CLI command: send",
        Some("contact") => "CLI command: contact",
        Some("trust") => "CLI command: trust",
        Some("dex") => "CLI command: dex",
        Some("contract") => "CLI command: contract",
        Some("anchor") => "CLI command: anchor",
        Some("wallet") => "CLI command: wallet",
        Some("plugin") => "CLI command: plugin",
        _ => "CLI command dispatch",
    }
}

fn prompt_hidden(prompt: &str) -> Result<Zeroizing<String>, String> {
    rpassword::prompt_password(prompt)
        .map(Zeroizing::new)
        .map_err(|error| format!("unable to read secret input: {error}"))
}

fn validate_network(network: &str) -> Result<(), String> {
    if matches!(network, "mainnet" | "testnet") {
        Ok(())
    } else {
        Err(format!("unknown network: {network}"))
    }
}

fn default_home() -> Result<PathBuf, String> {
    if let Some(home) = env::var_os("FRESNICA_HOME") {
        let home = home.to_string_lossy();
        return expand_path(&home);
    }
    let base = env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .ok_or_else(|| "unable to determine home directory; set FRESNICA_HOME".to_owned())?;
    Ok(PathBuf::from(base).join(".fresnica"))
}

fn expand_path(value: &str) -> Result<PathBuf, String> {
    if value == "~" {
        return env::var_os("HOME")
            .or_else(|| env::var_os("USERPROFILE"))
            .map(PathBuf::from)
            .ok_or_else(|| "unable to expand ~; set HOME or USERPROFILE".to_owned());
    }
    if let Some(rest) = value.strip_prefix("~/") {
        let home = env::var_os("HOME")
            .or_else(|| env::var_os("USERPROFILE"))
            .ok_or_else(|| "unable to expand ~; set HOME or USERPROFILE".to_owned())?;
        return Ok(PathBuf::from(home).join(rest));
    }
    Ok(Path::new(value).to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transaction_timeout_parser_rejects_non_numeric_values() {
        assert_eq!(parse_tx_timeout("900").unwrap(), 900);
        assert!(parse_tx_timeout("0").is_err());
        assert!(parse_tx_timeout("five-minutes").is_err());
    }

    #[test]
    fn top_level_help_delegates_anchor_details_to_plugin() {
        assert!(HELP.contains("anchor     Stellar Anchor SEP flows (bundled plugin)"));
        assert!(HELP.contains("fresnica anchor --help"));
        assert!(!HELP.contains("anchor deposit"));
        assert!(!HELP.contains("anchor withdraw"));
        assert!(!HELP.contains("anchor customer"));
    }

    #[test]
    fn parses_verbose_global_options() {
        let args = [
            "-v",
            "--network",
            "testnet",
            "--horizon-url",
            "https://stellar.example/horizon",
            "--rpc-url",
            "https://stellar.example/rpc",
            "--tx-timeout",
            "900",
            "--verbose",
            "account",
        ]
        .map(str::to_owned);
        let global = GlobalOptions::parse(&args).unwrap();
        assert_eq!(global.verbosity, 2);
        assert_eq!(global.network, "testnet");
        assert_eq!(
            global.horizon_url.as_deref(),
            Some("https://stellar.example/horizon")
        );
        assert_eq!(
            global.rpc_url.as_deref(),
            Some("https://stellar.example/rpc")
        );
        assert_eq!(global.tx_timeout_seconds, Some(900));
        assert_eq!(global.command, ["account"]);
    }
}
