use fresnica_client::{
    balance_asset_label, operation_summary, AccountState, FresnicaClient, LedgerSignerKind,
};
use serde_json::{json, Value};

pub fn command_account(client: &FresnicaClient, arguments: &[String]) -> Result<(), String> {
    let options = parse_output_options(arguments, "fresnica account [--wallet NAME] [--json]")?;
    crate::diagnostics::stage("account: fetch ledger state");
    let snapshot = client.account(options.wallet.as_deref())?;
    if options.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&account_json(&snapshot.account))
                .map_err(|error| format!("unable to encode account data: {error}"))?
        );
        return Ok(());
    }

    println!("Wallet:       {}", snapshot.wallet.name);
    println!("Address:      {}", snapshot.wallet.address);
    println!("Network:      {}", snapshot.wallet.network);
    println!("Sequence:     {}", snapshot.account.sequence);
    println!("Subentries:   {}", snapshot.account.subentry_count);
    println!("Sponsoring:   {}", snapshot.account.num_sponsoring);
    println!("Sponsored:    {}", snapshot.account.num_sponsored);
    println!(
        "Home domain:  {}",
        snapshot.account.home_domain.as_deref().unwrap_or("-")
    );
    println!(
        "Thresholds:   low {} / medium {} / high {}",
        snapshot.account.thresholds.low,
        snapshot.account.thresholds.medium,
        snapshot.account.thresholds.high
    );
    println!("Signers:      {}", snapshot.account.signers.len());
    Ok(())
}

fn account_json(account: &AccountState) -> Value {
    let signers = account
        .signers
        .iter()
        .map(|signer| {
            json!({
                "kind": signer_kind_label(&signer.condition.kind),
                "key": signer.condition.key.as_str(),
                "weight": signer.weight,
            })
        })
        .collect::<Vec<_>>();

    json!({
        "account_id": account.account_id.as_str(),
        "sequence": account.sequence,
        "subentry_count": account.subentry_count,
        "num_sponsoring": account.num_sponsoring,
        "num_sponsored": account.num_sponsored,
        "home_domain": account.home_domain.as_deref(),
        "thresholds": {
            "low": account.thresholds.low,
            "medium": account.thresholds.medium,
            "high": account.thresholds.high,
        },
        "signers": signers,
    })
}

fn signer_kind_label(kind: &LedgerSignerKind) -> &'static str {
    match kind {
        LedgerSignerKind::Ed25519PublicKey => "ed25519",
        LedgerSignerKind::PreauthorizedTransaction => "preauth_tx",
        LedgerSignerKind::HashX => "hash_x",
        LedgerSignerKind::Ed25519SignedPayload => "ed25519_signed_payload",
    }
}

pub fn command_balance(client: &FresnicaClient, arguments: &[String]) -> Result<(), String> {
    let options = parse_output_options(arguments, "fresnica balance [--wallet NAME] [--json]")?;
    crate::diagnostics::stage("balance: fetch Horizon state");
    let snapshot = client.balances(options.wallet.as_deref())?;

    if options.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&snapshot.balances)
                .map_err(|error| format!("unable to encode balance data: {error}"))?
        );
        return Ok(());
    }

    println!(
        "Wallet: {} [{}]",
        snapshot.wallet.name, snapshot.wallet.network
    );
    println!(
        "{:<72} {:>16} {:>16} {:>16}",
        "Asset", "Balance", "Selling", "Buying"
    );
    for balance in &snapshot.balances {
        println!(
            "{:<72} {:>16} {:>16} {:>16}",
            balance_asset_label(balance),
            text(balance, "balance").unwrap_or("0"),
            text(balance, "selling_liabilities").unwrap_or("0"),
            text(balance, "buying_liabilities").unwrap_or("0"),
        );
    }
    Ok(())
}

pub fn command_history(client: &FresnicaClient, arguments: &[String]) -> Result<(), String> {
    let options = parse_history_options(arguments)?;
    crate::diagnostics::stage("history: fetch Horizon operations");
    let snapshot = client.history(options.wallet.as_deref(), options.limit)?;

    if options.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&snapshot.operations)
                .map_err(|error| format!("unable to encode history data: {error}"))?
        );
        return Ok(());
    }

    println!(
        "Wallet: {} [{}]",
        snapshot.wallet.name, snapshot.wallet.network
    );
    if snapshot.operations.is_empty() {
        println!("No account operations.");
        return Ok(());
    }
    for operation in &snapshot.operations {
        let created_at = text(operation, "created_at").unwrap_or("?");
        let operation_type = text(operation, "type").unwrap_or("unknown");
        println!(
            "{:<20} {:<28} {}",
            created_at,
            operation_type,
            operation_summary(operation, &snapshot.wallet.address)
        );
    }
    Ok(())
}

