use std::io::{self, Write};

use fresnica_client::{wallet as wallet_ops, RevealedSigningMaterial, WalletRecord, WalletStorage};
use serde_json::{json, Value};
use zeroize::Zeroizing;

use crate::{device_unlock, diagnostics, expand_path, friendbot, ledger, prompt_hidden};

const WALLET_HELP: &str = r#"Wallet commands:
  fresnica wallet list [--json]
  fresnica wallet use NAME
  fresnica wallet create NAME [--index N] [--language LANGUAGE] [--strength BITS]
  fresnica wallet import-secret NAME
  fresnica wallet import-mnemonic NAME [--index N] [--language LANGUAGE]
  fresnica wallet import-watch NAME G...
  fresnica wallet import-ledger NAME [--hd-path N]
  fresnica wallet attach-ledger NAME [--hd-path N]
  fresnica wallet detach-ledger NAME
  fresnica wallet attach-secret NAME
  fresnica wallet attach-mnemonic NAME [--index N] [--language LANGUAGE]
  fresnica wallet detach-signer NAME
  fresnica wallet device-unlock enable|disable|status NAME
  fresnica wallet testnet-fund [--wallet NAME]
  fresnica wallet reveal [NAME]
  fresnica wallet backup NAME PATH [--force]
  fresnica wallet restore PATH [--name NAME]
  fresnica wallet delete NAME
"#;

pub(crate) fn command_info(storage: &WalletStorage, arguments: &[String]) -> Result<(), String> {
    let mut wallet_name = None;
    let mut json_output = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--wallet" if wallet_name.is_none() => {
                index += 1;
                wallet_name = Some(
                    arguments
                        .get(index)
                        .ok_or_else(|| "usage: fresnica info [--wallet NAME] [--json]".to_owned())?
                        .as_str(),
                );
                index += 1;
            }
            "--json" if !json_output => {
                json_output = true;
                index += 1;
            }
            _ => return Err("usage: fresnica info [--wallet NAME] [--json]".to_owned()),
        }
    }
    let record = storage.resolve(wallet_name)?;
    let default = storage.default_name()?;
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "kind": "wallet_info",
                "wallet": wallet_public_json(&record, default.as_deref())?,
                "fresnica_revision": diagnostics::fresnica_revision(),
            }))
            .map_err(|error| format!("unable to encode wallet info JSON: {error}"))?
        );
        return Ok(());
    }
    println!("Name:       {}", record.name);
    println!("Address:    {}", record.address);
    println!("Network:    {}", record.network);
    println!("Type:       {}", record.wallet_type);
    if let Some(configuration) = ledger::configuration(&record)? {
        println!("Signer:     Ledger m/44'/148'/{}'", configuration.hd_path);
    }
    println!(
        "Protection: {}",
        if record.watch_only() {
            "none"
        } else {
            "Fresnica passphrase"
        }
    );
    println!(
        "Default:    {}",
        if default.as_deref() == Some(record.name.as_str()) {
            "yes"
        } else {
            "no"
        }
    );
    println!("SDK/Core:   Rust (direct link)");
    println!(
        "Shared API:  Fresnica SDK/client @ {}",
        diagnostics::short_fresnica_revision()
    );
    Ok(())
}

fn wallet_public_json(record: &WalletRecord, default_name: Option<&str>) -> Result<Value, String> {
    let signer = match ledger::configuration(record)? {
        Some(configuration) => json!({
            "kind": "ledger",
            "hd_path": configuration.hd_path,
        }),
        None => json!({
            "kind": record.wallet_type.as_str(),
        }),
    };
    Ok(json!({
        "name": record.name.as_str(),
        "address": record.address.as_str(),
        "network": record.network.as_str(),
        "wallet_type": record.wallet_type.as_str(),
        "watch_only": record.watch_only(),
        "protection": if record.watch_only() { "none" } else { "fresnica_passphrase" },
        "default": default_name == Some(record.name.as_str()),
        "signer": signer,
    }))
}

