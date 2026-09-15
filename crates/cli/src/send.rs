use fresnica_client::{
    FresnicaClient, PaymentMemo, PaymentRequest, PaymentReview, PreparedPayment, WalletRecord,
};
use serde_json::{json, Value};

use crate::transaction_flow::{
    authorization_json, confirm_submission, render_authorization_review,
    submit_with_classic_signers,
};

pub fn command_send(client: &FresnicaClient, arguments: &[String]) -> Result<(), String> {
    crate::diagnostics::stage("payment: parse request");
    let request = SendRequest::parse(arguments)?;
    crate::diagnostics::stage("payment: prepare reviewed transaction");
    let prepared = client.prepare_payment(&PaymentRequest {
        wallet: request.wallet.clone(),
        amount: request.amount,
        asset: request.asset,
        destination: request.destination,
        memo: request.memo,
    })?;
    review_and_submit_prepared(client, &prepared, request.yes, request.json)
}

pub(crate) fn review_and_submit_payment(
    client: &FresnicaClient,
    record: &WalletRecord,
    amount_text: &str,
    asset_text: &str,
    destination_address: &str,
    memo: PaymentMemo,
    yes: bool,
) -> Result<(), String> {
    crate::diagnostics::stage("payment: prepare anchor payment");
    let prepared = client.prepare_payment_to_address(
        record,
        amount_text,
        asset_text,
        destination_address,
        None,
        memo,
    )?;
    review_and_submit_prepared(client, &prepared, yes, false)
}

fn review_and_submit_prepared(
    client: &FresnicaClient,
    prepared: &PreparedPayment,
    yes: bool,
    json_output: bool,
) -> Result<(), String> {
    crate::diagnostics::stage("payment: review prepared transaction");
    if !json_output {
        render_review(&prepared.review);
        if !yes && !confirm_submission()? {
            println!("Transaction cancelled.");
            return Ok(());
        }
    }

    crate::diagnostics::stage("payment: sign and submit");
    let submission = submit_with_classic_signers(client, |passcode, system_auth, providers| {
        client.submit_payment_with_providers(prepared, passcode, system_auth, providers)
    })?;
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "kind": "payment_submission",
                "review": payment_review_json(&prepared.review),
                "submission": {
                    "hash": submission.hash.as_str(),
                    "ledger": submission.ledger,
                },
            }))
            .map_err(|error| format!("unable to encode payment JSON: {error}"))?
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
struct SendRequest {
    amount: String,
    asset: String,
    destination: String,
    wallet: Option<String>,
    memo: Option<String>,
    yes: bool,
    json: bool,
}

