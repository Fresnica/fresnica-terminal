use std::collections::BTreeMap;
use std::env;
use std::io::Read;
use std::path::Path;
use std::process::{self, Command, Stdio};

use fresnica_client::{
    anchor_status_requires_sep10, anchor_transaction_text, anchor_transfer_requires_sep10,
    anchor_withdrawal_payment_from_transaction, discover_anchor_at, fetch_anchor_transaction,
    get_anchor_customer, put_anchor_customer, select_anchor_status_protocol,
    select_anchor_transfer_protocol, start_anchor_sep24_transfer, start_anchor_sep6_transfer,
    AnchorCustomerFile, AnchorCustomerQuery, AnchorCustomerSnapshot, AnchorCustomerUpdate,
    AnchorProtocol, AnchorTransferKind, PaymentMemo,
};
use serde::Deserialize;
use serde_json::{json, Value as JsonValue};
use zeroize::Zeroizing;

const HELP: &str = "usage:\n  fresnica anchor discover CODE:GISSUER --home-domain DOMAIN [--json]\n  fresnica anchor auth CODE:GISSUER --home-domain DOMAIN [--wallet NAME] [--json]\n  fresnica anchor deposit CODE:GISSUER --home-domain DOMAIN [--wallet NAME] [--field NAME=VALUE]... [--json]\n  fresnica anchor withdraw CODE:GISSUER --home-domain DOMAIN [--wallet NAME] [--field NAME=VALUE]... [--json]\n  fresnica anchor status CODE:GISSUER TRANSACTION_ID --home-domain DOMAIN [--wallet NAME] [--protocol sep24|sep6] [--pay] [--json]\n  fresnica anchor customer CODE:GISSUER --home-domain DOMAIN [--wallet NAME] [--id ID] [--transaction ID] [--type TYPE] [--lang LANG] [--input PATH|-] [--json]";

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
        "status" => command_status(&args[1..]),
        "customer" => command_customer(&args[1..]),
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
    if kind == AnchorTransferKind::Deposit {
        host_anchor_receive_ready(&options)?;
    }
    let discovery = discover_anchor_at(&options.asset, &options.home_domain)?;
    let protocol = select_anchor_transfer_protocol(&discovery.capabilities, kind)?;
    let (network, wallet, address, token) =
        if anchor_transfer_requires_sep10(&discovery.capabilities, protocol, kind) {
            let auth = host_anchor_auth(&options)?;
            (auth.network, auth.wallet, auth.address, Some(auth.token))
        } else {
            let context = host_context(options.wallet.as_deref())?;
            (context.network, context.wallet, context.address, None)
        };

    match protocol {
        AnchorProtocol::Sep24 => {
            let token = token
                .as_ref()
                .ok_or_else(|| "SEP-24 requires SEP-10 authentication".to_owned())?;
            let result = start_anchor_sep24_transfer(
                &address,
                &discovery.asset,
                &discovery.capabilities,
                kind,
                &options.fields,
                token.as_str(),
            )?;
            if options.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "asset": discovery.asset.display(),
                        "network": network,
                        "wallet": wallet,
                        "address": address,
                        "domain": discovery.home_domain,
                        "kind": kind,
                        "protocol": AnchorProtocol::Sep24,
                        "url": result.url,
                        "id": result.transaction_id,
                    }))
                    .map_err(|error| format!("unable to encode SEP-24 result: {error}"))?
                );
            } else {
                println!("Anchor · {} [{}]", discovery.asset.display(), network);
                println!("Action: {} via SEP-24", kind.endpoint());
                println!("Domain: {}", discovery.home_domain);
                println!("Wallet: {wallet}");
                println!("Open URL: {}", result.url);
                println!("Transfer ID: {}", result.transaction_id);
                println!(
                    "Status: fresnica --network {network} anchor status {} {} --home-domain {} --protocol sep24 --wallet {wallet}",
                    discovery.asset.display(),
                    result.transaction_id,
                    discovery.home_domain
                );
            }
        }
        AnchorProtocol::Sep6 => {
            let response = start_anchor_sep6_transfer(
                &address,
                &discovery.asset,
                &discovery.capabilities,
                kind,
                &options.fields,
                token.as_ref().map(|value| value.as_str()),
            )?;
            if options.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "asset": discovery.asset.display(),
                        "network": network,
                        "wallet": wallet,
                        "address": address,
                        "domain": discovery.home_domain,
                        "kind": kind,
                        "protocol": AnchorProtocol::Sep6,
                        "response": response,
                    }))
                    .map_err(|error| format!("unable to encode SEP-6 result: {error}"))?
                );
            } else {
                println!("Anchor · {} [{}]", discovery.asset.display(), network);
                println!("Action: {} via SEP-6", kind.endpoint());
                println!("Domain: {}", discovery.home_domain);
                println!("Wallet: {wallet}");
                println!("Instructions:");
                println!(
                    "{}",
                    serde_json::to_string_pretty(&response)
                        .map_err(|error| format!("unable to encode SEP-6 response: {error}"))?
                );
                if let Some(transaction_id) = anchor_transaction_text(&response, "id") {
                    println!(
                        "Status: fresnica --network {network} anchor status {} {transaction_id} --home-domain {} --protocol sep6 --wallet {wallet}",
                        discovery.asset.display(),
                        discovery.home_domain
                    );
                }
            }
        }
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

