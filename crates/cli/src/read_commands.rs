use fresnica_client::{
    AccountState, AssetBalance, BalanceAsset, FresnicaClient, HistoryAsset, HistoryOperation,
    HistoryOperationKind, HistoryTrustAsset, LedgerSignerKind,
};
use fresnica_terminal_presentation::history_operation_summary;
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
    crate::diagnostics::stage("balance: fetch ledger balances");
    let snapshot = client.balances(options.wallet.as_deref())?;

    if options.json {
        let balances = snapshot
            .balances
            .iter()
            .map(balance_json)
            .collect::<Vec<_>>();
        println!(
            "{}",
            serde_json::to_string_pretty(&balances)
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
            balance.asset.identity(),
            balance.balance,
            balance.selling_liabilities,
            balance.buying_liabilities,
        );
    }
    Ok(())
}

fn balance_json(balance: &AssetBalance) -> Value {
    json!({
        "asset": balance_asset_json(&balance.asset),
        "balance": balance.balance.as_str(),
        "selling_liabilities": balance.selling_liabilities.as_str(),
        "buying_liabilities": balance.buying_liabilities.as_str(),
    })
}

fn balance_asset_json(asset: &BalanceAsset) -> Value {
    match asset {
        BalanceAsset::Native => json!({
            "kind": "native",
            "identity": "XLM",
        }),
        BalanceAsset::Issued { code, issuer } => json!({
            "kind": "issued",
            "identity": asset.identity(),
            "code": code,
            "issuer": issuer,
        }),
        BalanceAsset::LiquidityPoolShare { liquidity_pool_id } => json!({
            "kind": "liquidity_pool_share",
            "identity": asset.identity(),
            "liquidity_pool_id": liquidity_pool_id,
        }),
    }
}

