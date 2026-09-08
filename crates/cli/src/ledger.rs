use fresnica_client::{ExternalEd25519SigningProvider, FresnicaClient, WalletRecord};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use stellar_ledger::{hd_path::HdPath, Blob as _};
use stellar_xdr::{Hash, TransactionEnvelope};
use tokio::runtime::Builder;

const METADATA_KEY: &str = "fresnica_terminal_ledger";
const METADATA_VERSION: u8 = 1;
const MAX_HD_PATH_INDEX: u32 = (1 << 31) - 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct LedgerConfiguration {
    version: u8,
    pub(crate) hd_path: u32,
}

pub(crate) fn configuration(record: &WalletRecord) -> Result<Option<LedgerConfiguration>, String> {
    let Some(value) = record.metadata.get(METADATA_KEY) else {
        return Ok(None);
    };
    if !record.watch_only() {
        return Err("Ledger signer metadata requires a watch-only wallet".to_owned());
    }
    let configuration: LedgerConfiguration = serde_json::from_value(value.clone())
        .map_err(|error| format!("wallet has invalid Ledger signer metadata: {error}"))?;
    if configuration.version != METADATA_VERSION {
        return Err(format!(
            "wallet uses unsupported Ledger signer metadata version {}",
            configuration.version
        ));
    }
    validate_hd_path(configuration.hd_path)?;
    Ok(Some(configuration))
}

pub(crate) fn attach_configuration(record: &mut WalletRecord, hd_path: u32) -> Result<(), String> {
    validate_hd_path(hd_path)?;
    let value = serde_json::to_value(LedgerConfiguration {
        version: METADATA_VERSION,
        hd_path,
    })
    .map_err(|error| format!("unable to encode Ledger signer metadata: {error}"))?;
    record.metadata.insert(METADATA_KEY.to_owned(), value);
    Ok(())
}

pub(crate) fn detach_configuration(record: &mut WalletRecord) -> Result<bool, String> {
    configuration(record)?;
    Ok(record.metadata.remove(METADATA_KEY).is_some())
}

pub(crate) fn public_key(hd_path: u32) -> Result<String, String> {
    validate_hd_path(hd_path)?;
    let runtime = ledger_runtime()?;
    let signer = stellar_ledger::native().map_err(ledger_error)?;
    let path = HdPath::from(hd_path);
    let public = runtime
        .block_on(signer.get_public_key(&path))
        .map_err(ledger_error)?;
    Ok(format!("{public}"))
}

pub(crate) fn external_signing_providers(
    client: &FresnicaClient,
) -> Result<Vec<ExternalEd25519SigningProvider>, String> {
    let mut providers = Vec::new();
    for record in client.wallets()? {
        let Some(configuration) = configuration(&record)? else {
            continue;
        };
        let expected_public_key = record.address.clone();
        let provider_key = expected_public_key.clone();
        providers.push(ExternalEd25519SigningProvider::new(
            &provider_key,
            move |request| {
                sign_transaction(
                    configuration.hd_path,
                    &expected_public_key,
                    request.transaction_xdr.clone(),
                    &request.network_passphrase,
                )
            },
        )?);
    }
    Ok(providers)
}

fn sign_transaction(
    hd_path: u32,
    expected_public_key: &str,
    transaction_xdr: Vec<u8>,
    network_passphrase: &str,
) -> Result<Vec<u8>, String> {
    validate_hd_path(hd_path)?;
    let envelope = fresnica_client::parse_transaction_xdr(&transaction_xdr)?;
    let TransactionEnvelope::Tx(transaction) = envelope else {
        return Err(
            "Ledger Classic signing currently supports TransactionV1Envelope only".to_owned(),
        );
    };

    let runtime = ledger_runtime()?;
    let signer = stellar_ledger::native().map_err(ledger_error)?;
    let path = HdPath::from(hd_path);
    let live_public_key = runtime
        .block_on(signer.get_public_key(&path))
        .map_err(ledger_error)?;
    let live_public_key = format!("{live_public_key}");
    if live_public_key != expected_public_key {
        return Err(format!(
            "connected Ledger account at m/44'/148'/{hd_path}' is {live_public_key}, expected {expected_public_key}"
        ));
    }

    let network_id = Hash(Sha256::digest(network_passphrase.as_bytes()).into());
    runtime
        .block_on(signer.sign_transaction(path, transaction.tx, network_id))
        .map_err(ledger_error)
}

fn validate_hd_path(hd_path: u32) -> Result<(), String> {
    if hd_path <= MAX_HD_PATH_INDEX {
        Ok(())
    } else {
        Err(format!(
            "Ledger account index must be from 0 to {MAX_HD_PATH_INDEX}"
        ))
    }
}

fn ledger_runtime() -> Result<tokio::runtime::Runtime, String> {
    Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("unable to initialize Ledger runtime: {error}"))
}

fn ledger_error(error: impl std::fmt::Display) -> String {
    format!("Ledger device error: {error}")
}

#[cfg(test)]
mod tests {
    use serde_json::Map;

    use super::*;

    const PUBLIC: &str = "GDLVVGABQKYQVN6VJP7NHSLEA45A5YLS6PNKMIZFV4BBU2HXA5IRVHUR";

    fn watch_record() -> WalletRecord {
        WalletRecord {
            name: "ledger".to_owned(),
            address: PUBLIC.to_owned(),
            wallet_type: "watch-only".to_owned(),
            network: "testnet".to_owned(),
            secret: None,
            metadata: Map::new(),
        }
    }

    #[test]
    fn ledger_metadata_roundtrips_without_changing_wallet_identity() {
        let mut record = watch_record();
        attach_configuration(&mut record, 7).unwrap();

        assert_eq!(configuration(&record).unwrap().unwrap().hd_path, 7);
        assert_eq!(record.address, PUBLIC);
        assert!(record.watch_only());

        assert!(detach_configuration(&mut record).unwrap());
        assert_eq!(configuration(&record).unwrap(), None);
    }

    #[test]
    fn ledger_metadata_rejects_unknown_version_and_out_of_range_index() {
        let mut record = watch_record();
        record.metadata.insert(
            METADATA_KEY.to_owned(),
            serde_json::json!({"version": 2, "hd_path": 0}),
        );
        assert!(configuration(&record).unwrap_err().contains("unsupported"));

        assert!(validate_hd_path(1 << 31).is_err());
    }
}