fn host_anchor_receive_ready(options: &Options) -> Result<(), String> {
    let host = env::var_os("FRESNICA_PLUGIN_HOST")
        .ok_or_else(|| "FRESNICA_PLUGIN_HOST is missing".to_owned())?;
    let mut command = Command::new(host);
    command
        .arg("--network")
        .arg(plugin_network()?)
        .arg("__plugin-host")
        .arg("anchor-receive")
        .arg(&options.asset);
    if let Some(wallet) = &options.wallet {
        command.arg("--wallet").arg(wallet);
    }
    let output = command
        .output()
        .map_err(|error| format!("unable to call Fresnica receive preflight: {error}"))?;
    if output.status.success() {
        return Ok(());
    }
    let detail = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    if detail.is_empty() {
        Err(format!(
            "Anchor deposit receive preflight failed with status {}",
            output.status
        ))
    } else {
        Err(format!("Anchor deposit receive preflight failed: {detail}"))
    }
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

#[derive(Debug, Deserialize)]
struct AnchorCustomerInput {
    #[serde(default)]
    fields: BTreeMap<String, JsonValue>,
    #[serde(default)]
    files: BTreeMap<String, AnchorCustomerInputFile>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum AnchorCustomerInputFile {
    Path(String),
    Detail {
        path: String,
        #[serde(default)]
        content_type: Option<String>,
    },
}

#[derive(Debug)]
struct CustomerOptions {
    asset: String,
    home_domain: String,
    wallet: Option<String>,
    customer_id: Option<String>,
    transaction_id: Option<String>,
    customer_type: Option<String>,
    lang: Option<String>,
    input: Option<String>,
    json: bool,
}

impl CustomerOptions {
    fn parse(args: &[String]) -> Result<Self, String> {
        let asset = args.first().ok_or_else(|| HELP.to_owned())?.to_owned();
        let mut home_domain = None;
        let mut wallet = None;
        let mut customer_id = None;
        let mut transaction_id = None;
        let mut customer_type = None;
        let mut lang = None;
        let mut input = None;
        let mut json = false;
        let mut index = 1;
        while index < args.len() {
            let target = match args[index].as_str() {
                "--home-domain" => &mut home_domain,
                "--wallet" => &mut wallet,
                "--id" => &mut customer_id,
                "--transaction" => &mut transaction_id,
                "--type" => &mut customer_type,
                "--lang" => &mut lang,
                "--input" => &mut input,
                "--json" => {
                    json = true;
                    index += 1;
                    continue;
                }
                _ => return Err(HELP.to_owned()),
            };
            let flag = args[index].clone();
            index += 1;
            *target = Some(
                args.get(index)
                    .ok_or_else(|| format!("{flag} requires a value"))?
                    .to_owned(),
            );
            index += 1;
        }
        if input.is_some() && lang.is_some() {
            return Err("--lang is only valid for SEP-12 customer status lookup".to_owned());
        }
        Ok(Self {
            asset,
            home_domain: home_domain
                .ok_or_else(|| "--home-domain is required for the plugin spike".to_owned())?,
            wallet,
            customer_id,
            transaction_id,
            customer_type,
            lang,
            input,
            json,
        })
    }
}

fn command_customer(args: &[String]) -> Result<(), String> {
    let options = CustomerOptions::parse(args)?;
    let discovery = discover_anchor_at(&options.asset, &options.home_domain)?;
    let server = discovery.capabilities.customer_server().ok_or_else(|| {
        format!(
            "{} does not advertise KYC_SERVER or TRANSFER_SERVER for SEP-12",
            discovery.capabilities.domain
        )
    })?;
    let auth = host_anchor_auth(&Options {
        asset: options.asset.clone(),
        home_domain: options.home_domain.clone(),
        wallet: options.wallet.clone(),
        fields: BTreeMap::new(),
        json: false,
    })?;

    if let Some(input) = options.input.as_deref() {
        let customer_input = read_anchor_customer_input(input)?;
        let update = build_anchor_customer_update(
            options.customer_id,
            options.transaction_id,
            options.customer_type,
            customer_input,
        )?;
        let result = put_anchor_customer(server, auth.token.as_str(), &update)?;
        if options.json {
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "asset": discovery.asset.display(),
                    "network": auth.network,
                    "anchor": discovery.home_domain,
                    "wallet": auth.wallet,
                    "address": auth.address,
                    "customer": result,
                }))
                .map_err(|error| format!("unable to encode SEP-12 customer result: {error}"))?
            );
        } else {
            println!(
                "Anchor customer updated · {} [{}]",
                discovery.asset.display(),
                auth.network
            );
            println!("Anchor: {}", discovery.home_domain);
            println!("Wallet: {}", auth.wallet);
            println!("Customer: {}", result.id);
            println!("Next: query customer status with --id {}", result.id);
        }
        return Ok(());
    }

    let query = AnchorCustomerQuery {
        id: options.customer_id,
        customer_type: options.customer_type,
        transaction_id: options.transaction_id,
        lang: options.lang,
    };
    let snapshot = get_anchor_customer(server, auth.token.as_str(), &query)?;
    render_anchor_customer(
        &discovery.asset.display(),
        &auth,
        &discovery.home_domain,
        &snapshot,
        options.json,
    )
}