pub(crate) fn command_wallet(
    storage: &WalletStorage,
    network: &str,
    arguments: &[String],
) -> Result<(), String> {
    let Some(command) = arguments.first().map(String::as_str) else {
        print!("{WALLET_HELP}");
        return Ok(());
    };
    if matches!(command, "--help" | "-h") && arguments.len() == 1 {
        print!("{WALLET_HELP}");
        return Ok(());
    }
    match command {
        "list" if arguments.len() == 1 => wallet_list(storage, false),
        "list" if arguments.len() == 2 && arguments[1] == "--json" => wallet_list(storage, true),
        "use" if arguments.len() == 2 => {
            storage.set_default(&arguments[1])?;
            println!("Default wallet is now \"{}\"", arguments[1]);
            Ok(())
        }
        "create" => wallet_create(storage, network, &arguments[1..]),
        "import-secret" if arguments.len() == 2 => {
            wallet_import_secret(storage, network, &arguments[1])
        }
        "import-mnemonic" => wallet_import_mnemonic(storage, network, &arguments[1..]),
        "import-watch" if arguments.len() == 3 => {
            wallet_import_watch(storage, network, &arguments[1], &arguments[2])
        }
        "import-ledger" => wallet_import_ledger(storage, network, &arguments[1..]),
        "attach-ledger" => wallet_attach_ledger(storage, &arguments[1..]),
        "detach-ledger" if arguments.len() == 2 => wallet_detach_ledger(storage, &arguments[1]),
        "attach-secret" if arguments.len() == 2 => wallet_attach_secret(storage, &arguments[1]),
        "attach-mnemonic" => wallet_attach_mnemonic(storage, &arguments[1..]),
        "detach-signer" if arguments.len() == 2 => wallet_detach_signer(storage, &arguments[1]),
        "device-unlock" => device_unlock::command(storage, &arguments[1..]),
        "testnet-fund" | "fund" => friendbot::command_fund(storage, network, &arguments[1..]),
        "reveal" if arguments.len() <= 2 => {
            wallet_reveal(storage, arguments.get(1).map(String::as_str))
        }
        "backup" => wallet_backup(storage, &arguments[1..]),
        "restore" => wallet_restore(storage, &arguments[1..]),
        "delete" if arguments.len() == 2 => wallet_delete(storage, &arguments[1]),
        _ => Err(format!(
            "unknown or invalid wallet command: {command}\n\n{WALLET_HELP}"
        )),
    }
}

fn wallet_list(storage: &WalletStorage, json_output: bool) -> Result<(), String> {
    let records = storage.list()?;
    let default = storage.default_name()?;
    if json_output {
        let wallets = records
            .iter()
            .map(|record| wallet_public_json(record, default.as_deref()))
            .collect::<Result<Vec<_>, _>>()?;
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "kind": "wallet_list",
                "wallets": wallets,
            }))
            .map_err(|error| format!("unable to encode wallet list JSON: {error}"))?
        );
        return Ok(());
    }
    if records.is_empty() {
        println!("No local wallets.");
        return Ok(());
    }
    for record in records {
        let marker = if default.as_deref() == Some(record.name.as_str()) {
            "*"
        } else {
            " "
        };
        let signer_type = if ledger::configuration(&record)?.is_some() {
            "ledger"
        } else {
            record.wallet_type.as_str()
        };
        println!(
            "{marker} {:<20} {:<7} {:<10} {}",
            record.name, record.network, signer_type, record.address
        );
    }
    Ok(())
}

fn wallet_create(
    storage: &WalletStorage,
    network: &str,
    arguments: &[String],
) -> Result<(), String> {
    let (name, options) = parse_mnemonic_options(arguments, true)?;
    let mnemonic_passphrase = prompt_hidden("BIP39 passphrase (optional; leave empty if none): ")?;
    let passcode = prompt_app_passcode(storage)?;
    let (record, mnemonic) = wallet_ops::create_mnemonic_record(
        name,
        network,
        &mnemonic_passphrase,
        options.index,
        options.language.as_deref().unwrap_or("english"),
        options.strength,
        &passcode,
    )?;
    save_new_record(storage, &record)?;
    println!("Created wallet \"{}\" [{}]", record.name, record.network);
    println!("Address: {}", record.address);
    println!("Mnemonic: {}", mnemonic.as_str());
    println!("Back up this mnemonic before using the wallet.");
    Ok(())
}

