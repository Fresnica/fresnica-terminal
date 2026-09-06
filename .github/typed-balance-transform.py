from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file = Path(path)
    text = file.read_text()
    if new in text:
        return
    if old not in text:
        raise SystemExit(f"expected source fragment not found in {path}")
    file.write_text(text.replace(old, new, 1))


replace_once(
    "crates/cli/src/read_commands.rs",
    """use fresnica_client::{
    balance_asset_label, operation_summary, AccountState, FresnicaClient, LedgerSignerKind,
};
""",
    """use fresnica_client::{
    operation_summary, AccountState, AssetBalance, BalanceAsset, FresnicaClient, LedgerSignerKind,
};
""",
)

replace_once(
    "crates/cli/src/read_commands.rs",
    '''pub fn command_balance(client: &FresnicaClient, arguments: &[String]) -> Result<(), String> {
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
''',
    '''pub fn command_balance(client: &FresnicaClient, arguments: &[String]) -> Result<(), String> {
    let options = parse_output_options(arguments, "fresnica balance [--wallet NAME] [--json]")?;
    crate::diagnostics::stage("balance: fetch ledger balances");
    let snapshot = client.balances(options.wallet.as_deref())?;

    if options.json {
        let balances = snapshot.balances.iter().map(balance_json).collect::<Vec<_>>();
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
''',
)

replace_once(
    "crates/cli/src/read_commands.rs",
    '''    #[test]
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
''',
    '''    #[test]
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
}
''',
)

replace_once(
    "crates/tui/src/app.rs",
    '''use fresnica_client::{
    AssetCatalogEntry, BalanceSnapshot, FresnicaClient, HistorySnapshot, OpenOffer, WalletRecord,
    MAX_ASSET_CATALOG_LIMIT,
};
''',
    '''use fresnica_client::{
    AssetBalance, AssetCatalogEntry, BalanceSnapshot, FresnicaClient, HistorySnapshot, OpenOffer,
    WalletRecord, MAX_ASSET_CATALOG_LIMIT,
};
''',
)
replace_once(
    "crates/tui/src/app.rs",
    "    pub(super) balances: Vec<Value>,\n",
    "    pub(super) balances: Vec<AssetBalance>,\n",
)

replace_once(
    "crates/tui/src/render.rs",
    '''use fresnica_client::{
    balance_asset_label, operation_summary, AuthorizationScope, AuthorizationThreshold,
    ClassicOperationKind, LedgerAuthorizationSnapshot, LedgerSignerAvailability, LedgerSignerKind,
    OfferReviewDetails, PreparedOffer, PreparedPayment, PreparedTrustline,
};
''',
    '''use fresnica_client::{
    operation_summary, AuthorizationScope, AuthorizationThreshold, ClassicOperationKind,
    LedgerAuthorizationSnapshot, LedgerSignerAvailability, LedgerSignerKind, OfferReviewDetails,
    PreparedOffer, PreparedPayment, PreparedTrustline,
};
''',
)
replace_once(
    "crates/tui/src/render.rs",
    '''        let rows = self.balances.iter().map(|balance| {
            Row::new([
                balance_asset_label(balance),
                text(balance, "balance").unwrap_or("0").to_owned(),
                text(balance, "selling_liabilities")
                    .unwrap_or("0")
                    .to_owned(),
                text(balance, "buying_liabilities")
                    .unwrap_or("0")
                    .to_owned(),
            ])
        });
''',
    '''        let rows = self.balances.iter().map(|balance| {
            Row::new([
                balance.asset.identity(),
                balance.balance.clone(),
                balance.selling_liabilities.clone(),
                balance.buying_liabilities.clone(),
            ])
        });
''',
)
