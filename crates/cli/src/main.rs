mod activity;
mod anchor;
mod asset_discovery;
mod contacts;
mod contract;
mod dex;
mod diagnostics;
mod friendbot;
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

const HELP: &str = r#"Fresnica native Rust CLI

Usage:
  fresnica [--home PATH] [--network mainnet|testnet] info [--wallet NAME]
  fresnica [--home PATH] [--network mainnet|testnet] account [--wallet NAME] [--json]
  fresnica [--home PATH] [--network mainnet|testnet] balance [--wallet NAME] [--json]
  fresnica [--home PATH] [--network mainnet|testnet] history [--wallet NAME] [--limit N] [--json]
  fresnica [--home PATH] [--network mainnet|testnet] activity [--wallet NAME] [--limit N] [--cursor TOKEN] [--json]
  fresnica [--home PATH] [--network mainnet|testnet] asset discover [--limit N] [--cached] [--json]
  fresnica [--home PATH] [--network mainnet|testnet] send AMOUNT ASSET to DESTINATION [--wallet NAME] [--memo TEXT] [-y]
  fresnica [--home PATH] contact COMMAND ...
  fresnica [--home PATH] [--network mainnet|testnet] trust add CODE:GISSUER [--limit VALUE] [--wallet NAME] [-y]
  fresnica [--home PATH] [--network mainnet|testnet] trust limit CODE:GISSUER LIMIT [--wallet NAME] [-y]
  fresnica [--home PATH] [--network mainnet|testnet] trust remove CODE:GISSUER [--wallet NAME] [-y]
  fresnica [--home PATH] [--network mainnet|testnet] dex orderbook SELLING BUYING [--json]
  fresnica [--home PATH] [--network mainnet|testnet] dex offers [--wallet NAME] [--limit N] [--json]
  fresnica [--home PATH] [--network mainnet|testnet] contract invoke C... [--wallet NAME] [-y] [--json] -- FUNCTION [--NAME VALUE]...
  fresnica [--network mainnet|testnet] anchor discover CODE:GISSUER [--json]
  fresnica [--home PATH] [--network mainnet|testnet] anchor auth CODE:GISSUER [--wallet NAME]
  fresnica [--home PATH] [--network mainnet|testnet] anchor deposit CODE:GISSUER [--wallet NAME] [--field NAME=VALUE]... [--json]
  fresnica [--home PATH] [--network mainnet|testnet] anchor withdraw CODE:GISSUER [--wallet NAME] [--field NAME=VALUE]... [--json]
  fresnica [--home PATH] [--network mainnet|testnet] anchor status CODE:GISSUER ID [--wallet NAME] [--protocol sep24|sep6] [--pay] [-y] [--json]
  fresnica [--home PATH] [--network mainnet|testnet] anchor customer CODE:GISSUER [--wallet NAME] [--id CUSTOMER_ID] [--transaction ID] [--type TYPE] [--lang LANG] [--input PATH|-] [--json]
  fresnica [--home PATH] [--network mainnet|testnet] wallet COMMAND ...

Global options:
  -v, --verbose                Show safe execution stages and failure context
  -vv                          Also show CLI version, network, and pinned Fresnica source
  --horizon-url URL            Override the Horizon endpoint for this invocation
  --rpc-url URL                Override the Stellar RPC endpoint for this invocation

Environment:
  FRESNICA_HORIZON_URL         Default Horizon endpoint override; CLI flag wins
  FRESNICA_RPC_URL             Default Stellar RPC endpoint override; CLI flag wins

Network commands:
  account                       Show current ledger account state
  balance                       Show current account balances and liabilities
  history                       Show newest account operations (default 20, max 200)
  activity                      Show complete transactions with stable cursor paging
  asset                         Discover exact issued-asset identities and optional metadata
  send                          Review, sign through Fresnica SDK/Core, and submit a payment
  trust                         Add, change, or remove an issued-asset trustline
  dex                           Read and trade on the Stellar DEX
  contract                      Invoke deployed contracts through their on-chain interface
  anchor                        Discover anchor capabilities and start SEP-24/SEP-6 transfers

