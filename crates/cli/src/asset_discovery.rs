use fresnica_client::{FresnicaClient, MAX_ASSET_CATALOG_LIMIT};

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
