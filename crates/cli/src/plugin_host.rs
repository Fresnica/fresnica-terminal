use fresnica_client::{discover_anchor_at, FresnicaClient, PaymentMemo};
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
        Some("anchor-receive") => command_anchor_receive(client, &arguments[1..]),
        Some("anchor-payment") => command_anchor_payment(client, &arguments[1..]),
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

fn command_anchor_receive(client: &FresnicaClient, arguments: &[String]) -> Result<(), String> {
    let asset = arguments
        .first()
        .ok_or_else(|| "anchor-receive requires an asset".to_owned())?;
    let wallet = match &arguments[1..] {
        [] => None,
        [flag, name] if flag == "--wallet" => Some(name.as_str()),
        _ => return Err("anchor-receive accepts ASSET [--wallet NAME]".to_owned()),
    };
    client.ensure_payment_receive_ready(wallet, asset)
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

const ANCHOR_PAYMENT_PROPOSAL_ENV: &str = "FRESNICA_PLUGIN_ANCHOR_PAYMENT";
const MAX_ANCHOR_PAYMENT_PROPOSAL_BYTES: usize = 16 * 1024;

#[derive(serde::Deserialize)]
struct AnchorPaymentProposal {
    schema: String,
    wallet: String,
    asset: String,
    amount: String,
    destination: String,
    memo: PaymentMemoWire,
    anchor: String,
    transaction_id: Option<String>,
    external: Option<String>,
    details: Option<String>,
    more_info: Option<String>,
}

#[derive(serde::Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
enum PaymentMemoWire {
    None,
    Text(String),
    Id(u64),
    Hash(Vec<u8>),
}

impl PaymentMemoWire {
    fn into_payment_memo(self) -> Result<PaymentMemo, String> {
        match self {
            Self::None => Ok(PaymentMemo::None),
            Self::Text(value) => Ok(PaymentMemo::Text(value)),
            Self::Id(value) => Ok(PaymentMemo::Id(value)),
            Self::Hash(value) => {
                let hash: [u8; 32] = value.try_into().map_err(|_| {
                    "anchor payment hash memo must contain exactly 32 bytes".to_owned()
                })?;
                Ok(PaymentMemo::Hash(hash))
            }
        }
    }
}

fn command_anchor_payment(client: &FresnicaClient, arguments: &[String]) -> Result<(), String> {
    if !arguments.is_empty() {
        return Err("anchor-payment accepts no command-line arguments".to_owned());
    }
    let raw = std::env::var(ANCHOR_PAYMENT_PROPOSAL_ENV)
        .map_err(|_| "anchor-payment proposal payload is missing".to_owned())?;
    if raw.len() > MAX_ANCHOR_PAYMENT_PROPOSAL_BYTES {
        return Err("anchor-payment proposal payload is too large".to_owned());
    }
    let proposal: AnchorPaymentProposal = serde_json::from_str(&raw)
        .map_err(|error| format!("invalid anchor-payment proposal: {error}"))?;
    if proposal.schema != "fresnica-plugin-anchor-payment-v1" {
        return Err("unsupported anchor-payment proposal schema".to_owned());
    }
    let record = client.resolve_wallet(Some(&proposal.wallet))?;
    let memo = proposal.memo.into_payment_memo()?;

    println!("Anchor withdrawal payment handoff");
    println!("Anchor:   {}", proposal.anchor);
    if let Some(transaction_id) = proposal.transaction_id.as_deref() {
        println!("Transfer: {transaction_id}");
    } else {
        println!("Transfer: immediate SEP-6 response");
    }
    if let Some(value) = proposal.external.as_deref() {
        println!("External: {value}");
    }
    if let Some(value) = proposal.details.as_deref() {
        println!("Details:  {value}");
    }
    if let Some(value) = proposal.more_info.as_deref() {
        println!("More info: {value}");
    }
    println!("Authorization: interactive Fresnica payment review is mandatory");
    println!();

    crate::send::review_and_submit_payment(
        client,
        &record,
        &proposal.amount,
        &proposal.asset,
        &proposal.destination,
        memo,
        false,
    )
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anchor_payment_proposal_accepts_immediate_sep6_without_transaction_id() {
        let proposal: AnchorPaymentProposal = serde_json::from_value(serde_json::json!({
            "schema": "fresnica-plugin-anchor-payment-v1",
            "wallet": "watch",
            "asset": "XRP:GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
            "amount": "5",
            "destination": "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
            "memo": {"type": "none"},
            "anchor": "anchor.example",
            "transaction_id": null,
            "external": "rExample",
            "details": null,
            "more_info": null
        }))
        .unwrap();
        assert!(proposal.transaction_id.is_none());
    }

    #[test]
    fn anchor_payment_hash_memo_requires_exactly_32_bytes() {
        let error = PaymentMemoWire::Hash(vec![7; 31])
            .into_payment_memo()
            .unwrap_err();
        assert!(error.contains("exactly 32 bytes"));
        assert!(matches!(
            PaymentMemoWire::Hash(vec![7; 32])
                .into_payment_memo()
                .unwrap(),
            PaymentMemo::Hash(_)
        ));
    }
}