Contract invocation:
  Fresnica options come before `--`; the function and named arguments after `--`
  are resolved from the deployed contract specification.

Contact commands:
  list
  add NAME G... [--memo TEXT]
  remove NAME

Wallet commands:
  list
  use NAME
  create NAME [--index N] [--language LANGUAGE] [--strength BITS]
  import-secret NAME
  import-mnemonic NAME [--index N] [--language LANGUAGE]
  import-watch NAME G...
  attach-secret NAME             Add matching S... signing material to watch-only wallet
  attach-mnemonic NAME [--index N] [--language LANGUAGE]
  detach-signer NAME             Remove local signing material and keep the G address
  testnet-fund [--wallet NAME]   Fund a testnet wallet with Friendbot
  fund [--wallet NAME]           Alias for testnet-fund
  reveal [NAME]
  backup NAME PATH [--force]
  restore PATH [--name NAME]
  delete NAME

The native client uses the platform-neutral Fresnica SDK for wallet protection and
signing, while low-level Stellar/XDR primitives remain in Rust Core. It uses the
same wallet files and version-1 encrypted backup format as the Python reference
client. All local software wallets share one Fresnica passphrase while retaining
independent Core salt/nonce-derived encryption keys.
"#;

fn main() {
    let arguments: Vec<String> = env::args().skip(1).collect();
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
    match global.command[0].as_str() {
        "info" | "contact" | "wallet" => run_local_command(&global),
        "account" | "balance" | "assets" | "history" | "activity" | "asset" | "send" | "trust"
        | "dex" | "contract" | "anchor" => run_network_command(&global),
        other => Err(format!("unknown command: {other}\n\n{HELP}")),
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
    let client = FresnicaClient::from_profile(&global.home, profile)?;
    match global.command[0].as_str() {
        "account" => read_commands::command_account(&client, &global.command[1..]),
        "balance" | "assets" => read_commands::command_balance(&client, &global.command[1..]),
        "history" => read_commands::command_history(&client, &global.command[1..]),
        "activity" => activity::command_activity(&client, &global.command[1..]),
        "asset" => asset_discovery::command_asset(&client, &global.command[1..]),
        "send" => send::command_send(&client, &global.command[1..]),
        "trust" => trust::command_trust(&client, &global.command[1..]),
        "dex" => dex::command_dex(&client, &global.command[1..]),
        "contract" => contract::command_contract(&client, &global.command[1..]),
        "anchor" => anchor::command_anchor(&client, &global.command[1..]),
        _ => unreachable!("network command was classified before dispatch"),
    }
}

struct GlobalOptions {
    home: PathBuf,
    network: String,
    horizon_url: Option<String>,
    rpc_url: Option<String>,
    verbosity: u8,
    command: Vec<String>,
}

impl GlobalOptions {
    fn parse(arguments: &[String]) -> Result<Self, String> {
        let mut home = None;
        let mut network = "mainnet".to_owned();
        let mut horizon_url = None;
        let mut rpc_url = None;
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

fn command_stage(command: &[String]) -> &'static str {
    match command.first().map(String::as_str) {
        Some("info") => "CLI command: info",
        Some("account") => "CLI command: account",
        Some("balance" | "assets") => "CLI command: balance",
        Some("history") => "CLI command: history",
        Some("activity") => "CLI command: activity",
        Some("asset") => "CLI command: asset",
        Some("send") => "CLI command: send",
        Some("contact") => "CLI command: contact",
        Some("trust") => "CLI command: trust",
        Some("dex") => "CLI command: dex",
        Some("contract") => "CLI command: contract",
        Some("anchor") => "CLI command: anchor",
        Some("wallet") => "CLI command: wallet",
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
    fn parses_verbose_global_options() {
        let args = [
            "-v",
            "--network",
            "testnet",
            "--horizon-url",
            "https://stellar.example/horizon",
            "--rpc-url",
            "https://stellar.example/rpc",
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
        assert_eq!(global.command, ["account"]);
    }
}
