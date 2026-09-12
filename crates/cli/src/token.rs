use fresnica_client::{
    resolve_token, FresnicaClient, ResolvedToken, ResolvedTokenSource, TokenBalanceRequest,
    TokenTransferRequest,
};
use serde_json::{json, Value};
use tokio::runtime::Builder;

use crate::contract;
use crate::contract_store::{looks_like_contract_id, ContractStore};

const USAGE: &str = "usage:\n  fresnica token TOKEN [--json]\n  fresnica token TOKEN balance OWNER [--json]\n  fresnica token TOKEN transfer AMOUNT TO [--wallet NAME] [-y] [--json]\n\nTOKEN may be XLM, native, CODE:GISSUER, a C... contract id, or a saved contract name. OWNER/TO may be a raw Address or a Fresnica wallet/contact/saved-contract name. Transfer AMOUNT is a positive decimal token amount; FROM is always the selected Fresnica wallet.";

pub fn command_token(client: &FresnicaClient, arguments: &[String]) -> Result<(), String> {
    let options = TokenOptions::parse(arguments)?;
    let resolved = resolve_terminal_token(client, &options.reference)?;
    let runtime = Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("unable to initialize Soroban runtime: {error}"))?;

    match options.action {
        TokenAction::Inspect => inspect_token(
            client,
            &runtime,
            &options.reference,
            &resolved,
            options.json,
        ),
        TokenAction::Balance { owner } => balance_token(
            client,
            &runtime,
            &options.reference,
            &resolved,
            &owner,
            options.json,
        ),
        TokenAction::Transfer(transfer) => transfer_token(
            client,
            &runtime,
            &options.reference,
            &resolved,
            &transfer,
            options.json,
        ),
    }
}

fn inspect_token(
    client: &FresnicaClient,
    runtime: &tokio::runtime::Runtime,
    reference: &str,
    resolved: &TerminalResolvedToken,
    json_output: bool,
) -> Result<(), String> {
    crate::diagnostics::stage("token: inspect resolved contract interface");
    let interface = runtime.block_on(client.contract_interface(&resolved.token.contract_id))?;
    if resolved.saved_name.is_some() {
        ContractStore::for_home(client.storage().home(), client.network()).record_observation(
            &interface.contract_id,
            &interface.executable,
            &interface.metadata,
            &interface.capabilities,
        )?;
    }

    if json_output {
        let value = json!({
            "kind": "token_inspect",
            "network": client.network(),
            "reference": reference,
            "resolution": resolution_json(resolved),
            "contract": contract::interface_json(&interface),
        });
        print_json(&value)?;
        return Ok(());
    }

    println!("Token: {reference}");
    render_resolution(resolved);
    contract::render_capability_evidence(&interface.capabilities);
    println!("Functions: {}", interface.functions.len());
    Ok(())
}

fn balance_token(
    client: &FresnicaClient,
    runtime: &tokio::runtime::Runtime,
    reference: &str,
    resolved: &TerminalResolvedToken,
    owner: &str,
    json_output: bool,
) -> Result<(), String> {
    crate::diagnostics::stage("token: read SEP-41 balance");
    let mut request = TokenBalanceRequest::new(resolved.token.clone(), owner);
    add_balance_address_names(client, &mut request)?;
    let balance = runtime.block_on(client.token_balance(request))?;
    verify_saved_observation(
        client,
        resolved,
        &balance.contract.contract_id,
        &balance.contract.executable,
        &balance.contract.metadata,
        &balance.contract.capabilities,
    )?;

    if json_output {
        print_json(&json!({
            "kind": "token_balance",
            "network": client.network(),
            "reference": reference,
            "resolution": resolution_json(resolved),
            "owner": balance.owner,
            "balance": {
                "raw": balance.amount.raw.to_string(),
                "decimals": balance.amount.decimals,
                "amount": balance.amount.amount,
            },
            "capabilities": contract::capabilities_json(&balance.contract.capabilities),
            "simulation_ledger": balance.contract.simulation_ledger,
        }))?;
        return Ok(());
    }

    println!("Token: {reference}");
    render_resolution(resolved);
    println!("Owner: {}", compact_json(&balance.owner));
    match balance.amount.amount.as_deref() {
        Some(amount) => println!("Balance: {amount}"),
        None => println!(
            "Balance: {} raw units ({} decimals)",
            balance.amount.raw, balance.amount.decimals
        ),
    }
    println!("Raw: {}", balance.amount.raw);
    println!("Decimals: {}", balance.amount.decimals);
    Ok(())
}