fn read_anchor_customer_input(source: &str) -> Result<AnchorCustomerInput, String> {
    let text = if source == "-" {
        let mut text = String::new();
        std::io::stdin()
            .read_to_string(&mut text)
            .map_err(|error| format!("unable to read SEP-12 JSON from stdin: {error}"))?;
        text
    } else {
        std::fs::read_to_string(source)
            .map_err(|error| format!("unable to read SEP-12 input {source}: {error}"))?
    };
    serde_json::from_str(&text).map_err(|error| format!("invalid SEP-12 input JSON: {error}"))
}

fn build_anchor_customer_update(
    id: Option<String>,
    transaction_id: Option<String>,
    customer_type: Option<String>,
    input: AnchorCustomerInput,
) -> Result<AnchorCustomerUpdate, String> {
    let mut fields = BTreeMap::new();
    for (name, value) in input.fields {
        let value = match value {
            JsonValue::String(value) => value,
            JsonValue::Number(value) => value.to_string(),
            _ => {
                return Err(format!(
                    "SEP-12 field {name} must be a string or number in the Rust CLI input"
                ));
            }
        };
        fields.insert(name, value);
    }

    let mut files = Vec::with_capacity(input.files.len());
    for (name, file) in input.files {
        let (path, content_type) = match file {
            AnchorCustomerInputFile::Path(path) => (path, None),
            AnchorCustomerInputFile::Detail { path, content_type } => (path, content_type),
        };
        let bytes = std::fs::read(&path)
            .map_err(|error| format!("unable to read SEP-12 file {path}: {error}"))?;
        let file_name = Path::new(&path)
            .file_name()
            .and_then(|value| value.to_str())
            .filter(|value| !value.is_empty())
            .ok_or_else(|| format!("SEP-12 file path has no usable file name: {path}"))?;
        files.push(AnchorCustomerFile {
            name,
            file_name: file_name.to_owned(),
            content_type,
            bytes,
        });
    }

    Ok(AnchorCustomerUpdate {
        id,
        customer_type,
        transaction_id,
        fields,
        files,
    })
}

