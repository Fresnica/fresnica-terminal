use std::{env, time::Duration};

use serde_json::Value;
use stellar_strkey::Strkey;
use url::Url;

const MAINNET: &str = "7ac33997544e3175d266bd022439b22cdb16508c01163f26e5cb2a3e1045a979";
const UNAVAILABLE: &str = "SAINT portrait unavailable. You can return to the wallet with Esc.";

pub(super) fn lookup(address: &str) -> Result<Vec<String>, String> {
    let base = env::var("FRESNICA_SAINT_URL")
        .map_err(|_| "Set FRESNICA_SAINT_URL to your private SAINT origin.".to_owned())?;
    lookup_at(address, &base)
}

fn portrait_url(address: &str, base: &str) -> Result<Url, String> {
    if !matches!(
        Strkey::from_string(address),
        Ok(Strkey::PublicKeyEd25519(_) | Strkey::Contract(_))
    ) {
        return Err("Enter a valid Stellar G or C address.".to_owned());
    }
    let invalid = || "FRESNICA_SAINT_URL must be a bare HTTP(S) origin.".to_owned();
    let mut url = Url::parse(base).map_err(|_| invalid())?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path() != "/"
    {
        return Err(invalid());
    }
    url.set_path("/v1/address-portrait");
    url.query_pairs_mut().append_pair("address", address);
    Ok(url)
}

fn lookup_at(address: &str, base: &str) -> Result<Vec<String>, String> {
    let url = portrait_url(address, base)?;
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(3)))
        .proxy(None)
        .max_redirects(0)
        .http_status_as_error(false)
        .build()
        .into();
    let mut response = agent
        .get(url.as_str())
        .call()
        .map_err(|_| UNAVAILABLE.to_owned())?;
    let code = response.status().as_u16();
    if code == 429 {
        return Ok(vec!["SAINT is busy. Press Enter to retry later.".to_owned()]);
    }
    if !matches!(code, 200 | 202) {
        return Err(UNAVAILABLE.to_owned());
    }
    let json: Value = response
        .body_mut()
        .with_config()
        .limit(512 * 1024)
        .read_json()
        .map_err(|_| UNAVAILABLE.to_owned())?;
    format_portrait(address, code, &json)
}

fn required_number(value: &Value) -> Result<u64, String> {
    value.as_u64().ok_or_else(|| UNAVAILABLE.to_owned())
}

fn ready_label(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(|s| s.chars().filter(|c| !c.is_control()).take(140).collect())
}

fn items(value: &Value) -> &[Value] {
    value.as_array().map(Vec::as_slice).unwrap_or_default()
}