impl SendRequest {
    fn parse(arguments: &[String]) -> Result<Self, String> {
        const USAGE: &str =
            "usage: fresnica send AMOUNT ASSET to DESTINATION [--wallet NAME] [--memo TEXT] [-y] [--json]";
        if arguments.len() < 4 || arguments[2].to_lowercase() != "to" {
            return Err(USAGE.to_owned());
        }
        let mut wallet = None;
        let mut memo = None;
        let mut yes = false;
        let mut json = false;
        let mut index = 4;
        while index < arguments.len() {
            match arguments[index].as_str() {
                "--wallet" => {
                    index += 1;
                    wallet = Some(
                        arguments
                            .get(index)
                            .ok_or_else(|| USAGE.to_owned())?
                            .clone(),
                    );
                    index += 1;
                }
                "--memo" => {
                    index += 1;
                    memo = Some(
                        arguments
                            .get(index)
                            .ok_or_else(|| USAGE.to_owned())?
                            .clone(),
                    );
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
                _ => return Err(USAGE.to_owned()),
            }
        }
        if json && !yes {
            return Err(
                "send --json requires -y so stdout remains one machine-readable JSON document"
                    .to_owned(),
            );
        }
        Ok(Self {
            amount: arguments[0].clone(),
            asset: arguments[1].clone(),
            destination: arguments[3].clone(),
            wallet,
            memo,
            yes,
            json,
        })
    }
}

fn payment_review_json(review: &PaymentReview) -> Value {
    json!({
        "operation": review.operation.label(),
        "wallet": {
            "name": review.wallet_name.as_str(),
            "address": review.source.as_str(),
        },
        "destination": {
            "address": review.destination.as_str(),
            "contact_name": review.contact_name.as_deref(),
        },
        "amount": review.amount.as_str(),
        "asset": review.asset.as_str(),
        "fee_xlm": review.fee_xlm.as_str(),
        "network": review.network.as_str(),
        "transaction_timeout_seconds": review.transaction_timeout_seconds,
        "memo": review.memo.as_ref().map(|memo| json!({
            "type": memo.memo_type.as_str(),
            "value": memo.value.as_str(),
        })),
        "authorization": authorization_json(&review.ledger_authorization),
    })
}

fn render_review(review: &PaymentReview) {
    println!("Review transaction");
    println!("Operation: {}", review.operation.label());
    println!("From:      {} ({})", review.wallet_name, review.source);
    if let Some(name) = &review.contact_name {
        println!("To:        {name} ({})", review.destination);
    } else {
        println!("To:        {}", review.destination);
    }
    println!("Amount:    {} {}", review.amount, review.asset);
    println!("Fee:       {} XLM", review.fee_xlm);
    println!("Network:   {}", review.network);
    println!("Lifetime:  {} seconds", review.transaction_timeout_seconds);
    if let Some(memo) = &review.memo {
        println!("Memo:      {} ({})", memo.value, memo.memo_type);
    }
    render_authorization_review(&review.ledger_authorization);
}

#[cfg(test)]
mod tests {
    use fresnica_client::{LedgerAuthorizationSnapshot, PaymentOperation};

    use super::*;

    const DESTINATION: &str = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF";

    #[test]
    fn send_parser_matches_python_cli_shape() {
        let args = [
            "1.5",
            "XLM",
            "to",
            DESTINATION,
            "--memo",
            "hello",
            "--wallet",
            "alpha",
            "-y",
        ]
        .map(str::to_owned);
        let request = SendRequest::parse(&args).unwrap();
        assert_eq!(request.amount, "1.5");
        assert_eq!(request.asset, "XLM");
        assert_eq!(request.destination, DESTINATION);
        assert_eq!(request.memo.as_deref(), Some("hello"));
        assert_eq!(request.wallet.as_deref(), Some("alpha"));
        assert!(request.yes);
        assert!(!request.json);
    }

    #[test]
    fn machine_send_requires_explicit_noninteractive_approval() {
        let without_yes = ["1", "XLM", "to", DESTINATION, "--json"].map(str::to_owned);
        assert!(SendRequest::parse(&without_yes)
            .unwrap_err()
            .contains("requires -y"));

        let with_yes = ["1", "XLM", "to", DESTINATION, "-y", "--json"].map(str::to_owned);
        let request = SendRequest::parse(&with_yes).unwrap();
        assert!(request.yes);
        assert!(request.json);
    }

    #[test]
    fn payment_review_json_preserves_semantic_review() {
        let review = PaymentReview {
            operation: PaymentOperation::Payment,
            wallet_name: "alpha".to_owned(),
            source: "GSOURCE".to_owned(),
            destination: DESTINATION.to_owned(),
            contact_name: Some("alice".to_owned()),
            amount: "1.5".to_owned(),
            asset: "XLM".to_owned(),
            fee_xlm: "0.00001".to_owned(),
            network: "testnet".to_owned(),
            transaction_timeout_seconds: 180,
            memo: None,
            ledger_authorization: LedgerAuthorizationSnapshot {
                transaction_hash: "abc123".to_owned(),
                accounts: Vec::new(),
                extra_signers: Vec::new(),
                satisfied: false,
                locally_satisfiable: true,
            },
        };
        let value = payment_review_json(&review);
        assert_eq!(value["operation"], "Payment");
        assert_eq!(value["wallet"]["name"], "alpha");
        assert_eq!(value["destination"]["contact_name"], "alice");
        assert_eq!(value["authorization"]["status"], "local_signing_ready");
        assert_eq!(value["authorization"]["transaction_hash"], "abc123");
    }
}