fn render_anchor_customer(
    asset: &str,
    auth: &HostAuth,
    domain: &str,
    snapshot: &AnchorCustomerSnapshot,
    json_output: bool,
) -> Result<(), String> {
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "asset": asset,
                "network": auth.network,
                "anchor": domain,
                "wallet": auth.wallet,
                "address": auth.address,
                "customer": snapshot,
            }))
            .map_err(|error| format!("unable to encode SEP-12 customer status: {error}"))?
        );
        return Ok(());
    }

    println!("Anchor customer · {asset} [{}]", auth.network);
    println!("Anchor: {domain}");
    println!("Wallet: {}", auth.wallet);
    println!("Status: {}", snapshot.status.label());
    if let Some(id) = snapshot.id.as_deref() {
        println!("Customer: {id}");
    }
    if let Some(message) = snapshot.message.as_deref() {
        println!("Message: {message}");
    }
    if !snapshot.required_fields.is_empty() {
        println!("Required:");
        for field in &snapshot.required_fields {
            let kind = field.field_type.as_deref().unwrap_or("unknown");
            let required = if field.optional {
                "optional"
            } else {
                "required"
            };
            let mut detail = format!("  {} [{} · {}]", field.name, kind, required);
            if !field.choices.is_empty() {
                detail.push_str(&format!(" choices={}", field.choices.join("|")));
            }
            println!("{detail}");
            if let Some(description) = field.description.as_deref() {
                println!("    {description}");
            }
        }
    }
    if !snapshot.provided_fields.is_empty() {
        println!("Provided:");
        for field in &snapshot.provided_fields {
            let status = field
                .status
                .map(|value| value.label())
                .unwrap_or("RECEIVED");
            println!("  {} [{}]", field.name, status);
            if let Some(error) = field.error.as_deref() {
                println!("    {error}");
            }
        }
    }
    Ok(())
}

#[derive(Debug)]
struct HostContext {
    network: String,
    wallet: String,
    address: String,
}

#[derive(Debug, Deserialize)]
struct HostContextWire {
    schema: String,
    network: String,
    wallet: String,
    address: String,
}

fn host_context(wallet: Option<&str>) -> Result<HostContext, String> {
    let host = env::var_os("FRESNICA_PLUGIN_HOST")
        .ok_or_else(|| "FRESNICA_PLUGIN_HOST is missing".to_owned())?;
    let mut command = Command::new(host);
    command
        .arg("--network")
        .arg(plugin_network()?)
        .arg("__plugin-host")
        .arg("context")
        .stdin(Stdio::inherit())
        .stderr(Stdio::inherit())
        .stdout(Stdio::piped());
    if let Some(wallet) = wallet {
        command.arg("--wallet").arg(wallet);
    }
    let output = command
        .output()
        .map_err(|error| format!("unable to call Fresnica plugin host: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "Fresnica plugin host failed with status {}",
            output.status
        ));
    }
    let wire: HostContextWire = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("invalid plugin host response: {error}"))?;
    if wire.schema != "fresnica-plugin-context-v1" {
        return Err("unsupported Fresnica plugin host response".to_owned());
    }
    Ok(HostContext {
        network: wire.network,
        wallet: wire.wallet,
        address: wire.address,
    })
}

