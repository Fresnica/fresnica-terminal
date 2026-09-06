from pathlib import Path

OLD = "9ba6f23cefe34e8d5940b311ec78f27eed982fe7"
NEW = "5f5dc1715fd5a449f538afb6a643100075341732"


def replace(path: str, old: str, new: str, *, count: int | None = None) -> None:
    file = Path(path)
    text = file.read_text()
    found = text.count(old)
    if count is not None and found != count:
        raise SystemExit(f"{path}: expected {count} matches, found {found}")
    if found == 0:
        raise SystemExit(f"{path}: pattern not found: {old!r}")
    file.write_text(text.replace(old, new))


Path("FRESNICA_REV").write_text(NEW + "\n")
replace("Cargo.toml", OLD, NEW, count=2)
replace("Cargo.lock", OLD, NEW)

main = Path("crates/cli/src/main.rs")
text = main.read_text()

replacements = [
    (
        "mod anchor;\n",
        "mod anchor;\nmod asset_discovery;\n",
    ),
    (
        "  fresnica [--home PATH] [--network mainnet|testnet] history [--wallet NAME] [--limit N] [--json]\n"
        "  fresnica [--home PATH] [--network mainnet|testnet] send AMOUNT ASSET to DESTINATION [--wallet NAME] [--memo TEXT] [-y]\n",
        "  fresnica [--home PATH] [--network mainnet|testnet] history [--wallet NAME] [--limit N] [--json]\n"
        "  fresnica [--home PATH] [--network mainnet|testnet] asset discover [--limit N] [--cached] [--json]\n"
        "  fresnica [--home PATH] [--network mainnet|testnet] send AMOUNT ASSET to DESTINATION [--wallet NAME] [--memo TEXT] [-y]\n",
    ),
    (
        "  history                       Show newest Horizon operations (default 20, max 200)\n"
        "  send                          Review, sign through Fresnica SDK/Core, and submit a payment\n",
        "  history                       Show newest Horizon operations (default 20, max 200)\n"
        "  asset                         Discover exact issued-asset identities and optional metadata\n"
        "  send                          Review, sign through Fresnica SDK/Core, and submit a payment\n",
    ),
    (
        '        "history" => read_commands::command_history(&client, &global.command[1..]),\n'
        '        "send" => send::command_send(&client, &global.command[1..]),\n',
        '        "history" => read_commands::command_history(&client, &global.command[1..]),\n'
        '        "asset" => asset_discovery::command_asset(&client, &global.command[1..]),\n'
        '        "send" => send::command_send(&client, &global.command[1..]),\n',
    ),
    (
        '        Some("history") => "CLI command: history",\n'
        '        Some("send") => "CLI command: send",\n',
        '        Some("history") => "CLI command: history",\n'
        '        Some("asset") => "CLI command: asset",\n'
        '        Some("send") => "CLI command: send",\n',
    ),
]

for old, new in replacements:
    if text.count(old) != 1:
        raise SystemExit(f"main.rs replacement expected once, found {text.count(old)}: {old!r}")
    text = text.replace(old, new)
main.write_text(text)

Path("crates/cli/src/asset_discovery.rs").write_text(r'''use fresnica_client::{FresnicaClient, MAX_ASSET_CATALOG_LIMIT};

const DEFAULT_LIMIT: usize = 20;

pub fn command_asset(client: &FresnicaClient, arguments: &[String]) -> Result<(), String> {
    let Some(command) = arguments.first().map(String::as_str) else {
        return Err(usage().to_owned());
    };
    match command {
        "discover" => command_discover(client, &arguments[1..]),
        _ => Err(usage().to_owned()),
    }
}

fn command_discover(client: &FresnicaClient, arguments: &[String]) -> Result<(), String> {
    let request = DiscoverRequest::parse(arguments)?;
    crate::diagnostics::stage("asset: load discovery catalog");
    let snapshot = client.asset_catalog(request.limit, !request.cached)?;

    if request.json {
        let payload = serde_json::json!({
            "network": client.network(),
            "refreshed": snapshot.refreshed,
            "assets": snapshot.entries,
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&payload)
                .map_err(|error| format!("unable to encode asset catalog: {error}"))?
        );
        return Ok(());
    }

    println!(
        "Asset catalog · {} · {}",
        client.network(),
        if snapshot.refreshed {
            "refreshed"
        } else {
            "cached/manual"
        }
    );
    for entry in snapshot.entries {
        println!("{}", entry.identity);
        let mut metadata = Vec::new();
        if let Some(domain) = entry.domain {
            metadata.push(domain);
        }
        if let Some(name) = entry.name {
            metadata.push(name);
        }
        if let Some(organization) = entry.organization {
            metadata.push(organization);
        }
        metadata.push(entry.source);
        println!("  {}", metadata.join(" · "));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DiscoverRequest {
    limit: usize,
    cached: bool,
    json: bool,
}

impl DiscoverRequest {
    fn parse(arguments: &[String]) -> Result<Self, String> {
        let mut limit = DEFAULT_LIMIT;
        let mut cached = false;
        let mut json = false;
        let mut index = 0;
        while index < arguments.len() {
            match arguments[index].as_str() {
                "--limit" => {
                    index += 1;
                    limit = arguments
                        .get(index)
                        .ok_or_else(|| usage().to_owned())?
                        .parse::<usize>()
                        .ok()
                        .filter(|value| (1..=MAX_ASSET_CATALOG_LIMIT).contains(value))
                        .ok_or_else(|| {
                            format!("--limit must be from 1 to {MAX_ASSET_CATALOG_LIMIT}")
                        })?;
                    index += 1;
                }
                "--cached" => {
                    cached = true;
                    index += 1;
                }
                "--json" => {
                    json = true;
                    index += 1;
                }
                _ => return Err(usage().to_owned()),
            }
        }
        Ok(Self {
            limit,
            cached,
            json,
        })
    }
}

fn usage() -> &'static str {
    "usage: fresnica asset discover [--limit N] [--cached] [--json]"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discover_defaults_to_refreshable_catalog() {
        assert_eq!(
            DiscoverRequest::parse(&[]).unwrap(),
            DiscoverRequest {
                limit: DEFAULT_LIMIT,
                cached: false,
                json: false,
            }
        );
    }

    #[test]
    fn discover_accepts_cache_limit_and_json() {
        let args = ["--cached", "--limit", "7", "--json"].map(str::to_owned);
        assert_eq!(
            DiscoverRequest::parse(&args).unwrap(),
            DiscoverRequest {
                limit: 7,
                cached: true,
                json: true,
            }
        );
    }

    #[test]
    fn discover_rejects_out_of_range_limit() {
        let args = ["--limit", "51"].map(str::to_owned);
        assert_eq!(
            DiscoverRequest::parse(&args).unwrap_err(),
            "--limit must be from 1 to 50"
        );
    }
}
''')

if OLD in Path("Cargo.toml").read_text() or OLD in Path("Cargo.lock").read_text():
    raise SystemExit("stale Fresnica revision remains after pin update")
