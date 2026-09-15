use fresnica_client::{
    FresnicaClient, PreparedTrustline, TrustlineAction, TrustlineAuthorization, TrustlineRequest,
    TrustlineReview,
};
use serde_json::{json, Value};

use crate::transaction_flow::{
    authorization_json, confirm_submission, render_authorization_review,
    submit_with_classic_signers,
};

pub fn command_trust(client: &FresnicaClient, arguments: &[String]) -> Result<(), String> {
    crate::diagnostics::stage("trustline: parse request");
    let request = TrustRequest::parse(arguments)?;
    crate::diagnostics::stage("trustline: prepare reviewed transaction");
    let prepared = client.prepare_trustline(&request.service_request())?;
    review_and_submit(client, &prepared, request.yes(), request.json())
}

fn review_and_submit(
    client: &FresnicaClient,
    prepared: &PreparedTrustline,
    yes: bool,
    json_output: bool,
) -> Result<(), String> {
    crate::diagnostics::stage("trustline: review prepared transaction");
    if !json_output {
        render_review(&prepared.review);
        if !yes && !confirm_submission()? {
            println!("Transaction cancelled.");
            return Ok(());
        }
    }

    crate::diagnostics::stage("trustline: sign and submit");
    let submission = submit_with_classic_signers(client, |passcode, system_auth, providers| {
        client.submit_trustline_with_providers(prepared, passcode, system_auth, providers)
    })?;
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "kind": "trustline_submission",
                "review": trustline_review_json(&prepared.review),
                "submission": {
                    "hash": submission.hash.as_str(),
                    "ledger": submission.ledger,
                },
            }))
            .map_err(|error| format!("unable to encode trustline JSON: {error}"))?
        );
    } else {
        println!("Submitted: {}", submission.hash);
        if let Some(ledger) = submission.ledger {
            println!("Ledger:    {ledger}");
        }
    }
    Ok(())
}

fn trustline_review_json(review: &TrustlineReview) -> Value {
    json!({
        "operation": review.operation.label(),
        "wallet": {
            "name": review.wallet_name.as_str(),
            "address": review.source.as_str(),
        },
        "asset": review.asset.as_str(),
        "limit": review.limit.as_deref(),
        "authorization_state": review.authorization.map(trustline_authorization_machine_label),
        "clawback_enabled": review.clawback_enabled,
        "fee_xlm": review.fee_xlm.as_str(),
        "network": review.network.as_str(),
        "transaction_timeout_seconds": review.transaction_timeout_seconds,
        "authorization": authorization_json(&review.ledger_authorization),
    })
}

fn trustline_authorization_machine_label(authorization: TrustlineAuthorization) -> &'static str {
    match authorization {
        TrustlineAuthorization::Full => "full",
        TrustlineAuthorization::MaintainLiabilities => "maintain_liabilities",
        TrustlineAuthorization::Unauthorized => "unauthorized",
    }
}