#[derive(Debug)]
struct StatusOptions {
    asset: String,
    transaction_id: String,
    home_domain: String,
    wallet: Option<String>,
    protocol: Option<AnchorProtocol>,
    pay: bool,
    json: bool,
}

impl StatusOptions {
    fn parse(args: &[String]) -> Result<Self, String> {
        let asset = args.first().ok_or_else(|| HELP.to_owned())?.to_owned();
        let transaction_id = args
            .get(1)
            .ok_or_else(|| HELP.to_owned())?
            .trim()
            .to_owned();
        if transaction_id.is_empty() {
            return Err("anchor transaction id must not be empty".to_owned());
        }
        let mut home_domain = None;
        let mut wallet = None;
        let mut protocol = None;
        let mut pay = false;
        let mut json = false;
        let mut index = 2;
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
                "--wallet" => {
                    index += 1;
                    wallet = Some(
                        args.get(index)
                            .ok_or_else(|| "--wallet requires a wallet name".to_owned())?
                            .to_owned(),
                    );
                    index += 1;
                }
                "--protocol" => {
                    index += 1;
                    protocol = Some(parse_anchor_protocol(
                        args.get(index)
                            .ok_or_else(|| "--protocol requires sep24 or sep6".to_owned())?,
                    )?);
                    index += 1;
                }
                "--pay" => {
                    pay = true;
                    index += 1;
                }
                "-y" | "--yes" => {
                    return Err(
                        "native plugin payment proposals cannot bypass Fresnica interactive review with --yes"
                            .to_owned(),
                    );
                }
                "--json" => {
                    json = true;
                    index += 1;
                }
                _ => return Err(HELP.to_owned()),
            }
        }
        if json && pay {
            return Err(
                "--json cannot be combined with --pay because payment requires interactive Fresnica review"
                    .to_owned(),
            );
        }
        Ok(Self {
            asset,
            transaction_id,
            home_domain: home_domain
                .ok_or_else(|| "--home-domain is required for the plugin spike".to_owned())?,
            wallet,
            protocol,
            pay,
            json,
        })
    }
}

fn parse_anchor_protocol(value: &str) -> Result<AnchorProtocol, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "sep24" | "sep-24" => Ok(AnchorProtocol::Sep24),
        "sep6" | "sep-6" => Ok(AnchorProtocol::Sep6),
        _ => Err("--protocol must be sep24 or sep6".to_owned()),
    }
}