pub fn command_history(client: &FresnicaClient, arguments: &[String]) -> Result<(), String> {
    let options = parse_history_options(arguments)?;
    crate::diagnostics::stage("history: fetch account activity");
    let snapshot = client.history(options.wallet.as_deref(), options.limit)?;

    if options.json {
        let operations = snapshot
            .operations
            .iter()
            .map(history_operation_json)
            .collect::<Vec<_>>();
        println!(
            "{}",
            serde_json::to_string_pretty(&operations)
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
        let created_at = operation.created_at.as_deref().unwrap_or("?");
        println!(
            "{:<20} {:<28} {}",
            created_at,
            operation.operation_type(),
            history_operation_summary(operation, &snapshot.wallet.address)
        );
    }
    Ok(())
}

pub(crate) fn history_operation_json(operation: &HistoryOperation) -> Value {
    json!({
        "operation_id": operation.operation_id.as_deref(),
        "paging_token": operation.paging_token.as_deref(),
        "transaction_hash": operation.transaction_hash.as_deref(),
        "created_at": operation.created_at.as_deref(),
        "source_account": operation.source_account.as_deref(),
        "type": operation.operation_type(),
        "details": history_operation_details_json(&operation.kind),
    })
}

fn history_operation_details_json(kind: &HistoryOperationKind) -> Value {
    match kind {
        HistoryOperationKind::Payment {
            from,
            to,
            amount,
            asset,
        } => json!({
            "from": from.as_deref(),
            "to": to.as_deref(),
            "amount": amount.as_deref(),
            "asset": asset.as_ref().map(history_asset_json),
        }),
        HistoryOperationKind::CreateAccount {
            funder,
            account,
            starting_balance,
        } => json!({
            "funder": funder.as_deref(),
            "account": account.as_deref(),
            "starting_balance": starting_balance.as_deref(),
        }),
        HistoryOperationKind::ManageSellOffer {
            offer_id,
            amount,
            selling_asset,
            buying_asset,
            price,
        }
        | HistoryOperationKind::CreatePassiveSellOffer {
            offer_id,
            amount,
            selling_asset,
            buying_asset,
            price,
        }
        | HistoryOperationKind::ManageBuyOffer {
            offer_id,
            amount,
            selling_asset,
            buying_asset,
            price,
        } => json!({
            "offer_id": offer_id.as_deref(),
            "amount": amount.as_deref(),
            "selling_asset": selling_asset.as_ref().map(history_asset_json),
            "buying_asset": buying_asset.as_ref().map(history_asset_json),
            "price": price.as_deref(),
        }),
        HistoryOperationKind::ChangeTrust { asset, limit } => json!({
            "asset": history_trust_asset_json(asset),
            "limit": limit.as_deref(),
        }),
        HistoryOperationKind::AccountMerge { into } => json!({
            "into": into.as_deref(),
        }),
        HistoryOperationKind::ManageData { name } => json!({
            "name": name.as_deref(),
        }),
        HistoryOperationKind::BumpSequence { bump_to } => json!({
            "bump_to": bump_to.as_deref(),
        }),
        HistoryOperationKind::InvokeHostFunction
        | HistoryOperationKind::LiquidityPoolDeposit
        | HistoryOperationKind::LiquidityPoolWithdraw
        | HistoryOperationKind::SetOptions
        | HistoryOperationKind::Other { .. } => json!({}),
    }
}

fn history_asset_json(asset: &HistoryAsset) -> Value {
    match asset {
        HistoryAsset::Native => json!({
            "kind": "native",
            "identity": "XLM",
        }),
        HistoryAsset::Issued { code, issuer } => json!({
            "kind": "issued",
            "identity": asset.identity(),
            "code": code,
            "issuer": issuer,
        }),
    }
}

fn history_trust_asset_json(asset: &HistoryTrustAsset) -> Value {
    match asset {
        HistoryTrustAsset::Classic(asset) => history_asset_json(asset),
        HistoryTrustAsset::LiquidityPool { liquidity_pool_id } => json!({
            "kind": "liquidity_pool_share",
            "liquidity_pool_id": liquidity_pool_id,
        }),
        HistoryTrustAsset::Unknown => json!({
            "kind": "unknown",
        }),
    }
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

#[cfg(test)]
mod tests {
    use fresnica_client::{AccountThresholds, LedgerSignerCondition, WeightedLedgerSigner};

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

    #[test]
    fn balance_json_is_provider_neutral_and_typed() {
        let balance = AssetBalance {
            asset: BalanceAsset::Issued {
                code: "USD".to_owned(),
                issuer: "GISSUER".to_owned(),
            },
            balance: "7".to_owned(),
            selling_liabilities: "1.25".to_owned(),
            buying_liabilities: "0.5".to_owned(),
        };

        let value = balance_json(&balance);

        assert_eq!(value["asset"]["kind"], json!("issued"));
        assert_eq!(value["asset"]["identity"], json!("USD:GISSUER"));
        assert_eq!(value["asset"]["code"], json!("USD"));
        assert_eq!(value["balance"], json!("7"));
        assert_eq!(value["selling_liabilities"], json!("1.25"));
    }

    #[test]
    fn history_json_is_provider_neutral_and_keeps_full_asset_identity() {
        let operation = HistoryOperation {
            operation_id: Some("101".to_owned()),
            paging_token: Some("101".to_owned()),
            transaction_hash: Some("abc123".to_owned()),
            created_at: Some("2026-09-06T12:00:00Z".to_owned()),
            source_account: Some("GSOURCE".to_owned()),
            kind: HistoryOperationKind::Payment {
                from: Some("GSOURCE".to_owned()),
                to: Some("GDESTINATION".to_owned()),
                amount: Some("1.2500000".to_owned()),
                asset: Some(HistoryAsset::Issued {
                    code: "USD".to_owned(),
                    issuer: "GISSUER".to_owned(),
                }),
            },
        };

        let value = history_operation_json(&operation);
        assert_eq!(value["type"], json!("payment"));
        assert_eq!(value["operation_id"], json!("101"));
        assert_eq!(value["details"]["asset"]["kind"], json!("issued"));
        assert_eq!(value["details"]["asset"]["identity"], json!("USD:GISSUER"));
        assert_eq!(value["details"]["asset"]["issuer"], json!("GISSUER"));
        assert!(value.get("_links").is_none());
    }
}
