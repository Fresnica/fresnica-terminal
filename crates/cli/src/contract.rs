use fresnica_client::{
    ContactStore, ContractArgumentInput, ContractCapabilities, ContractExecutableObservation,
    ContractFunction, ContractInterface, ContractInvokePreparation, ContractInvokeRequest,
    ContractInvokeReview, ContractMetadataEntry, ContractReadResult, FresnicaClient,
    PreparedContractInvoke, TransactionSubmission, WalletStorage, SEP41_INTERFACE_VERSION,
};
use serde_json::{json, Value};
use tokio::runtime::Builder;

use crate::contract_store::{
    looks_like_contract_id, ContractDerivedCapabilities, ContractStore, SavedContract,
};
use crate::transaction_flow::{
    confirm_submission, submit_with_classic_signers, with_software_signer_authorization,
};

const USAGE: &str = "usage:\n  fresnica contract TARGET [--wallet NAME] [-y] [--json] [FUNCTION [--NAME VALUE]...]\n  fresnica contract list [--json]\n  fresnica contract add NAME C... [--json]\n  fresnica contract remove NAME [--json]\n\nlegacy:\n  fresnica contract invoke C... [--wallet NAME] [-y] [--json] -- FUNCTION [--NAME VALUE]...";
const LEGACY_USAGE: &str = "usage: fresnica contract invoke C... [--wallet NAME] [-y] [--json] -- FUNCTION [--NAME VALUE]...\n       fresnica contract invoke C... [--json] -- --help\n       fresnica contract invoke C... [--json] -- FUNCTION --help";
const SAVED_CONTRACT_USAGE: &str =
    "usage: fresnica contract list [--json] | contract add NAME C... [--json] | contract remove NAME [--json]";
const CONTRACT_LIST_SCHEMA: &str = "fresnica-contract-list-v1";

pub fn is_saved_contract_command(arguments: &[String]) -> bool {
    matches!(
        arguments.first().map(String::as_str),
        Some("list" | "add" | "remove")
    )
}

pub fn command_contract(client: &FresnicaClient, arguments: &[String]) -> Result<(), String> {
    if is_saved_contract_command(arguments) {
        return command_saved_contracts(client.storage(), client.network(), arguments);
    }

    let mut options = InvokeOptions::parse(arguments)?;
    let requested_target = options.contract_id.clone();
    options.contract_id = resolve_contract_target(client, &requested_target)?;
    let runtime = Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("unable to initialize Soroban runtime: {error}"))?;

    match &options.action {
        InvokeAction::InterfaceHelp => {
            crate::diagnostics::stage("contract: load deployed interface");
            let interface = runtime.block_on(client.contract_interface(&options.contract_id))?;
            record_contract_inspection(client, &interface)?;
            if options.json {
                print_json(&interface_json(&interface))?;
            } else {
                render_interface_help(&interface, &requested_target);
            }
            return Ok(());
        }
        InvokeAction::FunctionHelp { function_name } => {
            crate::diagnostics::stage("contract: load deployed interface");
            let interface = runtime.block_on(client.contract_interface(&options.contract_id))?;
            record_contract_inspection(client, &interface)?;
            let function = interface.function(function_name).ok_or_else(|| {
                format!(
                    "contract {} has no function {function_name:?}",
                    options.contract_id
                )
            })?;
            if options.json {
                print_json(&function_json(function))?;
            } else {
                render_function_help(
                    &requested_target,
                    &options.contract_id,
                    &interface.executable,
                    function,
                );
            }
            return Ok(());
        }
        InvokeAction::Invoke { .. } => {}
    }

    let InvokeAction::Invoke {
        function_name,
        arguments,
    } = options.action
    else {
        unreachable!("help actions returned before invocation")
    };

    crate::diagnostics::stage("contract: simulate invoke");
    let mut invoke = ContractInvokeRequest::new(options.contract_id, function_name, arguments);
    invoke.wallet = options.wallet;
    add_local_address_names(client, &mut invoke)?;
    let outcome = runtime.block_on(client.prepare_contract_invoke_outcome(invoke))?;

    let ContractInvokePreparation::Transaction(mut prepared) = outcome else {
        let ContractInvokePreparation::ReadOnly(result) = outcome else {
            unreachable!("contract invoke preparation has only read-only or transaction outcomes")
        };
        verify_contract_observation(
            client,
            &result.contract_id,
            &result.executable,
            &result.metadata,
            &result.capabilities,
        )?;
        crate::diagnostics::stage("contract: return read-only simulation");
        if options.json {
            print_json(&read_only_json(&result))?;
        } else {
            render_read_only(&result, &requested_target);
        }
        return Ok(());
    };

    verify_contract_observation(
        client,
        &prepared.review.contract_id,
        &prepared.review.executable,
        &prepared.review.metadata,
        &prepared.review.capabilities,
    )?;

    if options.json && !options.yes {
        return Err(
            "write contract call --json requires -y so stdout remains one machine-readable JSON document"
                .to_owned(),
        );
    }

    crate::diagnostics::stage("contract: review simulated transaction");
    if !options.json {
        render_review(&prepared.review, &requested_target);
        if !options.yes && !confirm_submission()? {
            println!("Transaction cancelled.");
            return Ok(());
        }
    }

    let submission = authorize_sign_submit_contract(client, &runtime, &mut prepared)?;
    if options.json {
        print_json(&submission_json(&prepared.review, &submission))?;
    } else {
        println!("Submitted: {}", submission.hash);
        if let Some(ledger) = submission.ledger {
            println!("Ledger:    {ledger}");
        }
    }
    Ok(())
}

