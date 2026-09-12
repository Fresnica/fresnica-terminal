use serde_json::{json, Value};

const SCHEMA: &str = "fresnica-capabilities-v1";
const USAGE: &str = "usage: fresnica capabilities --json";

pub fn command(arguments: &[String]) -> Result<(), String> {
    if arguments != ["--json"] {
        return Err(USAGE.to_owned());
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&inventory())
            .map_err(|error| format!("unable to encode capability inventory: {error}"))?
    );
    Ok(())
}

fn inventory() -> Value {
    json!({
        "schema": SCHEMA,
        "cli_version": env!("CARGO_PKG_VERSION"),
        "fresnica_revision": crate::diagnostics::fresnica_revision(),
        "operations": operations(),
    })
}

fn operations() -> Vec<Value> {
    vec![
        operation("account.inspect", "fresnica account [--wallet NAME] --json", "read", &["network", "horizon", "wallet"]),
        operation("balance.list", "fresnica balance [--wallet NAME] --json", "read", &["network", "horizon", "wallet"]),
        operation("history.list", "fresnica history [--wallet NAME] [--limit N] --json", "read", &["network", "horizon", "wallet"]),
        operation("asset.discover", "fresnica asset discover [--limit N] [--cached] --json", "read", &["network", "horizon"]),
        operation("wallet.list", "fresnica wallet list --json", "local_read", &[]),
        operation("wallet.info", "fresnica info [--wallet NAME] --json", "local_read", &["wallet"]),
        operation("wallet.use", "fresnica wallet use NAME --json", "local_write", &["wallet"]),
        operation("wallet.device_unlock.status", "fresnica wallet device-unlock status NAME --json", "local_read", &["wallet"]),
        operation("plugin.list", "fresnica plugin ls --json", "local_read", &[]),
        operation("contact.list", "fresnica contact list --json", "local_read", &[]),
        operation("contact.add", "fresnica contact add NAME G... [--memo TEXT] --json", "local_write", &[]),
        operation("contact.remove", "fresnica contact remove NAME --json", "local_write", &[]),
        operation_with_confirmation("payment.send", "fresnica send AMOUNT ASSET to DESTINATION [--wallet NAME] [--memo TEXT] -y --json", "write", &["network", "horizon", "wallet"], "-y"),
        operation_with_confirmation("trustline.add", "fresnica trust add CODE:GISSUER [--limit VALUE] [--wallet NAME] -y --json", "write", &["network", "horizon", "wallet"], "-y"),
        operation_with_confirmation("trustline.limit", "fresnica trust limit CODE:GISSUER LIMIT [--wallet NAME] -y --json", "write", &["network", "horizon", "wallet"], "-y"),
        operation_with_confirmation("trustline.remove", "fresnica trust remove CODE:GISSUER [--wallet NAME] -y --json", "write", &["network", "horizon", "wallet"], "-y"),
        operation("dex.orderbook", "fresnica dex orderbook SELLING BUYING --json", "read", &["network", "horizon"]),
        operation_with_confirmation("dex.offer.buy", "fresnica dex buy BASE COUNTER AMOUNT PRICE [--wallet NAME] [--allow-trustline] -y --json", "write", &["network", "horizon", "wallet"], "-y"),
        operation_with_confirmation("dex.offer.sell", "fresnica dex sell BASE COUNTER AMOUNT PRICE [--wallet NAME] [--allow-trustline] -y --json", "write", &["network", "horizon", "wallet"], "-y"),
        operation_with_confirmation("dex.offer.update", "fresnica dex update OFFER_ID BASE COUNTER AMOUNT PRICE [--wallet NAME] -y --json", "write", &["network", "horizon", "wallet"], "-y"),
        operation_with_confirmation("dex.offer.cancel", "fresnica dex cancel OFFER_ID [--wallet NAME] -y --json", "write", &["network", "horizon", "wallet"], "-y"),
        operation("dex.offers", "fresnica dex offers [--wallet NAME] [--limit N] --json", "read", &["network", "horizon", "wallet"]),
        operation("dex.trades", "fresnica dex trades BASE COUNTER [--limit N] --json", "read", &["network", "horizon"]),
        operation("dex.fills", "fresnica dex fills [--wallet NAME] [--limit N] --json", "read", &["network", "horizon", "wallet"]),
        operation("dex.candles", "fresnica dex candles BASE COUNTER [--resolution RESOLUTION] [--start MS] [--end MS] [--offset MS] [--limit N] --json", "read", &["network", "horizon"]),
        operation("contract.store.list", "fresnica --network NETWORK contract list --json", "local_read", &["network"]),
        operation("contract.store.add", "fresnica --network NETWORK contract add NAME C... --json", "local_write", &["network"]),
        operation("contract.store.remove", "fresnica --network NETWORK contract remove NAME --json", "local_write", &["network"]),
        operation("contract.inspect", "fresnica --network NETWORK contract TARGET --json", "read", &["network", "rpc"]),
        operation_with_confirmation("contract.invoke", "fresnica --network NETWORK contract TARGET [--wallet NAME] [-y] --json FUNCTION [--NAME VALUE]...", "read_write", &["network", "rpc"], "-y when simulation classifies the operation as a write"),
        operation("token.inspect", "fresnica --network NETWORK token TOKEN --json", "read", &["network", "rpc"]),
        operation("token.balance", "fresnica --network NETWORK token TOKEN balance OWNER --json", "read", &["network", "rpc"]),
        operation_with_confirmation("token.transfer", "fresnica --network NETWORK token TOKEN transfer AMOUNT TO [--wallet NAME] -y --json", "write", &["network", "rpc", "wallet"], "-y"),
    ]
}

fn operation(id: &str, usage: &str, effect: &str, dependencies: &[&str]) -> Value {
    json!({
        "id": id,
        "usage": usage,
        "effect": effect,
        "machine_output": "json",
        "dependencies": dependencies,
    })
}

fn operation_with_confirmation(
    id: &str,
    usage: &str,
    effect: &str,
    dependencies: &[&str],
    confirmation: &str,
) -> Value {
    let mut value = operation(id, usage, effect, dependencies);
    value["noninteractive_confirmation"] = Value::String(confirmation.to_owned());
    value
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn inventory_is_versioned_and_operation_ids_are_unique() {
        let inventory = inventory();
        assert_eq!(inventory["schema"], SCHEMA);
        let operations = inventory["operations"].as_array().unwrap();
        let ids = operations
            .iter()
            .map(|operation| operation["id"].as_str().unwrap())
            .collect::<BTreeSet<_>>();
        assert_eq!(ids.len(), operations.len());
        assert!(ids.contains("wallet.list"));
        assert!(ids.contains("wallet.info"));
        assert!(ids.contains("wallet.use"));
        assert!(ids.contains("wallet.device_unlock.status"));
        assert!(ids.contains("plugin.list"));
        assert!(ids.contains("contact.list"));
        assert!(ids.contains("contact.add"));
        assert!(ids.contains("contact.remove"));
        assert!(ids.contains("payment.send"));
        assert!(ids.contains("trustline.add"));
        assert!(ids.contains("trustline.limit"));
        assert!(ids.contains("trustline.remove"));
        assert!(ids.contains("dex.offer.buy"));
        assert!(ids.contains("dex.offer.sell"));
        assert!(ids.contains("dex.offer.update"));
        assert!(ids.contains("dex.offer.cancel"));
        assert!(ids.contains("contract.invoke"));
        assert!(ids.contains("token.transfer"));
    }
}