fn render_review(review: &TrustlineReview) {
    println!("Review transaction");
    println!("Operation: ChangeTrust ({})", review.operation.label());
    println!("Wallet:    {} ({})", review.wallet_name, review.source);
    println!("Asset:     {}", review.asset);
    if let Some(limit) = &review.limit {
        println!("Limit:     {limit}");
    }
    if let Some(authorization) = review.authorization {
        println!("Auth:      {}", authorization.label());
    }
    if let Some(clawback_enabled) = review.clawback_enabled {
        println!(
            "Clawback:  {}",
            if clawback_enabled {
                "enabled"
            } else {
                "disabled"
            }
        );
    }
    println!("Fee:       {} XLM", review.fee_xlm);
    println!("Network:   {}", review.network);
    println!("Lifetime:  {} seconds", review.transaction_timeout_seconds);
    render_authorization_review(&review.ledger_authorization);
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TrustRequest {
    Add {
        asset: String,
        limit: Option<String>,
        wallet: Option<String>,
        yes: bool,
        json: bool,
    },
    Limit {
        asset: String,
        limit: String,
        wallet: Option<String>,
        yes: bool,
        json: bool,
    },
    Remove {
        asset: String,
        wallet: Option<String>,
        yes: bool,
        json: bool,
    },
}

impl TrustRequest {
    fn parse(arguments: &[String]) -> Result<Self, String> {
        let Some(command) = arguments.first().map(String::as_str) else {
            return Err(usage().to_owned());
        };
        match command {
            "add" => {
                let asset = arguments.get(1).ok_or_else(|| usage().to_owned())?.clone();
                let (wallet, yes, limit, json) = parse_options(&arguments[2..], true)?;
                reject_machine_without_approval(json, yes)?;
                Ok(Self::Add {
                    asset,
                    limit,
                    wallet,
                    yes,
                    json,
                })
            }
            "limit" => {
                let asset = arguments.get(1).ok_or_else(|| usage().to_owned())?.clone();
                let limit = arguments.get(2).ok_or_else(|| usage().to_owned())?.clone();
                let (wallet, yes, extra, json) = parse_options(&arguments[3..], false)?;
                if extra.is_some() {
                    return Err(usage().to_owned());
                }
                reject_machine_without_approval(json, yes)?;
                Ok(Self::Limit {
                    asset,
                    limit,
                    wallet,
                    yes,
                    json,
                })
            }
            "remove" => {
                let asset = arguments.get(1).ok_or_else(|| usage().to_owned())?.clone();
                let (wallet, yes, extra, json) = parse_options(&arguments[2..], false)?;
                if extra.is_some() {
                    return Err(usage().to_owned());
                }
                reject_machine_without_approval(json, yes)?;
                Ok(Self::Remove {
                    asset,
                    wallet,
                    yes,
                    json,
                })
            }
            _ => Err(usage().to_owned()),
        }
    }

    fn service_request(&self) -> TrustlineRequest {
        match self {
            Self::Add {
                asset,
                limit,
                wallet,
                ..
            } => TrustlineRequest {
                wallet: wallet.clone(),
                asset: asset.clone(),
                action: TrustlineAction::Add {
                    limit: limit.clone(),
                },
            },
            Self::Limit {
                asset,
                limit,
                wallet,
                ..
            } => TrustlineRequest {
                wallet: wallet.clone(),
                asset: asset.clone(),
                action: TrustlineAction::SetLimit {
                    limit: limit.clone(),
                },
            },
            Self::Remove { asset, wallet, .. } => TrustlineRequest {
                wallet: wallet.clone(),
                asset: asset.clone(),
                action: TrustlineAction::Remove,
            },
        }
    }

    fn yes(&self) -> bool {
        match self {
            Self::Add { yes, .. } | Self::Limit { yes, .. } | Self::Remove { yes, .. } => *yes,
        }
    }

    fn json(&self) -> bool {
        match self {
            Self::Add { json, .. } | Self::Limit { json, .. } | Self::Remove { json, .. } => *json,
        }
    }
}

fn parse_options(
    arguments: &[String],
    allow_limit: bool,
) -> Result<(Option<String>, bool, Option<String>, bool), String> {
    let mut wallet = None;
    let mut yes = false;
    let mut limit = None;
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
            "--limit" if allow_limit => {
                index += 1;
                limit = Some(
                    arguments
                        .get(index)
                        .ok_or_else(|| usage().to_owned())?
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
            _ => return Err(usage().to_owned()),
        }
    }
    Ok((wallet, yes, limit, json))
}

fn reject_machine_without_approval(json: bool, yes: bool) -> Result<(), String> {
    if json && !yes {
        Err(
            "trust --json requires -y so stdout remains one machine-readable JSON document"
                .to_owned(),
        )
    } else {
        Ok(())
    }
}

fn usage() -> &'static str {
    "usage: fresnica trust add CODE:GISSUER [--limit VALUE] [--wallet NAME] [-y] [--json]\n       fresnica trust limit CODE:GISSUER LIMIT [--wallet NAME] [-y] [--json]\n       fresnica trust remove CODE:GISSUER [--wallet NAME] [-y] [--json]"
}

#[cfg(test)]
mod tests {
    use fresnica_client::{LedgerAuthorizationSnapshot, TrustlineOperation};

    use super::*;

    const ISSUER: &str = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF";

    #[test]
    fn add_parser_accepts_limit_wallet_and_yes() {
        let args = [
            "add",
            &format!("USD:{ISSUER}"),
            "--limit",
            "1000",
            "--wallet",
            "alpha",
            "-y",
        ]
        .map(str::to_owned);
        let request = TrustRequest::parse(&args).unwrap();
        assert!(request.yes());
        assert_eq!(
            request.service_request(),
            TrustlineRequest {
                wallet: Some("alpha".to_owned()),
                asset: format!("USD:{ISSUER}"),
                action: TrustlineAction::Add {
                    limit: Some("1000".to_owned())
                }
            }
        );
    }

    #[test]
    fn machine_trust_requires_explicit_noninteractive_approval() {
        let asset = format!("USD:{ISSUER}");
        for args in [
            vec!["add".to_owned(), asset.clone(), "--json".to_owned()],
            vec![
                "limit".to_owned(),
                asset.clone(),
                "100".to_owned(),
                "--json".to_owned(),
            ],
            vec!["remove".to_owned(), asset.clone(), "--json".to_owned()],
        ] {
            assert!(TrustRequest::parse(&args)
                .unwrap_err()
                .contains("requires -y"));
        }

        let args = ["add", asset.as_str(), "-y", "--json"].map(str::to_owned);
        let request = TrustRequest::parse(&args).unwrap();
        assert!(request.yes());
        assert!(request.json());
    }

    #[test]
    fn trustline_review_json_preserves_semantic_review() {
        let review = TrustlineReview {
            operation: TrustlineOperation::Add,
            wallet_name: "alpha".to_owned(),
            source: "GSOURCE".to_owned(),
            asset: format!("USD:{ISSUER}"),
            limit: Some("1000".to_owned()),
            authorization: Some(TrustlineAuthorization::MaintainLiabilities),
            clawback_enabled: Some(true),
            fee_xlm: "0.00001".to_owned(),
            network: "testnet".to_owned(),
            transaction_timeout_seconds: 180,
            ledger_authorization: LedgerAuthorizationSnapshot {
                transaction_hash: "abc123".to_owned(),
                accounts: Vec::new(),
                extra_signers: Vec::new(),
                satisfied: false,
                locally_satisfiable: true,
            },
        };
        let value = trustline_review_json(&review);
        assert_eq!(value["operation"], "add");
        assert_eq!(value["wallet"]["name"], "alpha");
        assert_eq!(value["authorization_state"], "maintain_liabilities");
        assert_eq!(value["clawback_enabled"], true);
        assert_eq!(value["authorization"]["status"], "local_signing_ready");
        assert_eq!(value["authorization"]["transaction_hash"], "abc123");
    }
}
