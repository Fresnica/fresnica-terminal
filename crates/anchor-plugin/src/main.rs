use std::collections::BTreeMap;
use std::env;
use std::io::Read;
use std::process::{self, Command, Stdio};

use fresnica_client::{discover_anchor_at, start_anchor_sep24_transfer, AnchorTransferKind};
use serde::Deserialize;
use serde_json::json;
use zeroize::Zeroizing;

const HELP: &str = "usage:\n  fresnica anchor discover CODE:GISSUER --home-domain DOMAIN [--json]\n  fresnica anchor auth CODE:GISSUER --home-domain DOMAIN [--wallet NAME] [--json]\n  fresnica anchor deposit CODE:GISSUER --home-domain DOMAIN [--wallet NAME] [--field NAME=VALUE]... [--json]\n  fresnica anchor withdraw CODE:GISSUER --home-domain DOMAIN [--wallet NAME] [--field NAME=VALUE]... [--json]";

fn main() {
    if let Err(error) = run() {
        eprintln!("Error: {error}");
        process::exit(2);
    }
}

fn run() -> Result<(), String> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    let command = args
        .first()
        .map(String::as_str)
        .ok_or_else(|| HELP.to_owned())?;
    match command {
        "discover" => command_discover(&args[1..]),
        "auth" => command_auth(&args[1..]),
        "deposit" => command_transfer(&args[1..], AnchorTransferKind::Deposit),
        "withdraw" => command_transfer(&args[1..], AnchorTransferKind::Withdraw),
        _ => Err(HELP.to_owned()),
    }
}

fn command_discover(args: &[String]) -> Result<(), String> {
    let options = Options::parse(args, false)?;
    let discovery = discover_anchor_at(&options.asset, &options.home_domain)?;
    if options.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "asset": discovery.asset.display(),
                "network": plugin_network()?,
                "home_domain": discovery.home_domain,
                "capabilities": discovery.capabilities,
            }))
            .map_err(|error| format!("unable to encode anchor discovery: {error}"))?
        );
    } else {
        println!(
            "Anchor · {} [{}]",
            discovery.asset.display(),
            plugin_network()?
        );
        println!("Domain: {}", discovery.home_domain);
        println!(
            "SEP-24 deposit={} withdraw={}",
            discovery.capabilities.sep24_deposit, discovery.capabilities.sep24_withdraw
        );
    }
    Ok(())
}

fn command_auth(args: &[String]) -> Result<(), String> {
    let options = Options::parse(args, true)?;
    let auth = host_anchor_auth(&options)?;
    if options.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "network": auth.network,
                "wallet": auth.wallet,
                "address": auth.address,
                "home_domain": auth.home_domain,
                "authenticated": true,
            }))
            .map_err(|error| format!("unable to encode auth result: {error}"))?
        );
    } else {
        println!("Authenticated · {} [{}]", auth.home_domain, auth.network);
        println!("Wallet: {}", auth.wallet);
        println!("Address: {}", auth.address);
    }
    Ok(())
}

fn command_transfer(args: &[String], kind: AnchorTransferKind) -> Result<(), String> {
    let options = Options::parse(args, true)?;
    let discovery = discover_anchor_at(&options.asset, &options.home_domain)?;
    let auth = host_anchor_auth(&options)?;
    let result = start_anchor_sep24_transfer(
        &auth.address,
        &discovery.asset,
        &discovery.capabilities,
        kind,
        &options.fields,
        auth.token.as_str(),
    )?;
    if options.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "asset": discovery.asset.display(),
                "network": auth.network,
                "wallet": auth.wallet,
                "address": auth.address,
                "home_domain": discovery.home_domain,
                "kind": kind,
                "protocol": "sep24",
                "url": result.url,
                "id": result.transaction_id,
            }))
            .map_err(|error| format!("unable to encode SEP-24 result: {error}"))?
        );
    } else {
        println!("Anchor · {} [{}]", discovery.asset.display(), auth.network);
        println!("Action: {} via SEP-24", kind.endpoint());
        println!("Open URL: {}", result.url);
        println!("Transfer ID: {}", result.transaction_id);
    }
    Ok(())
}

#[derive(Debug)]
struct Options {
    asset: String,
    home_domain: String,
    wallet: Option<String>,
    fields: BTreeMap<String, String>,
    json: bool,
}

