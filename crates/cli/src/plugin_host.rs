use fresnica_client::{discover_anchor_at, FresnicaClient};
use serde::Serialize;
use serde_json::json;

pub(crate) fn command(
    client: &FresnicaClient,
    network: &str,
    arguments: &[String],
) -> Result<(), String> {
    if std::env::var("FRESNICA_PLUGIN_API").as_deref() != Ok("1") {
        return Err(
            "plugin host capabilities are available only to a Fresnica-native plugin process"
                .to_owned(),
        );
    }
    match arguments.first().map(String::as_str) {
        Some("context") => command_context(client, network, &arguments[1..]),
        Some("anchor-auth") => command_anchor_auth(client, network, &arguments[1..]),
        _ => Err("invalid plugin host capability".to_owned()),
    }
}

fn command_context(
    client: &FresnicaClient,
    network: &str,
    arguments: &[String],
) -> Result<(), String> {
    let wallet = parse_wallet_only(arguments)?;
    let record = client.resolve_wallet(wallet.as_deref())?;
    println!(
        "{}",
        json!({
            "schema": "fresnica-plugin-context-v1",
            "network": network,
            "wallet": record.name,
            "address": record.address,
            "wallet_type": record.wallet_type,
        })
    );
    Ok(())
}

fn command_anchor_auth(
    client: &FresnicaClient,
    network: &str,
    arguments: &[String],
) -> Result<(), String> {
    let asset = arguments
        .first()
        .ok_or_else(|| "anchor-auth requires an asset".to_owned())?;
    let mut home_domain = None;
    let mut wallet = None;
    let mut index = 1;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--home-domain" => {
                index += 1;
                home_domain = Some(
                    arguments
                        .get(index)
                        .ok_or_else(|| "--home-domain requires a domain".to_owned())?
                        .as_str(),
                );
                index += 1;
            }
            "--wallet" => {
                index += 1;
                wallet = Some(
                    arguments
                        .get(index)
                        .ok_or_else(|| "--wallet requires a wallet name".to_owned())?
                        .as_str(),
                );
                index += 1;
            }
            _ => return Err("invalid anchor-auth plugin host arguments".to_owned()),
        }
    }
    let home_domain = home_domain.ok_or_else(|| "anchor-auth requires --home-domain".to_owned())?;
    let discovery = discover_anchor_at(asset, home_domain)?;
    let record = client.resolve_wallet(wallet)?;
    let token = crate::anchor_auth::authenticate_anchor_sep10(
        client,
        &record,
        network,
        &discovery.home_domain,
        &discovery.capabilities,
    )?;
    let response = AnchorAuthResponse {
        schema: "fresnica-plugin-anchor-auth-v1",
        network,
        wallet: &record.name,
        address: &record.address,
        home_domain: &discovery.home_domain,
        token: token.as_str(),
    };
    serde_json::to_writer(std::io::stdout().lock(), &response)
        .map_err(|error| format!("unable to encode plugin host response: {error}"))?;
    println!();
    Ok(())
}

#[derive(Serialize)]
struct AnchorAuthResponse<'a> {
    schema: &'static str,
    network: &'a str,
    wallet: &'a str,
    address: &'a str,
    home_domain: &'a str,
    token: &'a str,
}

fn parse_wallet_only(arguments: &[String]) -> Result<Option<String>, String> {
    match arguments {
        [] => Ok(None),
        [flag, name] if flag == "--wallet" => Ok(Some(name.to_owned())),
        _ => Err("context accepts only [--wallet NAME]".to_owned()),
    }
}