struct OutputOptions {
    wallet: Option<String>,
    json: bool,
}

#[derive(Debug)]
struct HistoryOptions {
    wallet: Option<String>,
    json: bool,
    limit: usize,
}

fn parse_output_options(arguments: &[String], usage: &str) -> Result<OutputOptions, String> {
    let mut wallet = None;
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--wallet" => {
                index += 1;
                wallet = Some(
                    arguments
                        .get(index)
                        .ok_or_else(|| usage.to_owned())?
                        .to_owned(),
                );
                index += 1;
            }
            "--json" => {
                json = true;
                index += 1;
            }
            _ => return Err(usage.to_owned()),
        }
    }
    Ok(OutputOptions { wallet, json })
}

fn parse_history_options(arguments: &[String]) -> Result<HistoryOptions, String> {
    let usage = "fresnica history [--wallet NAME] [--limit N] [--json]";
    let mut wallet = None;
    let mut json = false;
    let mut limit = 20usize;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--wallet" => {
                index += 1;
                wallet = Some(
                    arguments
                        .get(index)
                        .ok_or_else(|| usage.to_owned())?
                        .to_owned(),
                );
                index += 1;
            }
            "--limit" => {
                index += 1;
                limit = arguments
                    .get(index)
                    .ok_or_else(|| usage.to_owned())?
                    .parse()
                    .map_err(|_| "--limit requires an integer from 1 to 200".to_owned())?;
                if !(1..=200).contains(&limit) {
                    return Err("--limit must be from 1 to 200".to_owned());
                }
                index += 1;
            }
            "--json" => {
                json = true;
                index += 1;
            }
            _ => return Err(usage.to_owned()),
        }
    }
    Ok(HistoryOptions {
        wallet,
        json,
        limit,
    })
}

fn text<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

#[cfg(test)]
mod tests {
    use fresnica_client::{
        AccountThresholds, LedgerSignerCondition, WeightedLedgerSigner,
    };

    use super::*;

    #[test]
    fn history_limit_is_bounded_by_horizon_page_size() {
        let args = vec!["--limit".to_owned(), "201".to_owned()];
        assert_eq!(
            parse_history_options(&args).unwrap_err(),
            "--limit must be from 1 to 200"
        );
    }

    #[test]
    fn output_options_accept_wallet_and_json_in_either_order() {
        let args = vec![
            "--json".to_owned(),
            "--wallet".to_owned(),
            "alpha".to_owned(),
        ];
        let options = parse_output_options(&args, "usage").unwrap();
        assert!(options.json);
        assert_eq!(options.wallet.as_deref(), Some("alpha"));
    }

    #[test]
    fn account_json_is_provider_neutral_and_typed() {
        let account = AccountState {
            account_id: "GACCOUNT".to_owned(),
            sequence: 42,
            subentry_count: 7,
            num_sponsoring: 2,
            num_sponsored: 1,
            home_domain: Some("example.com".to_owned()),
            thresholds: AccountThresholds {
                low: 1,
                medium: 2,
                high: 3,
            },
            signers: vec![WeightedLedgerSigner {
                condition: LedgerSignerCondition {
                    kind: LedgerSignerKind::Ed25519PublicKey,
                    key: "GSIGNER".to_owned(),
                },
                weight: 2,
            }],
        };

        let value = account_json(&account);

        assert_eq!(value["account_id"], json!("GACCOUNT"));
        assert_eq!(value["sequence"], json!(42));
        assert_eq!(value["thresholds"]["medium"], json!(2));
        assert_eq!(value["signers"][0]["kind"], json!("ed25519"));
        assert_eq!(value["signers"][0]["weight"], json!(2));
        assert!(value.get("balances").is_none());
    }

    #[test]
    fn account_json_signer_kinds_follow_stellar_semantics() {
        assert_eq!(
            signer_kind_label(&LedgerSignerKind::Ed25519PublicKey),
            "ed25519"
        );
        assert_eq!(
            signer_kind_label(&LedgerSignerKind::PreauthorizedTransaction),
            "preauth_tx"
        );
        assert_eq!(signer_kind_label(&LedgerSignerKind::HashX), "hash_x");
        assert_eq!(
            signer_kind_label(&LedgerSignerKind::Ed25519SignedPayload),
            "ed25519_signed_payload"
        );
    }
}
