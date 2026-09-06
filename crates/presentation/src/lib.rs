use fresnica_client::{HistoryAsset, HistoryOperation, HistoryOperationKind, HistoryTrustAsset};

pub fn history_operation_summary(operation: &HistoryOperation, account: &str) -> String {
    match &operation.kind {
        HistoryOperationKind::Payment {
            from,
            to,
            amount: value,
            asset,
        } => {
            let amount = amount(value.as_deref());
            let asset = asset_label(asset.as_ref());
            let source = from.as_deref().unwrap_or("?");
            let destination = to.as_deref().unwrap_or("?");
            if destination == account {
                format!("Received {amount} {asset} from {}", short_address(source))
            } else if source == account {
                format!("Sent {amount} {asset} to {}", short_address(destination))
            } else {
                format!(
                    "{amount} {asset}: {} -> {}",
                    short_address(source),
                    short_address(destination)
                )
            }
        }
        HistoryOperationKind::CreateAccount {
            account: created,
            starting_balance,
            ..
        } => {
            let created = created.as_deref().unwrap_or("?");
            let starting = amount(starting_balance.as_deref());
            if created == account {
                format!("Account created with {starting} XLM")
            } else {
                format!("Created {} with {starting} XLM", short_address(created))
            }
        }
        HistoryOperationKind::ManageSellOffer {
            offer_id,
            amount: offer_amount,
            selling_asset,
            buying_asset,
            price,
        } => offer_summary(
            "SELL",
            false,
            offer_id.as_deref(),
            offer_amount.as_deref(),
            selling_asset.as_ref(),
            buying_asset.as_ref(),
            price.as_deref(),
        ),
        HistoryOperationKind::CreatePassiveSellOffer {
            offer_id,
            amount: offer_amount,
            selling_asset,
            buying_asset,
            price,
        } => offer_summary(
            "SELL",
            true,
            offer_id.as_deref(),
            offer_amount.as_deref(),
            selling_asset.as_ref(),
            buying_asset.as_ref(),
            price.as_deref(),
        ),
        HistoryOperationKind::ManageBuyOffer {
            offer_id,
            amount: offer_amount,
            selling_asset,
            buying_asset,
            price,
        } => {
            let offer_id = offer_id.as_deref().unwrap_or("0");
            let offer_amount = amount(offer_amount.as_deref());
            if offer_amount == "0" {
                return format!("Cancelled offer #{offer_id}");
            }
            let selling = asset_label(selling_asset.as_ref());
            let buying = asset_label(buying_asset.as_ref());
            let price = amount(price.as_deref());
            let verb = if offer_id == "0" {
                "Placed".to_owned()
            } else {
                format!("Updated #{offer_id}")
            };
            format!("{verb} BUY {offer_amount} {buying} @ {price} {selling}/{buying}")
        }
        HistoryOperationKind::ChangeTrust { asset, limit } => {
            let asset = trust_asset_label(asset);
            let limit = amount(limit.as_deref());
            if limit == "0" {
                format!("Removed trustline for {asset}")
            } else {
                format!("Set trustline for {asset} · limit {limit}")
            }
        }
        HistoryOperationKind::InvokeHostFunction => "Contract call".to_owned(),
        HistoryOperationKind::LiquidityPoolDeposit => "Added liquidity".to_owned(),
        HistoryOperationKind::LiquidityPoolWithdraw => "Removed liquidity".to_owned(),
        HistoryOperationKind::AccountMerge { into } => format!(
            "Merged account into {}",
            short_address(into.as_deref().unwrap_or("?"))
        ),
        HistoryOperationKind::ManageData { name } => format!(
            "Updated account data: {}",
            name.as_deref().unwrap_or("data entry")
        ),
        HistoryOperationKind::SetOptions => "Updated account settings".to_owned(),
        HistoryOperationKind::BumpSequence { bump_to } => format!(
            "Bumped sequence to {}",
            bump_to.as_deref().unwrap_or("?")
        ),
        HistoryOperationKind::Other { operation_type } => operation_type.replace('_', " "),
    }
}