fn wallet_import_secret(storage: &WalletStorage, network: &str, name: &str) -> Result<(), String> {
    let secret = prompt_hidden("Stellar secret (S...): ")?;
    let passcode = prompt_app_passcode(storage)?;
    let record = wallet_ops::import_secret_record(name, network, &secret, &passcode)?;
    save_new_record(storage, &record)?;
    println!("Imported wallet \"{}\"", record.name);
    println!("Address: {}", record.address);
    Ok(())
}

fn wallet_import_mnemonic(
    storage: &WalletStorage,
    network: &str,
    arguments: &[String],
) -> Result<(), String> {
    let (name, options) = parse_mnemonic_options(arguments, false)?;
    let mnemonic = prompt_hidden("Mnemonic phrase: ")?;
    let mnemonic_passphrase = prompt_hidden("BIP39 passphrase (optional; leave empty if none): ")?;
    let passcode = prompt_app_passcode(storage)?;
    let record = wallet_ops::import_mnemonic_record(
        name,
        network,
        &mnemonic,
        &mnemonic_passphrase,
        options.index,
        options.language.as_deref(),
        &passcode,
    )?;
    save_new_record(storage, &record)?;
    println!("Imported wallet \"{}\"", record.name);
    println!("Address: {}", record.address);
    Ok(())
}

fn wallet_import_watch(
    storage: &WalletStorage,
    network: &str,
    name: &str,
    address: &str,
) -> Result<(), String> {
    let record = wallet_ops::import_watch_record(name, network, address)?;
    save_new_record(storage, &record)?;
    println!("Added watch-only wallet \"{}\"", record.name);
    Ok(())
}

fn wallet_import_ledger(
    storage: &WalletStorage,
    network: &str,
    arguments: &[String],
) -> Result<(), String> {
    let (name, hd_path) = parse_ledger_options(arguments)?;
    let address = ledger::public_key(hd_path)?;
    let mut record = wallet_ops::import_watch_record(name, network, &address)?;
    ledger::attach_configuration(&mut record, hd_path)?;
    save_new_record(storage, &record)?;
    println!(
        "Imported Ledger signer \"{}\" [{}]",
        record.name, record.network
    );
    println!("Address: {}", record.address);
    println!("Path:    m/44'/148'/{hd_path}'");
    Ok(())
}

fn wallet_attach_ledger(storage: &WalletStorage, arguments: &[String]) -> Result<(), String> {
    let (name, hd_path) = parse_ledger_options(arguments)?;
    let mut record = storage.load(name)?;
    if !record.watch_only() {
        return Err("Ledger can only be attached to a watch-only wallet".to_owned());
    }
    if ledger::configuration(&record)?.is_some() {
        return Err("wallet already has a Ledger signer configured".to_owned());
    }
    let address = ledger::public_key(hd_path)?;
    if address != record.address {
        return Err(format!(
            "connected Ledger account at m/44'/148'/{hd_path}' is {address}, expected {}",
            record.address
        ));
    }
    ledger::attach_configuration(&mut record, hd_path)?;
    storage.save(&record, true)?;
    println!(
        "Attached Ledger signer to watch-only wallet \"{}\"",
        record.name
    );
    println!("Address: {}", record.address);
    println!("Path:    m/44'/148'/{hd_path}'");
    Ok(())
}

fn wallet_detach_ledger(storage: &WalletStorage, name: &str) -> Result<(), String> {
    let mut record = storage.load(name)?;
    if !ledger::detach_configuration(&mut record)? {
        return Err("wallet has no Ledger signer configured".to_owned());
    }
    storage.save(&record, true)?;
    println!("Detached Ledger signer from wallet \"{}\"", record.name);
    println!("Address: {}", record.address);
    Ok(())
}

