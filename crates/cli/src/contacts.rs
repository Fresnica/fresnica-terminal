use fresnica_client::{Contact, ContactStore, WalletStorage};
use serde_json::{json, Value};

pub fn command_contact(storage: &WalletStorage, arguments: &[String]) -> Result<(), String> {
    let store = ContactStore::for_home(storage.home());
    let Some(command) = arguments.first().map(String::as_str) else {
        return Err(usage().to_owned());
    };
    match command {
        "list" => contact_list(&store, &arguments[1..]),
        "add" => contact_add(&store, &arguments[1..]),
        "remove" => contact_remove(&store, &arguments[1..]),
        _ => Err(usage().to_owned()),
    }
}

fn contact_list(store: &ContactStore, arguments: &[String]) -> Result<(), String> {
    let json_output = parse_json_only(arguments)?;
    let contacts = store.list()?;
    if json_output {
        print_json(&json!({
            "kind": "contact_list",
            "contacts": contacts.iter().map(contact_json).collect::<Vec<_>>(),
        }))?;
        return Ok(());
    }
    if contacts.is_empty() {
        println!("No local contacts.");
        return Ok(());
    }
    for contact in contacts {
        if let Some(memo) = contact.memo.as_deref() {
            println!("{:<24} {}  memo={memo}", contact.name, contact.address);
        } else {
            println!("{:<24} {}", contact.name, contact.address);
        }
    }
    Ok(())
}

fn contact_add(store: &ContactStore, arguments: &[String]) -> Result<(), String> {
    if arguments.len() < 2 {
        return Err(usage().to_owned());
    }
    let mut memo = None;
    let mut json_output = false;
    let mut index = 2;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--memo" if memo.is_none() => {
                index += 1;
                memo = Some(
                    arguments
                        .get(index)
                        .ok_or_else(|| usage().to_owned())?
                        .as_str(),
                );
                index += 1;
            }
            "--json" if !json_output => {
                json_output = true;
                index += 1;
            }
            _ => return Err(usage().to_owned()),
        }
    }
    let contact = store.add(&arguments[0], &arguments[1], memo)?;
    if json_output {
        print_json(&json!({"kind": "contact_added", "contact": contact_json(&contact)}))?;
    } else {
        println!("Added contact \"{}\"", contact.name);
        println!("Address: {}", contact.address);
        if let Some(memo) = contact.memo {
            println!("Memo:    {memo}");
        }
    }
    Ok(())
}

fn contact_remove(store: &ContactStore, arguments: &[String]) -> Result<(), String> {
    if arguments.is_empty() || arguments.len() > 2 {
        return Err(usage().to_owned());
    }
    let json_output = match arguments.get(1).map(String::as_str) {
        None => false,
        Some("--json") => true,
        _ => return Err(usage().to_owned()),
    };
    let contact = store.remove(&arguments[0])?;
    if json_output {
        print_json(&json!({"kind": "contact_removed", "contact": contact_json(&contact)}))?;
    } else {
        println!("Removed contact \"{}\"", contact.name);
    }
    Ok(())
}

fn parse_json_only(arguments: &[String]) -> Result<bool, String> {
    match arguments {
        [] => Ok(false),
        [flag] if flag == "--json" => Ok(true),
        _ => Err(usage().to_owned()),
    }
}

fn contact_json(contact: &Contact) -> Value {
    json!({
        "name": contact.name.as_str(),
        "address": contact.address.as_str(),
        "memo": contact.memo.as_deref(),
    })
}

fn print_json(value: &Value) -> Result<(), String> {
    println!(
        "{}",
        serde_json::to_string_pretty(value)
            .map_err(|error| format!("unable to encode contact JSON: {error}"))?
    );
    Ok(())
}

fn usage() -> &'static str {
    "usage: fresnica contact list [--json] | contact add NAME G... [--memo TEXT] [--json] | contact remove NAME [--json]"
}