fn transfer_token(
    client: &FresnicaClient,
    runtime: &tokio::runtime::Runtime,
    reference: &str,
    resolved: &TerminalResolvedToken,
    transfer: &TokenTransferAction,
    json_output: bool,
) -> Result<(), String> {
    crate::diagnostics::stage("token: prepare SEP-41 transfer");
    let mut request =
        TokenTransferRequest::new(resolved.token.clone(), &transfer.amount, &transfer.to);
    request.wallet = transfer.wallet.clone();
    add_transfer_address_names(client, &mut request)?;
    let mut prepared = runtime.block_on(client.prepare_token_transfer(request))?;
    verify_saved_observation(
        client,
        resolved,
        &prepared.contract.review.contract_id,
        &prepared.contract.review.executable,
        &prepared.contract.review.metadata,
        &prepared.contract.review.capabilities,
    )?;

    crate::diagnostics::stage("token: review transfer");
    if !json_output {
        println!("Token transfer");
        println!("Token: {reference}");
        render_resolution(resolved);
        println!(
            "From: {} ({})",
            prepared.wallet_name, prepared.wallet_address
        );
        println!("To: {}", compact_json(&prepared.destination));
        match prepared.amount.amount.as_deref() {
            Some(amount) => println!("Amount: {amount}"),
            None => println!(
                "Amount: {} raw units ({} decimals)",
                prepared.amount.raw, prepared.amount.decimals
            ),
        }
        contract::render_review(&prepared.contract.review, reference);
        if !transfer.yes && !crate::transaction_flow::confirm_submission()? {
            println!("Transaction cancelled.");
            return Ok(());
        }
    }

    let submission =
        contract::authorize_sign_submit_contract(client, runtime, &mut prepared.contract)?;
    if json_output {
        print_json(&json!({
            "kind": "token_transfer",
            "network": client.network(),
            "reference": reference,
            "resolution": resolution_json(resolved),
            "wallet": {
                "name": prepared.wallet_name,
                "address": prepared.wallet_address,
            },
            "to": prepared.destination,
            "amount": {
                "input": prepared.amount_input,
                "raw": prepared.amount.raw.to_string(),
                "decimals": prepared.amount.decimals,
                "amount": prepared.amount.amount,
            },
            "capabilities": contract::capabilities_json(&prepared.contract.review.capabilities),
            "review": contract::review_json(&prepared.contract.review),
            "submission": {
                "hash": submission.hash.as_str(),
                "ledger": submission.ledger,
            },
        }))?;
    } else {
        println!("Submitted: {}", submission.hash);
        if let Some(ledger) = submission.ledger {
            println!("Ledger:    {ledger}");
        }
    }
    Ok(())
}

fn add_balance_address_names(
    client: &FresnicaClient,
    request: &mut TokenBalanceRequest,
) -> Result<(), String> {
    let entries = contract::local_address_name_entries(client, |name| {
        request.references_argument_value(name)
    })?;
    for (name, address) in entries {
        request.add_address_name(&name, &address)?;
    }
    Ok(())
}

fn add_transfer_address_names(
    client: &FresnicaClient,
    request: &mut TokenTransferRequest,
) -> Result<(), String> {
    let entries = contract::local_address_name_entries(client, |name| {
        request.references_argument_value(name)
    })?;
    for (name, address) in entries {
        request.add_address_name(&name, &address)?;
    }
    Ok(())
}