impl Options {
    fn parse(args: &[String], allow_wallet_and_fields: bool) -> Result<Self, String> {
        let asset = args.first().ok_or_else(|| HELP.to_owned())?.to_owned();
        let mut home_domain = None;
        let mut wallet = None;
        let mut fields = BTreeMap::new();
        let mut json = false;
        let mut index = 1;
        while index < args.len() {
            match args[index].as_str() {
                "--home-domain" => {
                    index += 1;
                    home_domain = Some(
                        args.get(index)
                            .ok_or_else(|| "--home-domain requires a domain".to_owned())?
                            .to_owned(),
                    );
                    index += 1;
                }
                "--wallet" if allow_wallet_and_fields => {
                    index += 1;
                    wallet = Some(
                        args.get(index)
                            .ok_or_else(|| "--wallet requires a wallet name".to_owned())?
                            .to_owned(),
                    );
                    index += 1;
                }
                "--field" if allow_wallet_and_fields => {
                    index += 1;
                    let raw = args
                        .get(index)
                        .ok_or_else(|| "--field requires NAME=VALUE".to_owned())?;
                    let (name, value) = raw
                        .split_once('=')
                        .ok_or_else(|| "--field requires NAME=VALUE".to_owned())?;
                    if name.is_empty()
                        || value.is_empty()
                        || matches!(name, "asset_code" | "asset_issuer" | "account")
                    {
                        return Err("invalid anchor field".to_owned());
                    }
                    fields.insert(name.to_owned(), value.to_owned());
                    index += 1;
                }
                "--json" => {
                    json = true;
                    index += 1;
                }
                _ => return Err(HELP.to_owned()),
            }
        }
        Ok(Self {
            asset,
            home_domain: home_domain
                .ok_or_else(|| "--home-domain is required for the plugin spike".to_owned())?,
            wallet,
            fields,
            json,
        })
    }
}

#[derive(Debug)]
struct HostAuth {
    network: String,
    wallet: String,
    address: String,
    home_domain: String,
    token: Zeroizing<String>,
}

#[derive(Debug, Deserialize)]
struct HostAuthWire {
    schema: String,
    network: String,
    wallet: String,
    address: String,
    home_domain: String,
    token: String,
}

fn host_anchor_auth(options: &Options) -> Result<HostAuth, String> {
    let host = env::var_os("FRESNICA_PLUGIN_HOST")
        .ok_or_else(|| "FRESNICA_PLUGIN_HOST is missing".to_owned())?;
    let mut command = Command::new(host);
    command
        .arg("--network")
        .arg(plugin_network()?)
        .arg("__plugin-host")
        .arg("anchor-auth")
        .arg(&options.asset)
        .arg("--home-domain")
        .arg(&options.home_domain)
        .stdin(Stdio::inherit())
        .stderr(Stdio::inherit())
        .stdout(Stdio::piped());
    if let Some(wallet) = &options.wallet {
        command.arg("--wallet").arg(wallet);
    }
    let mut child = command
        .spawn()
        .map_err(|error| format!("unable to call Fresnica plugin host: {error}"))?;
    let mut output = Zeroizing::new(String::new());
    child
        .stdout
        .take()
        .ok_or_else(|| "plugin host stdout is unavailable".to_owned())?
        .read_to_string(&mut output)
        .map_err(|error| format!("unable to read plugin host response: {error}"))?;
    let status = child
        .wait()
        .map_err(|error| format!("unable to wait for plugin host: {error}"))?;
    if !status.success() {
        return Err(format!("Fresnica plugin host failed with status {status}"));
    }
    let wire: HostAuthWire = serde_json::from_str(output.trim())
        .map_err(|error| format!("invalid plugin host response: {error}"))?;
    if wire.schema != "fresnica-plugin-anchor-auth-v1" {
        return Err("unsupported Fresnica plugin host response".to_owned());
    }
    Ok(HostAuth {
        network: wire.network,
        wallet: wire.wallet,
        address: wire.address,
        home_domain: wire.home_domain,
        token: Zeroizing::new(wire.token),
    })
}

fn plugin_network() -> Result<String, String> {
    env::var("FRESNICA_PLUGIN_NETWORK").map_err(|_| "FRESNICA_PLUGIN_NETWORK is missing".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ASSET: &str = "SRT:GCDNJUBQSX7AJWLJACMJ7I4BC3Z47BQUTMHEICZLE6MU4KQBRYG5JY6B";

    #[test]
    fn discovery_requires_explicit_home_domain_in_spike() {
        let error = Options::parse(&[ASSET.to_owned(), "--json".to_owned()], false).unwrap_err();
        assert_eq!(error, "--home-domain is required for the plugin spike");
    }

    #[test]
    fn transfer_parses_wallet_and_custom_fields() {
        let options = Options::parse(
            &[
                ASSET.to_owned(),
                "--home-domain".to_owned(),
                "testanchor.stellar.org".to_owned(),
                "--wallet".to_owned(),
                "treasury".to_owned(),
                "--field".to_owned(),
                "type=bank_account".to_owned(),
                "--json".to_owned(),
            ],
            true,
        )
        .unwrap();

        assert_eq!(options.home_domain, "testanchor.stellar.org");
        assert_eq!(options.wallet.as_deref(), Some("treasury"));
        assert_eq!(
            options.fields.get("type").map(String::as_str),
            Some("bank_account")
        );
        assert!(options.json);
    }

    #[test]
    fn transfer_rejects_host_managed_fields() {
        for field in ["account=GABC", "asset_code=SRT", "asset_issuer=GABC"] {
            let error = Options::parse(
                &[
                    ASSET.to_owned(),
                    "--home-domain".to_owned(),
                    "testanchor.stellar.org".to_owned(),
                    "--field".to_owned(),
                    field.to_owned(),
                ],
                true,
            )
            .unwrap_err();
            assert_eq!(error, "invalid anchor field");
        }
    }
}
