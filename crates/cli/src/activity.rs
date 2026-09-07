use fresnica_client::{FresnicaClient, HistoryTransaction, TransactionHistorySnapshot};
use fresnica_terminal_presentation::history_operation_summary;
use serde_json::{json, Value};

const DEFAULT_LIMIT: usize = 20;
const MAX_LIMIT: usize = 50;

pub fn command_activity(client: &FresnicaClient, arguments: &[String]) -> Result<(), String> {
    let options = ActivityOptions::parse(arguments)?;
    crate::diagnostics::stage("activity: fetch complete transactions");
    let snapshot = client.transaction_history(
        options.wallet.as_deref(),
        options.limit,
        options.cursor.as_deref(),
    )?;

    if options.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&activity_json(&snapshot))
                .map_err(|error| format!("unable to encode activity data: {error}"))?
        );
        return Ok(());
    }

    render_activity(&snapshot);
    Ok(())
}

fn render_activity(snapshot: &TransactionHistorySnapshot) {
    println!(
        "Wallet: {} [{}]",
        snapshot.wallet.name, snapshot.wallet.network
    );
    if snapshot.transactions.is_empty() {
        println!("No account activity.");
        return;
    }

    for transaction in &snapshot.transactions {
        println!();
        println!(
            "{}  tx {}  {} operation{}",
            transaction.created_at,
            short_hash(&transaction.transaction_hash),
            transaction.operations.len(),
            if transaction.operations.len() == 1 {
                ""
            } else {
                "s"
            }
        );
        for operation in &transaction.operations {
            println!(
                "  {:<28} {}",
                operation.operation_type(),
                history_operation_summary(operation, &snapshot.wallet.address)
            );
        }
    }

    if let Some(cursor) = &snapshot.next_cursor {
        println!();
        println!("Next cursor: {cursor}");
    }
}

fn activity_json(snapshot: &TransactionHistorySnapshot) -> Value {
    json!({
        "wallet": {
            "name": snapshot.wallet.name.as_str(),
            "address": snapshot.wallet.address.as_str(),
            "network": snapshot.wallet.network.as_str(),
        },
        "transactions": snapshot.transactions.iter().map(transaction_json).collect::<Vec<_>>(),
        "next_cursor": snapshot.next_cursor.as_deref(),
    })
}

fn transaction_json(transaction: &HistoryTransaction) -> Value {
    json!({
        "paging_token": transaction.paging_token.as_str(),
        "transaction_hash": transaction.transaction_hash.as_str(),
        "created_at": transaction.created_at.as_str(),
        "source_account": transaction.source_account.as_str(),
        "operations": transaction.operations.iter()
            .map(crate::read_commands::history_operation_json)
            .collect::<Vec<_>>(),
    })
}

fn short_hash(hash: &str) -> &str {
    hash.get(..hash.len().min(12)).unwrap_or(hash)
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ActivityOptions {
    wallet: Option<String>,
    json: bool,
    limit: usize,
    cursor: Option<String>,
}

impl ActivityOptions {
    fn parse(arguments: &[String]) -> Result<Self, String> {
        let usage = "fresnica activity [--wallet NAME] [--limit N] [--cursor TOKEN] [--json]";
        let mut wallet = None;
        let mut json = false;
        let mut limit = DEFAULT_LIMIT;
        let mut cursor = None;
        let mut index = 0;
        while index < arguments.len() {
            match arguments[index].as_str() {
                "--wallet" => {
                    index += 1;
                    wallet = Some(
                        arguments
                            .get(index)
                            .ok_or_else(|| usage.to_owned())?
                            .to_owned(),
                    );
                    index += 1;
                }
                "--limit" => {
                    index += 1;
                    limit = arguments
                        .get(index)
                        .ok_or_else(|| usage.to_owned())?
                        .parse()
                        .map_err(|_| {
                            format!("--limit requires an integer from 1 to {MAX_LIMIT}")
                        })?;
                    if !(1..=MAX_LIMIT).contains(&limit) {
                        return Err(format!("--limit must be from 1 to {MAX_LIMIT}"));
                    }
                    index += 1;
                }
                "--cursor" => {
                    index += 1;
                    cursor = Some(
                        arguments
                            .get(index)
                            .ok_or_else(|| usage.to_owned())?
                            .to_owned(),
                    );
                    index += 1;
                }
                "--json" => {
                    json = true;
                    index += 1;
                }
                _ => return Err(usage.to_owned()),
            }
        }
        Ok(Self {
            wallet,
            json,
            limit,
            cursor,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_twenty_complete_transactions() {
        assert_eq!(
            ActivityOptions::parse(&[]).unwrap(),
            ActivityOptions {
                wallet: None,
                json: false,
                limit: 20,
                cursor: None,
            }
        );
    }

    #[test]
    fn accepts_opaque_cursor_and_json() {
        let args = [
            "--wallet", "main", "--limit", "50", "--cursor", "123/abc?", "--json",
        ]
        .map(str::to_owned);
        let options = ActivityOptions::parse(&args).unwrap();
        assert_eq!(options.wallet.as_deref(), Some("main"));
        assert_eq!(options.limit, 50);
        assert_eq!(options.cursor.as_deref(), Some("123/abc?"));
        assert!(options.json);
    }

    #[test]
    fn rejects_unbounded_transaction_pages() {
        assert_eq!(
            ActivityOptions::parse(&["--limit".to_owned(), "51".to_owned()]).unwrap_err(),
            "--limit must be from 1 to 50"
        );
    }
}
