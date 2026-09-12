use fresnica_client::{FresnicaClient, OfferRequest, OfferReview, OfferReviewDetails, OfferSide};
use serde_json::{json, Value};

use crate::transaction_flow::{
    authorization_json, confirm_submission, render_authorization_review,
    submit_with_classic_signers,
};

pub fn command_dex_write(client: &FresnicaClient, arguments: &[String]) -> Result<(), String> {
    crate::diagnostics::stage("DEX write: parse request");
    let request = WriteRequest::parse(arguments)?;
    crate::diagnostics::stage("DEX write: prepare reviewed transaction");
    let prepared = client.prepare_offer(&request.service)?;
    crate::diagnostics::stage("DEX write: review prepared transaction");
    if !request.json {
        render_offer_review(&prepared.review);
        if !request.yes && !confirm_submission()? {
            println!("Transaction cancelled.");
            return Ok(());
        }
    }
    crate::diagnostics::stage("DEX write: sign and submit");
    let submission = submit_with_classic_signers(client, |passcode, system_auth, providers| {
        client.submit_offer_with_providers(&prepared, passcode, system_auth, providers)
    })?;
    if request.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "kind": "dex_offer_submission",
                "review": offer_review_json(&prepared.review),
                "submission": {
                    "hash": submission.hash.as_str(),
                    "ledger": submission.ledger,
                },
            }))
            .map_err(|error| format!("unable to encode DEX offer JSON: {error}"))?
        );
    } else {
        println!("Submitted: {}", submission.hash);
        if let Some(ledger) = submission.ledger {
            println!("Ledger:    {ledger}");
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WriteRequest {
    service: OfferRequest,
    yes: bool,
    json: bool,
}

impl WriteRequest {
    fn parse(arguments: &[String]) -> Result<Self, String> {
        let Some(command) = arguments.first().map(String::as_str) else {
            return Err(usage().to_owned());
        };
        match command {
            "buy" | "sell" => {
                if arguments.len() < 5 {
                    return Err(usage().to_owned());
                }
                let side = if command == "buy" {
                    OfferSide::Buy
                } else {
                    OfferSide::Sell
                };
                let (wallet, allow_trustline, yes, json) = parse_options(&arguments[5..], true)?;
                reject_machine_without_approval(json, yes)?;
                Ok(Self {
                    service: OfferRequest::Create {
                        side,
                        base: arguments[1].clone(),
                        counter: arguments[2].clone(),
                        amount: arguments[3].clone(),
                        price: arguments[4].clone(),
                        wallet,
                        allow_trustline,
                    },
                    yes,
                    json,
                })
            }
            "update" => {
                if arguments.len() < 6 {
                    return Err(usage().to_owned());
                }
                let offer_id = parse_offer_id(&arguments[1])?;
                let (wallet, allow_trustline, yes, json) = parse_options(&arguments[6..], false)?;
                reject_machine_without_approval(json, yes)?;
                if allow_trustline {
                    return Err(usage().to_owned());
                }
                Ok(Self {
                    service: OfferRequest::Update {
                        offer_id,
                        base: arguments[2].clone(),
                        counter: arguments[3].clone(),
                        amount: arguments[4].clone(),
                        price: arguments[5].clone(),
                        wallet,
                    },
                    yes,
                    json,
                })
            }
            "cancel" => {
                if arguments.len() < 2 {
                    return Err(usage().to_owned());
                }
                let offer_id = parse_offer_id(&arguments[1])?;
                let (wallet, allow_trustline, yes, json) = parse_options(&arguments[2..], false)?;
                reject_machine_without_approval(json, yes)?;
                if allow_trustline {
                    return Err(usage().to_owned());
                }
                Ok(Self {
                    service: OfferRequest::Cancel { wallet, offer_id },
                    yes,
                    json,
                })
            }
            _ => Err(usage().to_owned()),
        }
    }
}

fn parse_options(
    arguments: &[String],
    allow_trustline_option: bool,
) -> Result<(Option<String>, bool, bool, bool), String> {
    let mut wallet = None;
    let mut allow_trustline = false;
    let mut yes = false;
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--wallet" => {
                index += 1;
                wallet = Some(
                    arguments
                        .get(index)
                        .ok_or_else(|| usage().to_owned())?
                        .clone(),
                );
                index += 1;
            }
            "--allow-trustline" if allow_trustline_option => {
                allow_trustline = true;
                index += 1;
            }
            "-y" | "--yes" => {
                yes = true;
                index += 1;
            }
            "--json" => {
                json = true;
                index += 1;
            }
            _ => return Err(usage().to_owned()),
        }
    }
    Ok((wallet, allow_trustline, yes, json))
}