fn command_status(args: &[String]) -> Result<(), String> {
    let options = StatusOptions::parse(args)?;
    let discovery = discover_anchor_at(&options.asset, &options.home_domain)?;
    let protocol = select_anchor_status_protocol(&discovery.capabilities, options.protocol)?;

    let (network, wallet, address, token) =
        if anchor_status_requires_sep10(&discovery.capabilities, protocol) {
            let auth = host_anchor_auth(&Options {
                asset: options.asset.clone(),
                home_domain: options.home_domain.clone(),
                wallet: options.wallet.clone(),
                fields: BTreeMap::new(),
                json: false,
            })?;
            (auth.network, auth.wallet, auth.address, Some(auth.token))
        } else {
            let context = host_context(options.wallet.as_deref())?;
            (context.network, context.wallet, context.address, None)
        };

    let transaction = fetch_anchor_transaction(
        &discovery.capabilities,
        protocol,
        &options.transaction_id,
        token.as_ref().map(|value| value.as_str()),
    )?;

    if options.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "asset": discovery.asset.display(),
                "network": network,
                "wallet": wallet,
                "address": address,
                "domain": discovery.home_domain,
                "protocol": protocol,
                "transaction": transaction,
            }))
            .map_err(|error| format!("unable to encode anchor transaction status: {error}"))?
        );
        return Ok(());
    }

    println!(
        "Anchor transaction · {} [{}]",
        discovery.asset.display(),
        network
    );
    println!("Protocol: {}", protocol.label());
    println!("Domain: {}", discovery.home_domain);
    println!("Wallet: {wallet}");
    for (label, key) in [
        ("ID", "id"),
        ("Kind", "kind"),
        ("Status", "status"),
        ("Amount in", "amount_in"),
        ("Amount out", "amount_out"),
        ("Fee", "amount_fee"),
        ("Stellar TX", "stellar_transaction_id"),
        ("External TX", "external_transaction_id"),
        ("Action by", "user_action_required_by"),
        ("More info", "more_info_url"),
    ] {
        if let Some(value) = anchor_transaction_text(&transaction, key) {
            println!("{label:<12} {value}");
        }
    }
    if !options.pay {
        return Ok(());
    }

    let payment =
        anchor_withdrawal_payment_from_transaction(&transaction, &address, &discovery.asset)?;
    host_anchor_payment(AnchorPaymentProposalWire {
        schema: "fresnica-plugin-anchor-payment-v1",
        wallet,
        asset: discovery.asset.display(),
        amount: payment.amount,
        destination: payment.destination,
        memo: payment_memo_wire(payment.memo),
        anchor: discovery.home_domain,
        transaction_id: options.transaction_id,
        external: anchor_transaction_text(&transaction, "to").map(str::to_owned),
        details: anchor_transaction_text(&transaction, "external_extra_text").map(str::to_owned),
        more_info: anchor_transaction_text(&transaction, "more_info_url").map(str::to_owned),
    })
}

const ANCHOR_PAYMENT_PROPOSAL_ENV: &str = "FRESNICA_PLUGIN_ANCHOR_PAYMENT";

#[derive(serde::Serialize)]
struct AnchorPaymentProposalWire {
    schema: &'static str,
    wallet: String,
    asset: String,
    amount: String,
    destination: String,
    memo: PaymentMemoWire,
    anchor: String,
    transaction_id: String,
    external: Option<String>,
    details: Option<String>,
    more_info: Option<String>,
}

#[derive(serde::Serialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
enum PaymentMemoWire {
    None,
    Text(String),
    Id(u64),
    Hash(Vec<u8>),
}

fn payment_memo_wire(memo: PaymentMemo) -> PaymentMemoWire {
    match memo {
        PaymentMemo::None => PaymentMemoWire::None,
        PaymentMemo::Text(value) => PaymentMemoWire::Text(value),
        PaymentMemo::Id(value) => PaymentMemoWire::Id(value),
        PaymentMemo::Hash(value) => PaymentMemoWire::Hash(value.to_vec()),
    }
}