fn wallet_attach_secret(storage: &WalletStorage, name: &str) -> Result<(), String> {
    let record = storage.load(name)?;
    if !record.watch_only() {
        return Err("wallet already has signing material".to_owned());
    }
    if ledger::configuration(&record)?.is_some() {
        return Err("wallet already uses a Ledger signer; detach it before attaching software signing material".to_owned());
    }
    let secret = prompt_hidden("Stellar secret (S...): ")?;
    let passcode = prompt_app_passcode(storage)?;
    let updated = wallet_ops::attach_secret_record(&record, &secret, &passcode)?;
    storage.save(&updated, true)?;
    println!(
        "Attached matching secret signer to watch-only wallet \"{}\"",
        updated.name
    );
    println!("Address: {}", updated.address);
    Ok(())
}

fn wallet_attach_mnemonic(storage: &WalletStorage, arguments: &[String]) -> Result<(), String> {
    let (name, options) = parse_mnemonic_options(arguments, false)?;
    let record = storage.load(name)?;
    if !record.watch_only() {
        return Err("wallet already has signing material".to_owned());
    }
    if ledger::configuration(&record)?.is_some() {
        return Err("wallet already uses a Ledger signer; detach it before attaching software signing material".to_owned());
    }
    let mnemonic = prompt_hidden("Mnemonic phrase: ")?;
    let mnemonic_passphrase = prompt_hidden("BIP39 passphrase (optional; leave empty if none): ")?;
    let passcode = prompt_app_passcode(storage)?;
    let updated = wallet_ops::attach_mnemonic_record(
        &record,
        &mnemonic,
        &mnemonic_passphrase,
        options.index,
        options.language.as_deref(),
        &passcode,
    )?;
    storage.save(&updated, true)?;
    println!(
        "Attached matching mnemonic signer to watch-only wallet \"{}\"",
        updated.name
    );
    println!("Address: {}", updated.address);
    Ok(())
}

fn wallet_detach_signer(storage: &WalletStorage, name: &str) -> Result<(), String> {
    let record = storage.load(name)?;
    if record.watch_only() {
        return Err("wallet is already watch-only".to_owned());
    }
    print!("Remove local signing material from wallet \"{name}\" and keep it watch-only? [y/N] ");
    io::stdout()
        .flush()
        .map_err(|error| format!("unable to write prompt: {error}"))?;
    let mut answer = String::new();
    io::stdin()
        .read_line(&mut answer)
        .map_err(|error| format!("unable to read confirmation: {error}"))?;
    if !matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
        return Ok(());
    }
    let passcode = prompt_hidden("Fresnica passphrase: ")?;
    let updated = wallet_ops::detach_signer_record(&record, &passcode)?;
    storage.save(&updated, true)?;
    println!("Wallet \"{}\" is now watch-only", updated.name);
    println!("Address: {}", updated.address);
    Ok(())
}

fn wallet_reveal(storage: &WalletStorage, name: Option<&str>) -> Result<(), String> {
    let record = storage.resolve(name)?;
    if record.watch_only() {
        return Err("watch-only wallet has no signing material".to_owned());
    }
    let passcode = prompt_hidden("Fresnica passphrase: ")?;
    let material = wallet_ops::reveal_record(&record, &passcode)?;
    match material {
        RevealedSigningMaterial::Secret { secret } => {
            println!("Wallet: {}", record.name);
            println!("Stellar secret: {}", secret.as_str());
        }
        RevealedSigningMaterial::Mnemonic {
            mnemonic,
            mnemonic_passphrase,
            index,
            language,
        } => {
            println!("Wallet: {}", record.name);
            println!("Mnemonic: {}", mnemonic.as_str());
            if !mnemonic_passphrase.is_empty() {
                println!("BIP39 passphrase: {}", mnemonic_passphrase.as_str());
            }
            println!("Derivation index: {index}");
            println!("Language: {language}");
        }
    }
    Ok(())
}

