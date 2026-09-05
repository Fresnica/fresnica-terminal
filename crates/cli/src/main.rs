mod anchor;
mod contacts;
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

use fresnica_client::FresnicaClient;
use zeroize::Zeroizing;

const HELP: &str = r#"Fresnica native Rust CLI

Usage:
  fresnica [--home PATH] [--network mainnet|testnet] info [--wallet NAME]
  fresnica [--home PATH] [--network mainnet|testnet] account [--wallet NAME] [--json]
  fresnica [--home PATH] [--network mainnet|testnet] balance [--wallet NAME] [--json]
  fresnica [--home PATH] [--network mainnet|testnet] history [--wallet NAME] [--limit N] [--json]
  fresnica [--home PATH] [--network mainnet|testnet] send AMOUNT ASSET to DESTINATION [--wallet NAME] [--memo TEXT] [-y]
  fresnica [--home PATH] contact COMMAND ...
  fresnica [--home PATH] [--network mainnet|testnet] trust add CODE:GISSUER [--limit VALUE] [--wallet NAME] [-y]
  fresnica [--home PATH] [--network mainnet|testnet] trust limit CODE:GISSUER LIMIT [--wallet NAME] [-y]
  fresnica [--home PATH] [--network mainnet|testnet] trust remove CODE:GISSUER [--wallet NAME] [-y]
  fresnica [--home PATH] [--network mainnet|testnet] dex orderbook SELLING BUYING [--json]
  fresnica [--home PATH] [--network mainnet|testnet] dex offers [--wallet NAME] [--limit N] [--json]
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

Network commands:
  account                       Show current Horizon account state
  balance                       Show current account balances and liabilities
  history                       Show newest Horizon operations (default 20, max 200)
  send                          Review, sign through Fresnica SDK/Core, and submit a payment
  trust                         Add, change, or remove an issued-asset trustline
  dex                           Read and trade on the Stellar DEX
  anchor                        Discover anchor capabilities and start SEP-24/SEP-6 transfers

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

    diagnostics::stage("initialize Fresnica client");
    let client = FresnicaClient::new(&global.home, &global.network)?;
    let storage = client.storage();
    diagnostics::stage(command_stage(&global.command));
    match global.command[0].as_str() {
        "info" => wallet::command_info(storage, &global.command[1..]),
        "account" => read_commands::command_account(&client, &global.command[1..]),
        "balance" | "assets" => read_commands::command_balance(&client, &global.command[1..]),
        "history" => read_commands::command_history(&client, &global.command[1..]),
        "send" => send::command_send(&client, &global.command[1..]),
        "contact" => contacts::command_contact(storage, &global.command[1..]),
        "trust" => trust::command_trust(&client, &global.command[1..]),
        "dex" => dex::command_dex(&client, &global.command[1..]),
        "anchor" => anchor::command_anchor(&client, &global.command[1..]),
        "wallet" => wallet::command_wallet(storage, &global.network, &global.command[1..]),
        other => Err(format!("unknown command: {other}\n\n{HELP}")),
    }
}

struct GlobalOptions {
    home: PathBuf,
    network: String,
    verbosity: u8,
    command: Vec<String>,
}

impl GlobalOptions {
    fn parse(arguments: &[String]) -> Result<Self, String> {
        let mut home = None;
        let mut network = "mainnet".to_owned();
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
            verbosity,
            command: arguments[index..].to_vec(),
        })
    }
}

fn command_stage(command: &[String]) -> &'static str {
    match command.first().map(String::as_str) {
        Some("info") => "CLI command: info",
        Some("account") => "CLI command: account",
        Some("balance" | "assets") => "CLI command: balance",
        Some("history") => "CLI command: history",
        Some("send") => "CLI command: send",
        Some("contact") => "CLI command: contact",
        Some("trust") => "CLI command: trust",
        Some("dex") => "CLI command: dex",
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
        let args = ["-v", "--network", "testnet", "--verbose", "account"].map(str::to_owned);
        let global = GlobalOptions::parse(&args).unwrap();
        assert_eq!(global.verbosity, 2);
        assert_eq!(global.network, "testnet");
        assert_eq!(global.command, ["account"]);
    }
}