fn reject_machine_without_approval(json: bool, yes: bool) -> Result<(), String> {
    if json && !yes {
        Err(
            "dex write --json requires -y so stdout remains one machine-readable JSON document"
                .to_owned(),
        )
    } else {
        Ok(())
    }
}

fn parse_offer_id(value: &str) -> Result<i64, String> {
    value
        .parse::<i64>()
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| "offer id must be a positive integer".to_owned())
}

fn offer_review_json(review: &OfferReview) -> Value {
    let details = match &review.details {
        OfferReviewDetails::Trade {
            side,
            base,
            counter,
            amount,
            price,
            requested_price,
            price_n,
            price_d,
            total,
            trustline_asset,
            trustline_limit,
        } => json!({
            "kind": "trade",
            "side": side.label().to_ascii_lowercase(),
            "base": base.as_str(),
            "counter": counter.as_str(),
            "amount": amount.as_str(),
            "price": price.as_str(),
            "requested_price": requested_price.as_deref(),
            "price_n": price_n,
            "price_d": price_d,
            "total": total.as_str(),
            "trustline_asset": trustline_asset.as_deref(),
            "trustline_limit": trustline_limit.as_deref(),
        }),
        OfferReviewDetails::Cancel { selling, buying } => json!({
            "kind": "cancel",
            "selling": selling.as_str(),
            "buying": buying.as_str(),
        }),
    };
    json!({
        "action": review.action.label(),
        "operation": review.operation.label(),
        "wallet": {
            "name": review.wallet_name.as_str(),
            "address": review.source.as_str(),
        },
        "offer_id": review.offer_id,
        "details": details,
        "fee_xlm": review.fee_xlm.as_str(),
        "network": review.network.as_str(),
        "transaction_timeout_seconds": review.transaction_timeout_seconds,
        "authorization": authorization_json(&review.ledger_authorization),
    })
}

fn render_offer_review(review: &OfferReview) {
    println!("Review transaction");
    println!(
        "Operation: {} ({})",
        review.operation.label(),
        review.action.label()
    );
    println!("Wallet:    {} ({})", review.wallet_name, review.source);
    if let Some(offer_id) = review.offer_id {
        println!("Offer:     #{offer_id}");
    }
    match &review.details {
        OfferReviewDetails::Trade {
            side,
            base,
            counter,
            amount,
            price,
            requested_price,
            price_n,
            price_d,
            total,
            trustline_asset,
            trustline_limit,
        } => {
            println!("Side:      {}", side.label());
            println!("Pair:      {base} / {counter}");
            println!("Amount:    {amount} {base}");
            println!("Price:     {price} {counter}/{base}");
            println!("Encoded:   {price_n}/{price_d}");
            if let Some(requested) = requested_price {
                println!("Requested: {requested} {counter}/{base}");
            }
            println!("Total:     {total} {counter}");
            if let Some(asset) = trustline_asset {
                let limit = trustline_limit
                    .as_deref()
                    .map(|value| format!("; limit {value}"))
                    .unwrap_or_default();
                println!("Trustline: + {asset}{limit} (explicitly approved)");
            }
        }
        OfferReviewDetails::Cancel { selling, buying } => {
            println!("Selling:   {selling}");
            println!("Buying:    {buying}");
        }
    }
    println!("Fee:       {} XLM", review.fee_xlm);
    println!("Network:   {}", review.network);
    println!("Lifetime:  {} seconds", review.transaction_timeout_seconds);
    render_authorization_review(&review.ledger_authorization);
}