fn offer_summary(
    side: &str,
    passive: bool,
    offer_id: Option<&str>,
    offer_amount: Option<&str>,
    selling_asset: Option<&HistoryAsset>,
    buying_asset: Option<&HistoryAsset>,
    price: Option<&str>,
) -> String {
    let offer_id = offer_id.unwrap_or("0");
    let offer_amount = amount(offer_amount);
    if offer_amount == "0" {
        return format!("Cancelled offer #{offer_id}");
    }
    let selling = asset_label(selling_asset);
    let buying = asset_label(buying_asset);
    let price = amount(price);
    if passive {
        format!("Placed passive {side} {offer_amount} {selling} @ {price} {buying}/{selling}")
    } else {
        let verb = if offer_id == "0" {
            "Placed".to_owned()
        } else {
            format!("Updated #{offer_id}")
        };
        format!("{verb} {side} {offer_amount} {selling} @ {price} {buying}/{selling}")
    }
}

fn asset_label(asset: Option<&HistoryAsset>) -> String {
    match asset {
        Some(HistoryAsset::Native) => "XLM".to_owned(),
        Some(HistoryAsset::Issued { code, issuer }) => {
            format!("{code}:{}", short_address(issuer))
        }
        None => "asset".to_owned(),
    }
}

fn trust_asset_label(asset: &HistoryTrustAsset) -> String {
    match asset {
        HistoryTrustAsset::Classic(asset) => asset_label(Some(asset)),
        HistoryTrustAsset::LiquidityPool { liquidity_pool_id } => {
            format!("liquidity pool {}", short_id(liquidity_pool_id))
        }
        HistoryTrustAsset::Unknown => "asset".to_owned(),
    }
}

fn amount(value: Option<&str>) -> String {
    value
        .map(clean_decimal)
        .unwrap_or_else(|| "?".to_owned())
}

fn clean_decimal(value: &str) -> String {
    if !value.contains('.') {
        return value.to_owned();
    }
    let trimmed = value.trim_end_matches('0').trim_end_matches('.');
    if trimmed.is_empty() || trimmed == "-" {
        "0".to_owned()
    } else {
        trimmed.to_owned()
    }
}

fn short_address(value: &str) -> String {
    if value.len() <= 16 {
        return value.to_owned();
    }
    format!("{}...{}", &value[..6], &value[value.len() - 6..])
}

fn short_id(value: &str) -> String {
    if value.len() <= 12 {
        value.to_owned()
    } else {
        format!("{}...", &value[..8])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn operation(kind: HistoryOperationKind) -> HistoryOperation {
        HistoryOperation {
            operation_id: None,
            paging_token: None,
            transaction_hash: None,
            created_at: None,
            source_account: None,
            kind,
        }
    }

    #[test]
    fn payment_summary_preserves_terminal_wording() {
        let op = operation(HistoryOperationKind::Payment {
            from: Some("GSOURCE12345678901234567890".to_owned()),
            to: Some("GACCOUNT".to_owned()),
            amount: Some("1.2500000".to_owned()),
            asset: Some(HistoryAsset::Native),
        });
        assert_eq!(
            history_operation_summary(&op, "GACCOUNT"),
            "Received 1.25 XLM from GSOURC...567890"
        );
    }

    #[test]
    fn issued_asset_is_shortened_only_for_human_presentation() {
        let op = operation(HistoryOperationKind::ManageSellOffer {
            offer_id: Some("0".to_owned()),
            amount: Some("5.0000000".to_owned()),
            selling_asset: Some(HistoryAsset::Issued {
                code: "USD".to_owned(),
                issuer: "GISSUER12345678901234567890".to_owned(),
            }),
            buying_asset: Some(HistoryAsset::Native),
            price: Some("2.0000000".to_owned()),
        });
        assert_eq!(
            history_operation_summary(&op, "GACCOUNT"),
            "Placed SELL 5 USD: GISSU?"
                .replace("USD: GISSU?", "USD:GISSUE...567890 @ 2 XLM/USD:GISSUE...567890")
        );
    }

    #[test]
    fn trustline_and_unknown_operation_keep_existing_semantics() {
        let trust = operation(HistoryOperationKind::ChangeTrust {
            asset: HistoryTrustAsset::LiquidityPool {
                liquidity_pool_id: "abcdef0123456789".to_owned(),
            },
            limit: Some("0.0000000".to_owned()),
        });
        assert_eq!(
            history_operation_summary(&trust, "GACCOUNT"),
            "Removed trustline for liquidity pool abcdef01..."
        );

        let unknown = operation(HistoryOperationKind::Other {
            operation_type: "future_protocol_operation".to_owned(),
        });
        assert_eq!(
            history_operation_summary(&unknown, "GACCOUNT"),
            "future protocol operation"
        );
    }
}