fn host_anchor_payment(proposal: AnchorPaymentProposalWire) -> Result<(), String> {
    let host = env::var_os("FRESNICA_PLUGIN_HOST")
        .ok_or_else(|| "FRESNICA_PLUGIN_HOST is missing".to_owned())?;
    let payload = serde_json::to_string(&proposal)
        .map_err(|error| format!("unable to encode anchor payment proposal: {error}"))?;
    let status = Command::new(host)
        .arg("--network")
        .arg(plugin_network()?)
        .arg("__plugin-host")
        .arg("anchor-payment")
        .env(ANCHOR_PAYMENT_PROPOSAL_ENV, payload)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|error| format!("unable to call Fresnica payment host: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("Fresnica payment host failed with status {status}"))
    }
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
    fn status_parses_explicit_protocol_and_rejects_noninteractive_payment_approval() {
        let options = StatusOptions::parse(&[
            ASSET.to_owned(),
            "transfer-1".to_owned(),
            "--home-domain".to_owned(),
            "testanchor.stellar.org".to_owned(),
            "--protocol".to_owned(),
            "sep24".to_owned(),
            "--json".to_owned(),
        ])
        .unwrap();
        assert_eq!(options.protocol, Some(AnchorProtocol::Sep24));
        assert!(options.json);

        let pay = StatusOptions::parse(&[
            ASSET.to_owned(),
            "transfer-1".to_owned(),
            "--home-domain".to_owned(),
            "testanchor.stellar.org".to_owned(),
            "--pay".to_owned(),
        ])
        .unwrap();
        assert!(pay.pay);

        let error = StatusOptions::parse(&[
            ASSET.to_owned(),
            "transfer-1".to_owned(),
            "--home-domain".to_owned(),
            "testanchor.stellar.org".to_owned(),
            "--pay".to_owned(),
            "--yes".to_owned(),
        ])
        .unwrap_err();
        assert!(error.contains("cannot bypass Fresnica interactive review"));
    }

    #[test]
    fn customer_parser_separates_status_language_from_update_input() {
        let options = CustomerOptions::parse(&[
            ASSET.to_owned(),
            "--home-domain".to_owned(),
            "testanchor.stellar.org".to_owned(),
            "--wallet".to_owned(),
            "treasury".to_owned(),
            "--id".to_owned(),
            "customer-1".to_owned(),
            "--lang".to_owned(),
            "en".to_owned(),
        ])
        .unwrap();
        assert_eq!(options.customer_id.as_deref(), Some("customer-1"));
        assert_eq!(options.lang.as_deref(), Some("en"));

        let error = CustomerOptions::parse(&[
            ASSET.to_owned(),
            "--home-domain".to_owned(),
            "testanchor.stellar.org".to_owned(),
            "--lang".to_owned(),
            "en".to_owned(),
            "--input".to_owned(),
            "customer.json".to_owned(),
        ])
        .unwrap_err();
        assert_eq!(
            error,
            "--lang is only valid for SEP-12 customer status lookup"
        );
    }

    #[test]
    fn sep12_customer_input_accepts_scalar_fields_and_binary_files() {
        let path =
            std::env::temp_dir().join(format!("fresnica-sep12-plugin-{}-id.jpg", process::id()));
        std::fs::write(&path, [1_u8, 2, 3]).unwrap();
        let input = AnchorCustomerInput {
            fields: BTreeMap::from([
                ("first_name".to_owned(), JsonValue::String("Ada".to_owned())),
                ("annual_income".to_owned(), json!(42)),
            ]),
            files: BTreeMap::from([(
                "photo_id_front".to_owned(),
                AnchorCustomerInputFile::Detail {
                    path: path.to_string_lossy().into_owned(),
                    content_type: Some("image/jpeg".to_owned()),
                },
            )]),
        };
        let update = build_anchor_customer_update(
            Some("customer-1".to_owned()),
            None,
            Some("sep6".to_owned()),
            input,
        )
        .unwrap();
        assert_eq!(
            update.fields.get("first_name").map(String::as_str),
            Some("Ada")
        );
        assert_eq!(
            update.fields.get("annual_income").map(String::as_str),
            Some("42")
        );
        assert_eq!(update.files.len(), 1);
        assert_eq!(update.files[0].name, "photo_id_front");
        assert_eq!(update.files[0].content_type.as_deref(), Some("image/jpeg"));
        assert_eq!(update.files[0].bytes, vec![1, 2, 3]);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn sep12_customer_input_rejects_nested_values() {
        let input = AnchorCustomerInput {
            fields: BTreeMap::from([("organization".to_owned(), json!({"name": "Example"}))]),
            files: BTreeMap::new(),
        };
        assert_eq!(
            build_anchor_customer_update(None, None, None, input).unwrap_err(),
            "SEP-12 field organization must be a string or number in the Rust CLI input"
        );
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
