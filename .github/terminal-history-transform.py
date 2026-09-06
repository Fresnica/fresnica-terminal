from pathlib import Path

OLD_REV = "d36e2e2e55a0452702bda4b57389067c4765d7ad"
NEW_REV = "65a9da804ca16849e320d24365e6e839e62cf5d8"


def replace_once(path: str, old: str, new: str) -> None:
    file = Path(path)
    text = file.read_text()
    assert old in text, f"missing expected text in {path}"
    file.write_text(text.replace(old, new, 1))


root = Path("Cargo.toml")
text = root.read_text()
old_members = 'members = ["crates/cli", "crates/tui"]'
new_members = 'members = ["crates/cli", "crates/presentation", "crates/tui"]'
assert old_members in text
assert text.count(OLD_REV) == 1
root.write_text(text.replace(old_members, new_members, 1).replace(OLD_REV, NEW_REV, 1))

rev = Path("FRESNICA_REV")
assert rev.read_text().strip() == OLD_REV
rev.write_text(NEW_REV + "\n")

lock = Path("Cargo.lock")
text = lock.read_text()
assert text.count(OLD_REV) == 3, text.count(OLD_REV)
lock.write_text(text.replace(OLD_REV, NEW_REV))

for manifest in ["crates/cli/Cargo.toml", "crates/tui/Cargo.toml"]:
    replace_once(
        manifest,
        "fresnica-client.workspace = true\n",
        'fresnica-client.workspace = true\nfresnica-terminal-presentation = { path = "../presentation" }\n',
    )

read_commands = Path("crates/cli/src/read_commands.rs")
text = read_commands.read_text()
old_import = """use fresnica_client::{
    operation_summary, AccountState, AssetBalance, BalanceAsset, FresnicaClient, LedgerSignerKind,
};
use serde_json::{json, Value};
"""
new_import = """use fresnica_client::{
    AccountState, AssetBalance, BalanceAsset, FresnicaClient, HistoryAsset, HistoryOperation,
    HistoryOperationKind, HistoryTrustAsset, LedgerSignerKind,
};
use fresnica_terminal_presentation::history_operation_summary;
use serde_json::{json, Value};
"""
assert old_import in text
text = text.replace(old_import, new_import, 1)
text = text.replace(
    'crate::diagnostics::stage("history: fetch Horizon operations");',
    'crate::diagnostics::stage("history: fetch account activity");',
    1,
)
old_json = """    if options.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&snapshot.operations)
                .map_err(|error| format!("unable to encode history data: {error}"))?
        );
        return Ok(());
    }
"""
new_json = """    if options.json {
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
"""
assert old_json in text
text = text.replace(old_json, new_json, 1)
old_loop = """    for operation in &snapshot.operations {
        let created_at = text(operation, "created_at").unwrap_or("?");
        let operation_type = text(operation, "type").unwrap_or("unknown");
        println!(
            "{:<20} {:<28} {}",
            created_at,
            operation_type,
            operation_summary(operation, &snapshot.wallet.address)
        );
    }
"""
new_loop = """    for operation in &snapshot.operations {
        let created_at = operation.created_at.as_deref().unwrap_or("?");
        println!(
            "{:<20} {:<28} {}",
            created_at,
            operation.operation_type(),
            history_operation_summary(operation, &snapshot.wallet.address)
        );
    }
"""
assert old_loop in text
text = text.replace(old_loop, new_loop, 1)
helper_marker = "\nstruct OutputOptions {"
assert helper_marker in text
helpers = r'''

fn history_operation_json(operation: &HistoryOperation) -> Value {
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
'''
text = text.replace(helper_marker, helpers + helper_marker, 1)
old_text_helper = """
fn text<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}
"""
assert old_text_helper in text
text = text.replace(old_text_helper, "", 1)
insert = r'''

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
'''
head, sep, tail = text.rpartition("\n}")
assert sep
text = head + insert + sep + tail
read_commands.write_text(text)

app = Path("crates/tui/src/app.rs")
text = app.read_text()
old_import = """use fresnica_client::{
    AssetBalance, AssetCatalogEntry, BalanceSnapshot, FresnicaClient, HistorySnapshot, OpenOffer,
    WalletRecord, MAX_ASSET_CATALOG_LIMIT,
};
use ratatui::crossterm::event::KeyCode;
use serde_json::Value;
"""
new_import = """use fresnica_client::{
    AssetBalance, AssetCatalogEntry, BalanceSnapshot, FresnicaClient, HistoryOperation,
    HistorySnapshot, OpenOffer, WalletRecord, MAX_ASSET_CATALOG_LIMIT,
};
use ratatui::crossterm::event::KeyCode;
"""
assert old_import in text
text = text.replace(old_import, new_import, 1)
text = text.replace("pub(super) operations: Vec<Value>,", "pub(super) operations: Vec<HistoryOperation>,", 1)
text = text.replace('"Updated from Horizon".to_owned()', '"Account data refreshed".to_owned()', 1)
app.write_text(text)

render = Path("crates/tui/src/render.rs")
text = render.read_text()
old_import = """use fresnica_client::{
    operation_summary, AuthorizationScope, AuthorizationThreshold, ClassicOperationKind,
    LedgerAuthorizationSnapshot, LedgerSignerAvailability, LedgerSignerKind, OfferReviewDetails,
    PreparedOffer, PreparedPayment, PreparedTrustline,
};
"""
new_import = """use fresnica_client::{
    AuthorizationScope, AuthorizationThreshold, ClassicOperationKind, LedgerAuthorizationSnapshot,
    LedgerSignerAvailability, LedgerSignerKind, OfferReviewDetails, PreparedOffer, PreparedPayment,
    PreparedTrustline,
};
use fresnica_terminal_presentation::history_operation_summary;
"""
assert old_import in text
text = text.replace(old_import, new_import, 1)
text = text.replace("use serde_json::Value;\n", "", 1)
old_activity = """            self.operations
                .iter()
                .map(|operation| {
                    let created_at = text(operation, "created_at").unwrap_or("?");
                    let operation_type = text(operation, "type").unwrap_or("unknown");
                    ListItem::new(Line::from(format!(
                        "{created_at}  {operation_type}  {}",
                        operation_summary(operation, address)
                    )))
                })
                .collect()
"""
new_activity = """            self.operations
                .iter()
                .map(|operation| {
                    let created_at = operation.created_at.as_deref().unwrap_or("?");
                    ListItem::new(Line::from(format!(
                        "{created_at}  {}  {}",
                        operation.operation_type(),
                        history_operation_summary(operation, address)
                    )))
                })
                .collect()
"""
assert old_activity in text
text = text.replace(old_activity, new_activity, 1)
old_text_helper = """
fn text<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}
"""
assert old_text_helper in text
text = text.replace(old_text_helper, "", 1)
render.write_text(text)

presentation = Path("crates/presentation/src/lib.rs")
text = presentation.read_text()
old_assertion = '''            "Placed SELL 5 USD: GISSU?"\n                .replace("USD: GISSU?", "USD:GISSUE...567890 @ 2 XLM/USD:GISSUE...567890")\n'''
new_assertion = '''            "Placed SELL 5 USD:GISSUE...567890 @ 2 XLM/USD:GISSUE...567890"\n'''
assert old_assertion in text
presentation.write_text(text.replace(old_assertion, new_assertion, 1))