fn wallet_backup(storage: &WalletStorage, arguments: &[String]) -> Result<(), String> {
    if arguments.len() < 2 || arguments.len() > 3 {
        return Err("usage: fresnica wallet backup NAME PATH [--force]".to_owned());
    }
    let force = arguments.get(2).is_some_and(|value| value == "--force");
    if arguments.len() == 3 && !force {
        return Err("usage: fresnica wallet backup NAME PATH [--force]".to_owned());
    }
    let record = storage.load(&arguments[0])?;
    let destination = expand_path(&arguments[1])?;
    storage.write_backup(&record, &destination, force)?;
    println!(
        "Encrypted backup for \"{}\" written to {}",
        record.name,
        destination.display()
    );
    Ok(())
}

fn wallet_restore(storage: &WalletStorage, arguments: &[String]) -> Result<(), String> {
    if arguments.is_empty() || arguments.len() > 3 {
        return Err("usage: fresnica wallet restore PATH [--name NAME]".to_owned());
    }
    let source = expand_path(&arguments[0])?;
    let mut record = WalletStorage::read_backup(&source)?;
    if arguments.len() > 1 {
        if arguments.len() != 3 || arguments[1] != "--name" {
            return Err("usage: fresnica wallet restore PATH [--name NAME]".to_owned());
        }
        if arguments[2].trim().is_empty() {
            return Err("restored wallet name cannot be empty".to_owned());
        }
        record.name = arguments[2].clone();
    }
    if !record.watch_only() && wallet_ops::has_app_passcode(storage)? {
        let passcode = prompt_existing_app_passcode(storage)?;
        wallet_ops::validate_restore_signer_compatibility(&record, &passcode)?;
    }
    let ledger_signer = ledger::configuration(&record)?;
    save_new_record(storage, &record)?;
    if let Some(configuration) = ledger_signer {
        println!(
            "Restored Ledger wallet \"{}\" [{}]",
            record.name, record.network
        );
        println!("Path: m/44'/148'/{}'", configuration.hd_path);
    } else if record.watch_only() {
        println!(
            "Restored watch-only wallet \"{}\" [{}]",
            record.name, record.network
        );
    } else {
        println!(
            "Restored wallet \"{}\" [{}]; unlock with the Fresnica passphrase",
            record.name, record.network
        );
    }
    Ok(())
}

fn wallet_delete(storage: &WalletStorage, name: &str) -> Result<(), String> {
    storage.load(name)?;
    print!("Delete wallet \"{name}\" metadata and encrypted secret? [y/N] ");
    io::stdout()
        .flush()
        .map_err(|error| format!("unable to write prompt: {error}"))?;
    let mut answer = String::new();
    io::stdin()
        .read_line(&mut answer)
        .map_err(|error| format!("unable to read confirmation: {error}"))?;
    if answer.trim().to_lowercase() != "y" {
        return Ok(());
    }
    let was_default = storage.default_name()?.as_deref() == Some(name);
    storage.delete(name)?;
    if was_default {
        if let Some(next) = storage.list()?.first() {
            storage.set_default(&next.name)?;
        }
    }
    println!("Deleted wallet \"{name}\"");
    Ok(())
}

fn save_new_record(storage: &WalletStorage, record: &WalletRecord) -> Result<(), String> {
    storage.save(record, false)?;
    if storage.default_name()?.is_none() {
        storage.set_default(&record.name)?;
    }
    Ok(())
}

fn prompt_app_passcode(storage: &WalletStorage) -> Result<Zeroizing<String>, String> {
    if wallet_ops::has_app_passcode(storage)? {
        prompt_existing_app_passcode(storage)
    } else {
        prompt_new_passcode()
    }
}

fn prompt_existing_app_passcode(storage: &WalletStorage) -> Result<Zeroizing<String>, String> {
    let passcode = prompt_hidden("Fresnica passphrase: ")?;
    if passcode.is_empty() {
        return Err("Fresnica passphrase cannot be empty".to_owned());
    }
    wallet_ops::validate_app_passcode(storage, &passcode)?;
    Ok(passcode)
}

