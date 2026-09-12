use fresnica_client::{
    resolve_token, ContractArgumentInput, ContractInvokePreparation, ContractInvokeRequest,
    ContractReadResult, FresnicaClient, ResolvedToken, ResolvedTokenSource,
};
use serde_json::{json, Value};
use tokio::runtime::Builder;

use crate::contract;
use crate::contract_store::{looks_like_contract_id, ContractStore};

const USAGE: &str = "usage:\n  fresnica token TOKEN [--json]\n  fresnica token TOKEN balance OWNER [--json]\n\nTOKEN may be XLM, native, CODE:GISSUER, a C... contract id, or a saved contract name. OWNER may be a raw Address or a Fresnica wallet/contact/saved-contract name.";

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
    let balance = read_token_function(
        client,
        runtime,
        resolved,
        "balance",
        vec![ContractArgumentInput::new("id", owner)],
    )?;
    ensure_current_sep41(&balance)?;
    let raw = parse_i128_output(balance.output.as_ref(), "balance")?;

    let decimals = if balance.capabilities.sep41.native_sac {
        7
    } else {
        crate::diagnostics::stage("token: read SEP-41 decimals");
        let decimals = read_token_function(client, runtime, resolved, "decimals", Vec::new())?;
        ensure_current_sep41(&decimals)?;
        parse_u32_output(decimals.output.as_ref(), "decimals")?
    };
    let amount = format_fixed_point(raw, decimals);
    let resolved_owner = balance
        .arguments
        .first()
        .map(|argument| argument.value.clone())
        .unwrap_or(Value::Null);

    if json_output {
        print_json(&json!({
            "kind": "token_balance",
            "network": client.network(),
            "reference": reference,
            "resolution": resolution_json(resolved),
            "owner": resolved_owner,
            "balance": {
                "raw": raw.to_string(),
                "decimals": decimals,
                "amount": amount,
            },
            "capabilities": contract::capabilities_json(&balance.capabilities),
            "simulation_ledger": balance.simulation_ledger,
        }))?;
        return Ok(());
    }

    println!("Token: {reference}");
    render_resolution(resolved);
    println!("Owner: {}", compact_json(&resolved_owner));
    match amount {
        Some(amount) => println!("Balance: {amount}"),
        None => println!("Balance: {raw} raw units ({decimals} decimals)"),
    }
    println!("Raw: {raw}");
    println!("Decimals: {decimals}");
    Ok(())
}

fn read_token_function(
    client: &FresnicaClient,
    runtime: &tokio::runtime::Runtime,
    resolved: &TerminalResolvedToken,
    function: &str,
    arguments: Vec<ContractArgumentInput>,
) -> Result<ContractReadResult, String> {
    let mut request = ContractInvokeRequest::new(&resolved.token.contract_id, function, arguments);
    contract::add_local_address_names(client, &mut request)?;
    let outcome = runtime.block_on(client.prepare_contract_invoke_outcome(request))?;
    let ContractInvokePreparation::ReadOnly(result) = outcome else {
        return Err(format!(
            "SEP-41 {function} unexpectedly requires a write transaction; refusing token read"
        ));
    };
    if resolved.saved_name.is_some() {
        ContractStore::for_home(client.storage().home(), client.network()).verify_observation(
            &result.contract_id,
            &result.executable,
            &result.metadata,
            &result.capabilities,
        )?;
    }
    Ok(result)
}

fn ensure_current_sep41(result: &ContractReadResult) -> Result<(), String> {
    if result.capabilities.sep41.current_interface_compatible {
        return Ok(());
    }
    Err(format!(
        "contract {} is not compatible with the current SEP-41 interface",
        result.contract_id
    ))
}

struct TokenOptions {
    reference: String,
    json: bool,
    action: TokenAction,
}

enum TokenAction {
    Inspect,
    Balance { owner: String },
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
        let mut json = false;
        for argument in arguments {
            match argument.as_str() {
                "--json" => json = true,
                value if value.starts_with('-') => {
                    return Err(format!("unknown token option: {value}\n{USAGE}"));
                }
                value => positional.push(value.to_owned()),
            }
        }
        let reference = positional
            .first()
            .cloned()
            .ok_or_else(|| USAGE.to_owned())?;
        let action = match positional.as_slice() {
            [_] => TokenAction::Inspect,
            [_, action, owner] if action == "balance" => TokenAction::Balance {
                owner: owner.clone(),
            },
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

fn parse_i128_output(value: Option<&Value>, label: &str) -> Result<i128, String> {
    parse_integer_text(value, label)?
        .parse()
        .map_err(|_| format!("SEP-41 {label} returned a value outside i128 range"))
}

fn parse_u32_output(value: Option<&Value>, label: &str) -> Result<u32, String> {
    parse_integer_text(value, label)?
        .parse()
        .map_err(|_| format!("SEP-41 {label} returned a value outside u32 range"))
}

fn parse_integer_text<'a>(
    value: Option<&'a Value>,
    label: &str,
) -> Result<std::borrow::Cow<'a, str>, String> {
    match value {
        Some(Value::String(value)) => Ok(std::borrow::Cow::Borrowed(value)),
        Some(Value::Number(value)) => Ok(std::borrow::Cow::Owned(value.to_string())),
        Some(other) => Err(format!(
            "SEP-41 {label} returned a non-integer value: {other}"
        )),
        None => Err(format!("SEP-41 {label} returned no value")),
    }
}

fn format_fixed_point(raw: i128, decimals: u32) -> Option<String> {
    if decimals > 38 {
        return None;
    }
    if decimals == 0 {
        return Some(raw.to_string());
    }
    let text = raw.to_string();
    let (negative, digits) = match text.strip_prefix('-') {
        Some(digits) => (true, digits),
        None => (false, text.as_str()),
    };
    let decimals = decimals as usize;
    let mut value = if digits.len() > decimals {
        let split = digits.len() - decimals;
        format!("{}.{}", &digits[..split], &digits[split..])
    } else {
        format!("0.{}{}", "0".repeat(decimals - digits.len()), digits)
    };
    while value.ends_with('0') {
        value.pop();
    }
    if value.ends_with('.') {
        value.pop();
    }
    if value.is_empty() || value == "-0" {
        value = "0".to_owned();
    }
    if negative && value != "0" {
        value.insert(0, '-');
    }
    Some(value)
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
    fn fixed_point_format_is_exact_and_bounded() {
        assert_eq!(
            format_fixed_point(12_345_678, 7).as_deref(),
            Some("1.2345678")
        );
        assert_eq!(format_fixed_point(10_000_000, 7).as_deref(), Some("1"));
        assert_eq!(format_fixed_point(-1, 7).as_deref(), Some("-0.0000001"));
        assert_eq!(format_fixed_point(0, 7).as_deref(), Some("0"));
        assert!(format_fixed_point(1, 39).is_none());
    }
}