fn verify_saved_observation(
    client: &FresnicaClient,
    resolved: &TerminalResolvedToken,
    contract_id: &str,
    executable: &fresnica_client::ContractExecutableObservation,
    metadata: &[fresnica_client::ContractMetadataEntry],
    capabilities: &fresnica_client::ContractCapabilities,
) -> Result<(), String> {
    if resolved.saved_name.is_none() {
        return Ok(());
    }
    ContractStore::for_home(client.storage().home(), client.network()).verify_observation(
        contract_id,
        executable,
        metadata,
        capabilities,
    )
}

struct TokenOptions {
    reference: String,
    json: bool,
    action: TokenAction,
}

enum TokenAction {
    Inspect,
    Balance { owner: String },
    Transfer(TokenTransferAction),
}

struct TokenTransferAction {
    amount: String,
    to: String,
    wallet: Option<String>,
    yes: bool,
}

impl TokenOptions {
    fn parse(arguments: &[String]) -> Result<Self, String> {
        if arguments.is_empty()
            || arguments
                .iter()
                .any(|arg| matches!(arg.as_str(), "-h" | "--help"))
        {
            return Err(USAGE.to_owned());
        }
        let mut positional = Vec::new();
        let mut wallet = None;
        let mut yes = false;
        let mut json = false;
        let mut index = 0;
        while index < arguments.len() {
            match arguments[index].as_str() {
                "--json" => json = true,
                "-y" => yes = true,
                "--wallet" => {
                    index += 1;
                    let value = arguments.get(index).ok_or_else(|| USAGE.to_owned())?;
                    if wallet.replace(value.clone()).is_some() {
                        return Err(format!("--wallet was provided more than once\n{USAGE}"));
                    }
                }
                value if value.starts_with('-') => {
                    return Err(format!("unknown token option: {value}\n{USAGE}"));
                }
                value => positional.push(value.to_owned()),
            }
            index += 1;
        }
        let reference = positional
            .first()
            .cloned()
            .ok_or_else(|| USAGE.to_owned())?;
        let is_transfer = matches!(
            positional.as_slice(),
            [_, action, _, _] if action == "transfer"
        );
        if !is_transfer && (wallet.is_some() || yes) {
            return Err(format!(
                "--wallet and -y are only valid for token transfer\n{USAGE}"
            ));
        }
        let action = match positional.as_slice() {
            [_] => TokenAction::Inspect,
            [_, action, owner] if action == "balance" => TokenAction::Balance {
                owner: owner.clone(),
            },
            [_, action, amount, to] if action == "transfer" => {
                if json && !yes {
                    return Err(
                        "token transfer --json requires -y so stdout remains one machine-readable JSON document"
                            .to_owned(),
                    );
                }
                TokenAction::Transfer(TokenTransferAction {
                    amount: amount.clone(),
                    to: to.clone(),
                    wallet,
                    yes,
                })
            }
            _ => return Err(USAGE.to_owned()),
        };
        Ok(Self {
            reference,
            json,
            action,
        })
    }
}

struct TerminalResolvedToken {
    token: ResolvedToken,
    saved_name: Option<String>,
}

fn resolve_terminal_token(
    client: &FresnicaClient,
    reference: &str,
) -> Result<TerminalResolvedToken, String> {
    if reference.eq_ignore_ascii_case("XLM")
        || reference.eq_ignore_ascii_case("native")
        || reference.contains(':')
        || looks_like_contract_id(reference)
    {
        return Ok(TerminalResolvedToken {
            token: resolve_token(reference, client.network())?,
            saved_name: None,
        });
    }

    let store = ContractStore::for_home(client.storage().home(), client.network());
    if let Some(saved) = store.find(reference)? {
        return Ok(TerminalResolvedToken {
            token: resolve_token(&saved.contract_id, client.network())?,
            saved_name: Some(saved.user.name),
        });
    }

    Err(format!(
        "Unknown token {reference:?} on {}. Use XLM, native, CODE:GISSUER, a C... contract id, or a saved contract name.",
        client.network()
    ))
}