fn parse_ledger_options(arguments: &[String]) -> Result<(&str, u32), String> {
    const USAGE: &str = "usage: fresnica wallet import-ledger NAME [--hd-path N]
       fresnica wallet attach-ledger NAME [--hd-path N]";
    let name = arguments.first().ok_or_else(|| USAGE.to_owned())?;
    let mut hd_path = 0u32;
    let mut cursor = 1;
    while cursor < arguments.len() {
        if arguments[cursor] != "--hd-path" {
            return Err(USAGE.to_owned());
        }
        cursor += 1;
        hd_path = arguments
            .get(cursor)
            .ok_or_else(|| USAGE.to_owned())?
            .parse()
            .map_err(|_| "--hd-path requires an unsigned account index".to_owned())?;
        cursor += 1;
    }
    Ok((name, hd_path))
}

struct MnemonicOptions {
    index: usize,
    language: Option<String>,
    strength: usize,
}

fn parse_mnemonic_options(
    arguments: &[String],
    allow_strength: bool,
) -> Result<(&str, MnemonicOptions), String> {
    let name = arguments
        .first()
        .ok_or_else(|| "wallet name is required".to_owned())?;
    let mut index_value = 0usize;
    let mut language = if allow_strength {
        Some("english".to_owned())
    } else {
        None
    };
    let mut strength = 256usize;
    let mut cursor = 1;
    while cursor < arguments.len() {
        match arguments[cursor].as_str() {
            "--index" => {
                cursor += 1;
                index_value = arguments
                    .get(cursor)
                    .ok_or_else(|| "--index requires a number".to_owned())?
                    .parse()
                    .map_err(|_| "--index requires a non-negative integer".to_owned())?;
                cursor += 1;
            }
            "--language" => {
                cursor += 1;
                language = Some(
                    arguments
                        .get(cursor)
                        .ok_or_else(|| "--language requires a value".to_owned())?
                        .to_owned(),
                );
                cursor += 1;
            }
            "--strength" if allow_strength => {
                cursor += 1;
                strength = arguments
                    .get(cursor)
                    .ok_or_else(|| "--strength requires a value".to_owned())?
                    .parse()
                    .map_err(|_| "--strength requires a number".to_owned())?;
                if !matches!(strength, 128 | 160 | 192 | 224 | 256) {
                    return Err("--strength must be 128, 160, 192, 224, or 256".to_owned());
                }
                cursor += 1;
            }
            other => return Err(format!("unknown option: {other}")),
        }
    }
    Ok((
        name,
        MnemonicOptions {
            index: index_value,
            language,
            strength,
        },
    ))
}

fn prompt_new_passcode() -> Result<Zeroizing<String>, String> {
    let passcode = prompt_hidden("Create Fresnica passphrase: ")?;
    wallet_ops::validate_new_passphrase(&passcode)?;
    if wallet_ops::new_passphrase_is_short(&passcode) {
        eprintln!(
            "Warning: this passphrase is shorter than the recommended {} characters.",
            wallet_ops::RECOMMENDED_FRESNICA_PASSPHRASE_CHARS
        );
        eprintln!("If your wallet data is stolen, a short passphrase is easier to guess offline.");
        print!("Use this passphrase anyway? [y/N] ");
        io::stdout()
            .flush()
            .map_err(|error| format!("unable to write prompt: {error}"))?;
        let mut answer = String::new();
        io::stdin()
            .read_line(&mut answer)
            .map_err(|error| format!("unable to read confirmation: {error}"))?;
        if !answer.trim().eq_ignore_ascii_case("y") {
            return Err("passphrase creation cancelled".to_owned());
        }
    }
    let confirmation = prompt_hidden("Confirm Fresnica passphrase: ")?;
    if passcode.as_str() != confirmation.as_str() {
        return Err("Fresnica passphrases do not match".to_owned());
    }
    Ok(passcode)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_create_options_without_cli_framework() {
        let args = vec![
            "alpha".to_owned(),
            "--index".to_owned(),
            "4".to_owned(),
            "--language".to_owned(),
            "japanese".to_owned(),
            "--strength".to_owned(),
            "128".to_owned(),
        ];
        let (name, options) = parse_mnemonic_options(&args, true).unwrap();
        assert_eq!(name, "alpha");
        assert_eq!(options.index, 4);
        assert_eq!(options.language.as_deref(), Some("japanese"));
        assert_eq!(options.strength, 128);
    }
}