fn format_portrait(address: &str, code: u16, json: &Value) -> Result<Vec<String>, String> {
    if json["schema"] != "saint.address-portrait.v1"
        || json["address"] != address
        || json["network"] != "mainnet"
        || json["network_fingerprint"] != MAINNET
    {
        return Err(UNAVAILABLE.to_owned());
    }
    let published = required_number(&json["published_through"])?;
    let status = json["status"].as_str().unwrap_or_default();
    if code == 202 && status == "QUEUED" {
        let through = required_number(&json["requested_through"])?;
        if through > published {
            return Err(UNAVAILABLE.to_owned());
        }
        return Ok(vec![format!(
            "Queued at ledger {through}. Press Enter to check again."
        )]);
    }
    if code != 200 || !matches!(status, "OBSERVED" | "NO_ACTIVITY_IN_PERIOD") {
        return Err(UNAVAILABLE.to_owned());
    }
    let first = required_number(&json["coverage"]["first"])?;
    let through = required_number(&json["coverage"]["through"])?;
    if first == 0 || first > through || through > published {
        return Err(UNAVAILABLE.to_owned());
    }
    let mut lines = vec![format!(
        "Observed window: {first}–{through} (published: {published})"
    )];
    if status == "NO_ACTIVITY_IN_PERIOD" {
        lines.push("No activity in this ledger window.".to_owned());
        return Ok(lines);
    }
    let transactions = required_number(&json["indexed_transactions"])?;
    let sampled = required_number(&json["sampled_history"]["transactions"])?;
    lines.push(format!(
        "Indexed transactions: {transactions} · analyzed: {sampled}"
    ));
    for pattern in items(&json["known_patterns"]) {
        if let Some(label) =
            ready_label(&pattern["pattern"]).or_else(|| ready_label(&pattern["mechanism"]))
        {
            lines.push(format!("Observed pattern: {label}"));
        }
    }
    for candidate in items(&json["pending_patterns"]) {
        let Some(topology) = ready_label(&candidate["topology"]) else {
            continue;
        };
        lines.push(format!(
            "Candidate {} · review pending",
            topology.chars().take(12).collect::<String>()
        ));
        let review = &candidate["review"];
        if review["status"] == "REPRODUCED_IN_TWO_WINDOWS" {
            if let Some(mechanism) = ready_label(&review["mechanism"]) {
                lines.push(format!("Reproduced in two windows: {mechanism}"));
            }
            for sample in items(&review["samples"]) {
                for witness in items(&sample["witnesses"]) {
                    if let (Some(ledger), Some(hash)) =
                        (witness["ledger"].as_u64(), witness["tx_hash"].as_str())
                    {
                        if hash.len() == 64 && hash.bytes().all(|c| c.is_ascii_hexdigit()) {
                            lines.push(format!("Ledger {ledger} tx: {hash}"));
                        }
                    }
                }
            }
        }
    }
    for witness in items(&json["witnesses"]) {
        let (Some(ledger), Some(tx), Some(op), Some(family)) = (
            witness["ledger"].as_u64(),
            witness["tx"].as_u64(),
            witness["op"].as_u64(),
            ready_label(&witness["family"]),
        ) else {
            continue;
        };
        lines.push(format!("Ledger {ledger} · tx {tx} · op {op} · {family}"));
    }
    Ok(lines)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::{
        io::{BufRead, BufReader, Read, Write},
        net::TcpListener,
        thread,
    };

    const ADDRESS: &str = "GBS3IY34W22EK3TPXWDKRTECYVTRZS3SFKRLJHNKK6BRDRE25CGP2QQC";

    fn envelope(status: &str) -> Value {
        json!({"schema":"saint.address-portrait.v1","address":ADDRESS,"network":"mainnet",
              "network_fingerprint":MAINNET,"published_through":200,"status":status})
    }

    fn request_line(reader: &mut impl BufRead) -> String {
        let mut first = String::new();
        assert!(reader.read_line(&mut first).unwrap() > 0);
        let mut header = String::new();
        loop {
            assert!(reader.read_line(&mut header).unwrap() > 0);
            if header == "\r\n" {
                return first;
            }
            header.clear();
        }
    }

    #[test]
    fn queued_result_uses_the_configured_private_origin_and_exact_address() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let mut body = envelope("QUEUED");
        body["requested_through"] = json!(200);
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let wire = request_line(&mut BufReader::new(&mut stream));
            assert!(wire.starts_with(&format!("GET /v1/address-portrait?address={ADDRESS} ")));
            let body = body.to_string();
            write!(
                stream,
                "HTTP/1.1 202 Accepted\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        });
        let lines = lookup_at(ADDRESS, &origin).unwrap();
        assert!(lines[0].contains("Queued at ledger 200"));
        server.join().unwrap();
    }

    #[test]
    fn renders_coverage_and_evidence_and_rejects_a_wrong_scope() {
        let mut result = envelope("OBSERVED");
        result["coverage"] = json!({"first":11,"through":170});
        result["indexed_transactions"] = json!(24);
        result["sampled_history"] = json!({"transactions":24});
        result["witnesses"] = json!([{"ledger":170,"tx":1,"op":2,"family":"transfer"}]);
        let text = format_portrait(ADDRESS, 200, &result).unwrap().join("\n");
        assert!(text.contains("11–170 (published: 200)"));
        assert!(text.contains("Ledger 170 · tx 1 · op 2 · transfer"));
        result["status"] = json!("NO_ACTIVITY_IN_PERIOD");
        assert!(
            format_portrait(ADDRESS, 200, &result).unwrap()[1].contains("in this ledger window")
        );
        result["network_fingerprint"] = json!("bad");
        assert!(format_portrait(ADDRESS, 200, &result).is_err());
        assert!(portrait_url("invalid", "http://127.0.0.1").is_err());
        assert!(portrait_url(ADDRESS, "http://user@127.0.0.1").is_err());
    }

    #[test]
    fn stalled_portrait_response_returns_an_optional_error() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let server = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut reader = BufReader::new(stream);
            assert!(request_line(&mut reader).starts_with("GET "));
            assert_eq!(reader.read(&mut [0]).unwrap(), 0);
        });
        assert_eq!(lookup_at(ADDRESS, &origin).unwrap_err(), UNAVAILABLE);
        server.join().unwrap();
    }
}