pub(crate) fn authorize_sign_submit_contract(
    client: &FresnicaClient,
    runtime: &tokio::runtime::Runtime,
    prepared: &mut PreparedContractInvoke,
) -> Result<TransactionSubmission, String> {
    crate::diagnostics::stage("contract: authorize detached Soroban entries");
    with_software_signer_authorization(client, |passphrase, system_auth| {
        client.authorize_contract_invoke_with_system_auth(prepared, passphrase, system_auth)
    })?;

    crate::diagnostics::stage("contract: sign transaction envelope");
    submit_with_classic_signers(client, |passphrase, system_auth, external| {
        client.sign_contract_invoke_with_providers(prepared, passphrase, system_auth, external)
    })?;

    crate::diagnostics::stage("contract: submit and reconcile");
    runtime.block_on(client.submit_contract_invoke(prepared))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InvokeOptions {
    contract_id: String,
    wallet: Option<String>,
    yes: bool,
    json: bool,
    action: InvokeAction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum InvokeAction {
    InterfaceHelp,
    FunctionHelp {
        function_name: String,
    },
    Invoke {
        function_name: String,
        arguments: Vec<ContractArgumentInput>,
    },
}

impl InvokeOptions {
    fn parse(arguments: &[String]) -> Result<Self, String> {
        if arguments.first().map(String::as_str) == Some("invoke") {
            return Self::parse_legacy(arguments);
        }
        Self::parse_direct(arguments)
    }

    fn parse_direct(arguments: &[String]) -> Result<Self, String> {
        let contract_id = arguments.first().ok_or_else(|| USAGE.to_owned())?.clone();
        let mut wallet = None;
        let mut yes = false;
        let mut json = false;
        let mut index = 1;
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
                "-y" | "--yes" => {
                    yes = true;
                    index += 1;
                }
                "--json" => {
                    json = true;
                    index += 1;
                }
                "-h" | "--help" => {
                    if index + 1 != arguments.len() {
                        return Err(USAGE.to_owned());
                    }
                    return Ok(Self {
                        contract_id,
                        wallet,
                        yes,
                        json,
                        action: InvokeAction::InterfaceHelp,
                    });
                }
                value if !value.starts_with('-') => break,
                _ => return Err(USAGE.to_owned()),
            }
        }
        let action = if index == arguments.len() {
            InvokeAction::InterfaceHelp
        } else {
            parse_dynamic(&arguments[index..])?
        };
        Ok(Self {
            contract_id,
            wallet,
            yes,
            json,
            action,
        })
    }

    fn parse_legacy(arguments: &[String]) -> Result<Self, String> {
        if arguments.len() < 2 {
            return Err(LEGACY_USAGE.to_owned());
        }
        let contract_id = arguments[1].clone();
        let mut wallet = None;
        let mut yes = false;
        let mut json = false;
        let mut index = 2;
        while index < arguments.len() && arguments[index] != "--" {
            match arguments[index].as_str() {
                "--wallet" => {
                    index += 1;
                    wallet = Some(
                        arguments
                            .get(index)
                            .ok_or_else(|| LEGACY_USAGE.to_owned())?
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
                value if !value.starts_with('-') => break,
                _ => return Err(LEGACY_USAGE.to_owned()),
            }
        }
        if arguments.get(index).map(String::as_str) != Some("--") {
            return Err(format!(
                "contract-specific function arguments must follow `--`\n{LEGACY_USAGE}"
            ));
        }
        let action = parse_dynamic(&arguments[index + 1..])?;
        Ok(Self {
            contract_id,
            wallet,
            yes,
            json,
            action,
        })
    }
}

pub fn command_saved_contracts(
    storage: &WalletStorage,
    network: &str,
    arguments: &[String],
) -> Result<(), String> {
    let store = ContractStore::for_home(storage.home(), network);
    match arguments {
        [command] if command == "list" => render_saved_contract_list(network, &store, false),
        [command, format] if command == "list" && format == "--json" => {
            render_saved_contract_list(network, &store, true)
        }
        [command, name, contract_id] if command == "add" => {
            let contract = store.add(name, contract_id)?;
            render_saved_contract_change(network, "saved", &contract, false)
        }
        [command, name, contract_id, format] if command == "add" && format == "--json" => {
            let contract = store.add(name, contract_id)?;
            render_saved_contract_change(network, "saved", &contract, true)
        }
        [command, name] if command == "remove" => {
            let contract = store.remove(name)?;
            render_saved_contract_change(network, "removed", &contract, false)
        }
        [command, name, format] if command == "remove" && format == "--json" => {
            let contract = store.remove(name)?;
            render_saved_contract_change(network, "removed", &contract, true)
        }
        _ => Err(SAVED_CONTRACT_USAGE.to_owned()),
    }
}

fn render_saved_contract_list(
    network: &str,
    store: &ContractStore,
    json_output: bool,
) -> Result<(), String> {
    let contracts = store.list()?;
    if json_output {
        return print_json(&json!({
            "schema": CONTRACT_LIST_SCHEMA,
            "network": network,
            "contracts": contracts.iter().map(saved_contract_json).collect::<Vec<_>>(),
        }));
    }
    if contracts.is_empty() {
        println!("No saved contracts on {network}.");
        return Ok(());
    }
    println!("Saved contracts ({network})");
    for contract in contracts {
        println!("  {:<20} {}", contract.user.name, contract.contract_id);
    }
    Ok(())
}

fn render_saved_contract_change(
    network: &str,
    action: &str,
    contract: &SavedContract,
    json_output: bool,
) -> Result<(), String> {
    if json_output {
        return print_json(&json!({
            "kind": format!("contract_{action}"),
            "network": network,
            "contract": saved_contract_json(contract),
        }));
    }
    match action {
        "saved" => println!("Saved contract \"{}\" on {}.", contract.user.name, network),
        "removed" => println!(
            "Removed contract \"{}\" from {}.",
            contract.user.name, network
        ),
        _ => unreachable!("saved-contract action is internal"),
    }
    println!("Contract: {}", contract.contract_id);
    Ok(())
}

fn saved_contract_json(contract: &SavedContract) -> Value {
    let observed = contract.observed.as_ref().map(|observation| {
        json!({
            "executable": observation.executable.as_str(),
            "wasm_hash": observation.wasm_hash.as_deref(),
            "wasm_meta": observation.wasm_meta.iter().map(|entry| json!({
                "key": entry.key.as_str(),
                "value": entry.value.as_str(),
            })).collect::<Vec<_>>(),
            "derived": observation.derived.as_ref().map(stored_capabilities_json),
        })
    });
    json!({
        "name": contract.user.name.as_str(),
        "contract_id": contract.contract_id.as_str(),
        "observed": observed,
    })
}

fn resolve_contract_target(client: &FresnicaClient, target: &str) -> Result<String, String> {
    if looks_like_contract_id(target) {
        return Ok(target.to_owned());
    }
    let store = ContractStore::for_home(client.storage().home(), client.network());
    if let Some(contract) = store.find(target)? {
        return Ok(contract.contract_id);
    }
    Err(format!(
        "Unknown contract {target:?} on {}. Use a C... contract id or save one with `fresnica --network {} contract add {target} C...`.",
        client.network(),
        client.network()
    ))
}

fn record_contract_inspection(
    client: &FresnicaClient,
    interface: &ContractInterface,
) -> Result<(), String> {
    ContractStore::for_home(client.storage().home(), client.network()).record_observation(
        &interface.contract_id,
        &interface.executable,
        &interface.metadata,
        &interface.capabilities,
    )
}

fn verify_contract_observation(
    client: &FresnicaClient,
    contract_id: &str,
    observation: &ContractExecutableObservation,
    metadata: &[ContractMetadataEntry],
    capabilities: &ContractCapabilities,
) -> Result<(), String> {
    ContractStore::for_home(client.storage().home(), client.network()).verify_observation(
        contract_id,
        observation,
        metadata,
        capabilities,
    )
}

pub(crate) fn add_local_address_names(
    client: &FresnicaClient,
    request: &mut ContractInvokeRequest,
) -> Result<(), String> {
    for wallet in client.storage().list()? {
        if wallet.network == client.network() && request.references_argument_value(&wallet.name) {
            request.add_address_name(&wallet.name, &wallet.address)?;
        }
    }
    let contacts = ContactStore::for_home(client.storage().home());
    for contact in contacts.list()? {
        if request.references_argument_value(&contact.name) {
            request.add_address_name(&contact.name, &contact.address)?;
        }
    }
    let contracts = ContractStore::for_home(client.storage().home(), client.network());
    for contract in contracts.list()? {
        if request.references_argument_value(&contract.user.name) {
            request.add_address_name(&contract.user.name, &contract.contract_id)?;
        }
    }
    Ok(())
}

fn parse_dynamic(arguments: &[String]) -> Result<InvokeAction, String> {
    if arguments.is_empty() || is_help(&arguments[0]) {
        if arguments.len() <= 1 {
            return Ok(InvokeAction::InterfaceHelp);
        }
        return Err(USAGE.to_owned());
    }
    let function_name = arguments[0].clone();
    if arguments.len() == 2 && is_help(&arguments[1]) {
        return Ok(InvokeAction::FunctionHelp { function_name });
    }

    let mut parsed = Vec::new();
    let mut index = 1;
    while index < arguments.len() {
        let flag = &arguments[index];
        let name = flag
            .strip_prefix("--")
            .filter(|name| !name.is_empty())
            .ok_or_else(|| {
                format!("contract argument {flag:?} must be a named option such as --amount 10")
            })?;
        if is_help(flag) {
            return Err(format!(
                "{function_name} --help must be used without invocation arguments"
            ));
        }
        index += 1;
        let value = arguments
            .get(index)
            .ok_or_else(|| format!("contract argument --{name} requires a value"))?;
        parsed.push(ContractArgumentInput::new(name, value));
        index += 1;
    }
    Ok(InvokeAction::Invoke {
        function_name,
        arguments: parsed,
    })
}

fn is_help(value: &str) -> bool {
    matches!(value, "--help" | "-h")
}

fn render_contract_identity(requested_target: &str, contract_id: &str) {
    if requested_target == contract_id {
        println!("Contract: {contract_id}");
    } else {
        println!("Contract: {requested_target}");
        println!("ID:       {contract_id}");
    }
}

fn render_interface_help(interface: &ContractInterface, requested_target: &str) {
    render_contract_identity(requested_target, &interface.contract_id);
    render_executable(&interface.executable);
    render_wasm_metadata(&interface.metadata);
    render_capability_evidence(&interface.capabilities);
    println!("Functions:");
    if interface.functions.is_empty() {
        println!("  (none)");
        return;
    }
    for function in &interface.functions {
        let signature = function
            .inputs
            .iter()
            .map(|input| format!("--{} <{}>", input.name, input.value_type.name))
            .collect::<Vec<_>>()
            .join(" ");
        if signature.is_empty() {
            println!("  {}", function.name);
        } else {
            println!("  {} {signature}", function.name);
        }
        let doc = sanitize_terminal_text(&function.doc);
        if !doc.trim().is_empty() {
            println!("      {}", one_line(&doc));
        }
    }
    println!();
    println!(
        "Use `fresnica contract {} FUNCTION --help` for parameter details.",
        requested_target
    );
}

fn render_function_help(
    requested_target: &str,
    contract_id: &str,
    executable: &ContractExecutableObservation,
    function: &ContractFunction,
) {
    render_contract_identity(requested_target, contract_id);
    render_executable(executable);
    println!("Function: {}", function.name);
    let doc = sanitize_terminal_text(&function.doc);
    if !doc.trim().is_empty() {
        println!("{}", doc.trim());
    }
    println!();
    print!(
        "Usage: fresnica contract {requested_target} [--wallet NAME] [-y] {}",
        function.name
    );
    for input in &function.inputs {
        print!(" --{} <{}>", input.name, input.value_type.name);
    }
    println!();
    if function.inputs.is_empty() {
        println!("Arguments: none");
    } else {
        println!("Arguments:");
        for input in &function.inputs {
            println!("  --{}  {}", input.name, input.value_type.name);
            let doc = sanitize_terminal_text(&input.doc);
            if !doc.trim().is_empty() {
                println!("      {}", one_line(&doc));
            }
            if let Some(example) = &input.value_type.example {
                println!(
                    "      example: {}",
                    one_line(&sanitize_terminal_text(example))
                );
            }
        }
    }
    if !function.outputs.is_empty() {
        println!(
            "Returns: {}",
            function
                .outputs
                .iter()
                .map(|output| output.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
}

fn render_read_only(result: &ContractReadResult, requested_target: &str) {
    println!("Read-only contract result");
    if requested_target == result.contract_id {
        println!("Contract:   {}", result.contract_id);
    } else {
        println!("Contract:   {requested_target} ({})", result.contract_id);
    }
    println!("Function:   {}", result.function_name);
    println!("Network:    {}", result.network);
    render_executable(&result.executable);
    for argument in &result.arguments {
        println!(
            "  --{:<14} {:<16} {}",
            argument.name,
            argument.value_type,
            compact_json(&argument.value)
        );
    }
    if result.arguments.is_empty() {
        println!("Arguments:  none");
    }
    match &result.output {
        Some(output) => println!("Result:     {}", compact_json(output)),
        None => println!("Result:     null"),
    }
    println!("Simulation: ledger {}", result.simulation_ledger);
    println!("Submitted:  no");
}

pub(crate) fn render_review(review: &ContractInvokeReview, requested_target: &str) {
    println!("Review contract invocation");
    println!("Wallet:     {}", review.wallet_name);
    println!("Fee payer:  {}", review.fee_payer);
    if requested_target == review.contract_id {
        println!("Contract:   {}", review.contract_id);
    } else {
        println!("Contract:   {requested_target} ({})", review.contract_id);
    }
    println!("Function:   {}", review.function_name);
    println!("Network:    {}", review.network);
    render_executable(&review.executable);
    for argument in &review.arguments {
        println!(
            "  --{:<14} {:<16} {}",
            argument.name,
            argument.value_type,
            compact_json(&argument.value)
        );
    }
    if review.arguments.is_empty() {
        println!("Arguments:  none");
    }
    println!("Fee:        {} stroops total", review.total_fee_stroops);
    println!("  inclusion {}", review.inclusion_fee_stroops);
    println!("  resource  {}", review.resource_fee_stroops);
    println!("Simulation: ledger {}", review.simulation_ledger);
    if let Some(ledger) = review.authorization_expiration_ledger {
        println!("Auth until: ledger {ledger}");
    }
    if review.authorizers.is_empty() {
        println!("Authorizers: none detached");
    } else {
        println!("Authorizers:");
        for (index, authorizer) in review.authorizers.iter().enumerate() {
            let credential = review
                .credential_types
                .get(index)
                .map(String::as_str)
                .unwrap_or("unknown");
            println!("  {credential}: {authorizer}");
        }
    }
    println!("Prepared tx: {}", review.transaction_hash);
}

fn render_executable(observation: &ContractExecutableObservation) {
    println!("Executable: {}", observation.kind.as_str());
    if let Some(hash) = &observation.wasm_hash {
        println!("Wasm hash:  {hash}");
    }
}

fn executable_json(observation: &ContractExecutableObservation) -> Value {
    json!({
        "kind": observation.kind.as_str(),
        "wasm_hash": observation.wasm_hash.as_deref(),
    })
}

fn render_wasm_metadata(metadata: &[ContractMetadataEntry]) {
    if metadata.is_empty() {
        return;
    }
    println!("Wasm metadata:");
    for entry in metadata {
        println!(
            "  {} = {}",
            one_line(&sanitize_terminal_text(&entry.key)),
            one_line(&sanitize_terminal_text(&entry.value))
        );
    }
}

fn metadata_json(metadata: &[ContractMetadataEntry]) -> Vec<Value> {
    metadata
        .iter()
        .map(|entry| {
            json!({
                "key": entry.key.as_str(),
                "value": entry.value.as_str(),
            })
        })
        .collect()
}

pub(crate) fn render_capability_evidence(capabilities: &ContractCapabilities) {
    let sep41 = &capabilities.sep41;
    if !sep41.native_sac && !sep41.sep47_declared && !sep41.current_interface_compatible {
        return;
    }
    let mut evidence = Vec::new();
    if sep41.native_sac {
        evidence.push("native SAC");
    }
    if sep41.sep47_declared {
        evidence.push("SEP-47 declared");
    }
    evidence.push(if sep41.current_interface_compatible {
        "current interface compatible"
    } else {
        "current interface mismatch"
    });
    println!(
        "SEP-41 {} evidence: {}",
        SEP41_INTERFACE_VERSION,
        evidence.join(", ")
    );
}

pub(crate) fn capabilities_json(capabilities: &ContractCapabilities) -> Value {
    json!({
        "sep41": {
            "interface_version": SEP41_INTERFACE_VERSION,
            "native_sac": capabilities.sep41.native_sac,
            "sep47_declared": capabilities.sep41.sep47_declared,
            "interface_compatible": capabilities.sep41.current_interface_compatible,
        }
    })
}

fn stored_capabilities_json(capabilities: &ContractDerivedCapabilities) -> Value {
    json!({
        "sep41": {
            "interface_version": capabilities.sep41.interface_version.as_str(),
            "native_sac": capabilities.sep41.native_sac,
            "sep47_declared": capabilities.sep41.sep47_declared,
            "interface_compatible": capabilities.sep41.interface_compatible,
        }
    })
}

fn compact_json(value: &Value) -> String {
    serde_json::to_string(value).expect("serde_json::Value serialization cannot fail")
}

fn argument_json(argument: &fresnica_client::ContractArgumentReview) -> Value {
    json!({
        "name": argument.name.as_str(),
        "type": argument.value_type.as_str(),
        "value": argument.value.clone(),
    })
}

pub(crate) fn interface_json(interface: &ContractInterface) -> Value {
    json!({
        "contract_id": interface.contract_id.as_str(),
        "executable": executable_json(&interface.executable),
        "wasm_meta": metadata_json(&interface.metadata),
        "capabilities": capabilities_json(&interface.capabilities),
        "functions": interface.functions.iter().map(function_json).collect::<Vec<_>>(),
    })
}

fn function_json(function: &ContractFunction) -> Value {
    json!({
        "name": function.name.as_str(),
        "doc": function.doc.as_str(),
        "inputs": function.inputs.iter().map(|input| json!({
            "name": input.name.as_str(),
            "doc": input.doc.as_str(),
            "type": input.value_type.name.as_str(),
            "example": input.value_type.example.as_deref(),
        })).collect::<Vec<_>>(),
        "outputs": function.outputs.iter().map(|output| json!({
            "type": output.name.as_str(),
            "example": output.example.as_deref(),
        })).collect::<Vec<_>>(),
    })
}

fn read_only_json(result: &ContractReadResult) -> Value {
    json!({
        "kind": "read_only",
        "contract_id": result.contract_id.as_str(),
        "executable": executable_json(&result.executable),
        "wasm_meta": metadata_json(&result.metadata),
        "capabilities": capabilities_json(&result.capabilities),
        "function": result.function_name.as_str(),
        "arguments": result.arguments.iter().map(argument_json).collect::<Vec<_>>(),
        "result": result.output.clone(),
        "simulation_ledger": result.simulation_ledger,
        "network": result.network.as_str(),
        "submission": Value::Null,
    })
}

pub(crate) fn review_json(review: &ContractInvokeReview) -> Value {
    json!({
        "wallet": review.wallet_name.as_str(),
        "fee_payer": review.fee_payer.as_str(),
        "operation_source": review.operation_source.as_str(),
        "contract_id": review.contract_id.as_str(),
        "executable": executable_json(&review.executable),
        "wasm_meta": metadata_json(&review.metadata),
        "capabilities": capabilities_json(&review.capabilities),
        "function": review.function_name.as_str(),
        "arguments": review.arguments.iter().map(argument_json).collect::<Vec<_>>(),
        "authorizers": review.authorizers.as_slice(),
        "credential_types": review.credential_types.as_slice(),
        "authorization_entry_count": review.auth_entry_count,
        "fee_stroops": {
            "total": review.total_fee_stroops,
            "resource": review.resource_fee_stroops,
            "inclusion": review.inclusion_fee_stroops,
            "minimum_resource": review.min_resource_fee_stroops,
        },
        "simulation_ledger": review.simulation_ledger,
        "authorization_expiration_ledger": review.authorization_expiration_ledger,
        "network": review.network.as_str(),
        "transaction_hash": review.transaction_hash.as_str(),
    })
}

fn submission_json(review: &ContractInvokeReview, submission: &TransactionSubmission) -> Value {
    json!({
        "kind": "transaction",
        "review": review_json(review),
        "submission": {
            "hash": submission.hash.as_str(),
            "ledger": submission.ledger,
        },
    })
}

fn print_json(value: &Value) -> Result<(), String> {
    println!(
        "{}",
        serde_json::to_string_pretty(value)
            .map_err(|error| format!("unable to encode contract JSON: {error}"))?
    );
    Ok(())
}

fn sanitize_terminal_text(input: &str) -> String {
    input
        .chars()
        .map(|character| match character {
            '\n' => '\n',
            '\t' => ' ',
            character if character.is_control() => ' ',
            character => character,
        })
        .collect()
}

fn one_line(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONTRACT: &str = "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAD2KM";

    #[test]
    fn invoke_parser_uses_contract_specific_named_arguments_after_separator() {
        let args = [
            "invoke",
            CONTRACT,
            "--wallet",
            "main",
            "-y",
            "--json",
            "--",
            "transfer",
            "--from",
            "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
            "--to",
            "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAD2KM",
            "--amount",
            "10000000",
        ]
        .map(str::to_owned);

        let options = InvokeOptions::parse(&args).unwrap();
        assert_eq!(options.wallet.as_deref(), Some("main"));
        let InvokeAction::Invoke {
            function_name,
            arguments,
        } = options.action
        else {
            panic!("expected invoke action");
        };
        assert_eq!(function_name, "transfer");
        assert_eq!(arguments[0].name, "from");
        assert_eq!(arguments[2].name, "amount");
        assert_eq!(arguments[2].value, "10000000");
        assert!(options.yes);
        assert!(options.json);
    }

    #[test]
    fn direct_contract_syntax_keeps_host_options_before_function() {
        let args = [
            "aqua",
            "--wallet",
            "bot",
            "-y",
            "--json",
            "transfer",
            "--from",
            "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
            "--wallet",
            "contract-owned-value",
            "--amount",
            "10",
        ]
        .map(str::to_owned);
        let options = InvokeOptions::parse(&args).unwrap();
        assert_eq!(options.contract_id, "aqua");
        assert_eq!(options.wallet.as_deref(), Some("bot"));
        assert!(options.yes);
        assert!(options.json);
        let InvokeAction::Invoke {
            function_name,
            arguments,
        } = options.action
        else {
            panic!("expected invoke action");
        };
        assert_eq!(function_name, "transfer");
        assert_eq!(arguments[1].name, "wallet");
        assert_eq!(arguments[1].value, "contract-owned-value");
    }

    #[test]
    fn direct_contract_without_function_means_interface_help() {
        let args = ["aqua", "--json"].map(str::to_owned);
        let options = InvokeOptions::parse(&args).unwrap();
        assert_eq!(options.contract_id, "aqua");
        assert!(options.json);
        assert!(matches!(options.action, InvokeAction::InterfaceHelp));
    }

    #[test]
    fn dynamic_help_does_not_require_noninteractive_confirmation() {
        let args = ["invoke", CONTRACT, "--json", "--", "transfer", "--help"].map(str::to_owned);
        let options = InvokeOptions::parse(&args).unwrap();
        assert!(matches!(
            options.action,
            InvokeAction::FunctionHelp { ref function_name } if function_name == "transfer"
        ));
    }

    #[test]
    fn machine_invocation_defers_confirmation_until_simulation_classifies_write() {
        let args = [
            "invoke",
            CONTRACT,
            "--json",
            "--",
            "balance",
            "--id",
            "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
        ]
        .map(str::to_owned);
        let options = InvokeOptions::parse(&args).unwrap();
        assert!(options.json);
        assert!(!options.yes);
        assert!(matches!(options.action, InvokeAction::Invoke { .. }));
    }

    #[test]
    fn local_name_matching_only_considers_referenced_argument_values() {
        let request = ContractInvokeRequest::new(
            CONTRACT,
            "balance",
            vec![ContractArgumentInput::new("id", "\"Alice\"")],
        );
        assert!(request.references_argument_value("alice"));
        assert!(!request.references_argument_value("bob"));
    }

    #[test]
    fn contract_specific_arguments_require_separator() {
        let args = ["invoke", CONTRACT, "transfer", "--amount", "10"].map(str::to_owned);
        assert!(InvokeOptions::parse(&args)
            .unwrap_err()
            .contains("must follow `--`"));
    }

    #[test]
    fn capabilities_json_preserves_sep41_evidence_and_version() {
        let capabilities = ContractCapabilities {
            sep41: fresnica_client::ContractSep41Evidence {
                native_sac: false,
                sep47_declared: true,
                current_interface_compatible: true,
            },
        };
        let encoded = capabilities_json(&capabilities);
        assert_eq!(
            encoded["sep41"]["interface_version"],
            SEP41_INTERFACE_VERSION
        );
        assert_eq!(encoded["sep41"]["native_sac"], false);
        assert_eq!(encoded["sep41"]["sep47_declared"], true);
        assert_eq!(encoded["sep41"]["interface_compatible"], true);
    }

    #[test]
    fn wasm_metadata_json_preserves_raw_key_value_facts() {
        let metadata = vec![ContractMetadataEntry {
            key: "home_domain".to_owned(),
            value: "example.org".to_owned(),
        }];
        let encoded = metadata_json(&metadata);
        assert_eq!(encoded[0]["key"], "home_domain");
        assert_eq!(encoded[0]["value"], "example.org");
    }

    #[test]
    fn wasm_metadata_terminal_text_strips_controls() {
        let metadata = ContractMetadataEntry {
            key: "home\u{1b}_domain".to_owned(),
            value: "example.org\u{7}".to_owned(),
        };
        assert_eq!(sanitize_terminal_text(&metadata.key), "home _domain");
        assert_eq!(sanitize_terminal_text(&metadata.value), "example.org ");
    }

    #[test]
    fn terminal_docs_strip_escape_controls() {
        assert_eq!(
            sanitize_terminal_text("safe\u{1b}[31mred\u{7}"),
            "safe [31mred "
        );
    }
}
