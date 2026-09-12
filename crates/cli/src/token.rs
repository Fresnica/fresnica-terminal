use fresnica_client::{resolve_token, FresnicaClient, ResolvedToken, ResolvedTokenSource};
use serde_json::{json, Value};
use tokio::runtime::Builder;

use crate::contract;
use crate::contract_store::{looks_like_contract_id, ContractStore};

const USAGE: &str = "usage: fresnica token TOKEN [--json]\n\nTOKEN may be XLM, native, CODE:GISSUER, a C... contract id, or a saved contract name.";

pub fn command_token(client: &FresnicaClient, arguments: &[String]) -> Result<(), String> {
    let options = TokenInspectOptions::parse(arguments)?;
    let resolved = resolve_terminal_token(client, &options.reference)?;
    let runtime = Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("unable to initialize Soroban runtime: {error}"))?;

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

    if options.json {
        let value = json!({
            "kind": "token_inspect",
            "network": client.network(),
            "reference": options.reference,
            "resolution": resolution_json(&resolved),
            "contract": contract::interface_json(&interface),
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&value)
                .map_err(|error| format!("unable to encode token JSON: {error}"))?
        );
        return Ok(());
    }

    println!("Token: {}", options.reference);
    match &resolved.token.source {
        ResolvedTokenSource::StellarAsset { asset } => println!("Source: Stellar asset {asset}"),
        ResolvedTokenSource::Contract => match &resolved.saved_name {
            Some(name) => println!("Source: Saved contract {name}"),
            None => println!("Source: Contract"),
        },
    }
    println!("Contract: {}", resolved.token.contract_id);
    contract::render_capability_evidence(&interface.capabilities);
    println!("Functions: {}", interface.functions.len());
    Ok(())
}

struct TokenInspectOptions {
    reference: String,
    json: bool,
}

impl TokenInspectOptions {
    fn parse(arguments: &[String]) -> Result<Self, String> {
        if arguments.is_empty()
            || arguments
                .iter()
                .any(|arg| matches!(arg.as_str(), "-h" | "--help"))
        {
            return Err(USAGE.to_owned());
        }
        let mut reference = None;
        let mut json = false;
        for argument in arguments {
            match argument.as_str() {
                "--json" => json = true,
                value if value.starts_with('-') => {
                    return Err(format!("unknown token option: {value}\n{USAGE}"));
                }
                value if reference.is_none() => reference = Some(value.to_owned()),
                _ => return Err(USAGE.to_owned()),
            }
        }
        Ok(Self {
            reference: reference.ok_or_else(|| USAGE.to_owned())?,
            json,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inspect_parser_accepts_json_before_or_after_reference() {
        for args in [
            vec!["--json".to_owned(), "XLM".to_owned()],
            vec!["XLM".to_owned(), "--json".to_owned()],
        ] {
            let parsed = TokenInspectOptions::parse(&args).unwrap();
            assert_eq!(parsed.reference, "XLM");
            assert!(parsed.json);
        }
    }

    #[test]
    fn inspect_parser_rejects_extra_or_unknown_arguments() {
        assert!(TokenInspectOptions::parse(&["XLM".into(), "extra".into()]).is_err());
        assert!(TokenInspectOptions::parse(&["XLM".into(), "--wat".into()]).is_err());
    }
}