fn usage() -> &'static str {
    "usage:\n  fresnica dex buy BASE COUNTER AMOUNT PRICE [--wallet NAME] [--allow-trustline] [-y] [--json]\n  fresnica dex sell BASE COUNTER AMOUNT PRICE [--wallet NAME] [--allow-trustline] [-y] [--json]\n  fresnica dex update OFFER_ID BASE COUNTER AMOUNT PRICE [--wallet NAME] [-y] [--json]\n  fresnica dex cancel OFFER_ID [--wallet NAME] [-y] [--json]"
}

#[cfg(test)]
mod tests {
    use fresnica_client::{LedgerAuthorizationSnapshot, OfferAction, OfferOperation};

    use super::*;

    const ISSUER: &str = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF";

    #[test]
    fn parser_matches_python_cli_write_shape() {
        let args = [
            "buy",
            "XLM",
            &format!("USD:{ISSUER}"),
            "10",
            "2.5",
            "--allow-trustline",
            "--wallet",
            "alpha",
            "-y",
        ]
        .map(str::to_owned);
        let request = WriteRequest::parse(&args).unwrap();
        assert!(request.yes);
        assert!(matches!(
            request.service,
            OfferRequest::Create {
                side: OfferSide::Buy,
                allow_trustline: true,
                wallet: Some(ref wallet),
                ..
            } if wallet == "alpha"
        ));
    }

    #[test]
    fn update_and_cancel_require_positive_offer_ids() {
        assert!(WriteRequest::parse(&[
            "update".to_owned(),
            "0".to_owned(),
            "XLM".to_owned(),
            format!("USD:{ISSUER}"),
            "1".to_owned(),
            "1".to_owned(),
        ])
        .is_err());
        assert!(WriteRequest::parse(&["cancel".to_owned(), "-1".to_owned()]).is_err());
    }

    #[test]
    fn machine_dex_write_requires_explicit_noninteractive_approval() {
        let pair = format!("USD:{ISSUER}");
        let without_yes = ["buy", "XLM", pair.as_str(), "1", "2", "--json"].map(str::to_owned);
        assert!(WriteRequest::parse(&without_yes)
            .unwrap_err()
            .contains("requires -y"));

        let with_yes = ["buy", "XLM", pair.as_str(), "1", "2", "-y", "--json"].map(str::to_owned);
        let request = WriteRequest::parse(&with_yes).unwrap();
        assert!(request.yes);
        assert!(request.json);
    }

    #[test]
    fn offer_review_json_preserves_semantic_review() {
        let review = OfferReview {
            action: OfferAction::Create,
            operation: OfferOperation::ManageBuyOffer,
            wallet_name: "alpha".to_owned(),
            source: "GSOURCE".to_owned(),
            offer_id: None,
            fee_xlm: "0.00001".to_owned(),
            network: "testnet".to_owned(),
            transaction_timeout_seconds: 180,
            details: OfferReviewDetails::Trade {
                side: OfferSide::Buy,
                base: "XLM".to_owned(),
                counter: format!("USD:{ISSUER}"),
                amount: "10".to_owned(),
                price: "2.5".to_owned(),
                requested_price: Some("2.5".to_owned()),
                price_n: 5,
                price_d: 2,
                total: "25".to_owned(),
                trustline_asset: Some(format!("USD:{ISSUER}")),
                trustline_limit: Some("100".to_owned()),
            },
            ledger_authorization: LedgerAuthorizationSnapshot {
                transaction_hash: "abc123".to_owned(),
                accounts: Vec::new(),
                extra_signers: Vec::new(),
                satisfied: false,
                locally_satisfiable: true,
            },
        };
        let value = offer_review_json(&review);
        assert_eq!(value["action"], "create");
        assert_eq!(value["operation"], "ManageBuyOffer");
        assert_eq!(value["details"]["side"], "buy");
        assert_eq!(value["details"]["price_n"], 5);
        assert_eq!(value["details"]["trustline_limit"], "100");
        assert_eq!(value["authorization"]["status"], "local_signing_ready");
        assert_eq!(value["authorization"]["transaction_hash"], "abc123");
    }
}