fn resolution_json(resolved: &TerminalResolvedToken) -> Value {
    match &resolved.token.source {
        ResolvedTokenSource::StellarAsset { asset } => json!({
            "contract_id": resolved.token.contract_id.as_str(),
            "source": "stellar_asset",
            "asset": asset.as_str(),
            "saved_name": Value::Null,
        }),
        ResolvedTokenSource::Contract => json!({
            "contract_id": resolved.token.contract_id.as_str(),
            "source": "contract",
            "asset": Value::Null,
            "saved_name": resolved.saved_name.as_deref(),
        }),
    }
}

fn render_resolution(resolved: &TerminalResolvedToken) {
    match &resolved.token.source {
        ResolvedTokenSource::StellarAsset { asset } => println!("Source: Stellar asset {asset}"),
        ResolvedTokenSource::Contract => match &resolved.saved_name {
            Some(name) => println!("Source: Saved contract {name}"),
            None => println!("Source: Contract"),
        },
    }
    println!("Contract: {}", resolved.token.contract_id);
}

fn compact_json(value: &Value) -> String {
    serde_json::to_string(value).expect("serde_json::Value serialization cannot fail")
}

fn print_json(value: &Value) -> Result<(), String> {
    println!(
        "{}",
        serde_json::to_string_pretty(value)
            .map_err(|error| format!("unable to encode token JSON: {error}"))?
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inspect_parser_accepts_json_before_or_after_reference() {
        for args in [
            vec!["--json".to_owned(), "XLM".to_owned()],
            vec!["XLM".to_owned(), "--json".to_owned()],
        ] {
            let parsed = TokenOptions::parse(&args).unwrap();
            assert_eq!(parsed.reference, "XLM");
            assert!(parsed.json);
        }
    }

    #[test]
    fn inspect_parser_rejects_extra_or_unknown_arguments() {
        assert!(TokenOptions::parse(&["XLM".into(), "extra".into()]).is_err());
        assert!(TokenOptions::parse(&["XLM".into(), "--wat".into()]).is_err());
    }

    #[test]
    fn balance_parser_accepts_wallet_native_resource_first_shape() {
        let parsed = TokenOptions::parse(&[
            "XLM".into(),
            "balance".into(),
            "main".into(),
            "--json".into(),
        ])
        .unwrap();
        assert_eq!(parsed.reference, "XLM");
        assert!(parsed.json);
        let TokenAction::Balance { owner } = parsed.action else {
            panic!("expected balance action");
        };
        assert_eq!(owner, "main");
    }

    #[test]
    fn transfer_parser_binds_wallet_options_to_transfer_only() {
        let parsed = TokenOptions::parse(&[
            "XLM".into(),
            "transfer".into(),
            "1.25".into(),
            "alice".into(),
            "--wallet".into(),
            "main".into(),
            "-y".into(),
            "--json".into(),
        ])
        .unwrap();
        assert!(parsed.json);
        let TokenAction::Transfer(transfer) = parsed.action else {
            panic!("expected transfer action");
        };
        assert_eq!(transfer.wallet.as_deref(), Some("main"));
        assert!(transfer.yes);
        assert_eq!(transfer.amount, "1.25");
        assert_eq!(transfer.to, "alice");

        assert!(TokenOptions::parse(&["XLM".into(), "--wallet".into(), "main".into()]).is_err());
        assert!(TokenOptions::parse(
            &["XLM".into(), "balance".into(), "main".into(), "-y".into(),]
        )
        .is_err());
        assert!(TokenOptions::parse(&[
            "XLM".into(),
            "transfer".into(),
            "1".into(),
            "alice".into(),
            "--json".into(),
        ])
        .err()
        .expect("machine transfer without -y must fail")
        .contains("requires -y"));
    }
}
