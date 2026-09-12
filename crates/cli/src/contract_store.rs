use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use fresnica_client::{ContractExecutableObservation, ContractMetadataEntry};
use serde::{Deserialize, Serialize};
use stellar_strkey::Contract as StrkeyContract;

pub const CONTRACT_STORE_SCHEMA: &str = "fresnica-contract-store-v1";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContractUserMetadata {
    pub name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContractWasmMetadata {
    pub key: String,
    pub value: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContractObservation {
    pub executable: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wasm_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub wasm_meta: Vec<ContractWasmMetadata>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SavedContract {
    pub contract_id: String,
    pub user: ContractUserMetadata,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed: Option<ContractObservation>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct ContractStoreFile {
    schema: String,
    contracts: Vec<SavedContract>,
}

#[derive(Clone, Debug, Deserialize)]
struct LegacyContractAlias {
    name: String,
    contract_id: String,
}

pub struct ContractStore {
    path: PathBuf,
}

impl ContractStore {
    pub fn for_home(home: &Path, network: &str) -> Self {
        Self {
            path: home.join(format!("contracts-{network}.json")),
        }
    }

    pub fn list(&self) -> Result<Vec<SavedContract>, String> {
        let mut contracts = self.load()?;
        contracts.sort_by_key(|contract| name_key(&contract.user.name));
        Ok(contracts)
    }
    pub fn find(&self, name: &str) -> Result<Option<SavedContract>, String> {
        let key = name_key_checked(name)?;
        Ok(self
            .load()?
            .into_iter()
            .find(|contract| name_key(&contract.user.name) == key))
    }

    pub fn add(&self, name: &str, contract_id: &str) -> Result<SavedContract, String> {
        let contract = normalize_contract(name, contract_id, None)?;
        let mut contracts = self.load()?;
        let key = name_key(&contract.user.name);
        if contracts
            .iter()
            .any(|existing| name_key(&existing.user.name) == key)
        {
            return Err(format!(
                "Contract name already exists: {}",
                contract.user.name
            ));
        }
        contracts.push(contract.clone());
        self.save(&contracts)?;
        Ok(contract)
    }

    pub fn remove(&self, name: &str) -> Result<SavedContract, String> {
        let key = name_key_checked(name)?;
        let mut contracts = self.load()?;
        let index = contracts
            .iter()
            .position(|contract| name_key(&contract.user.name) == key)
            .ok_or_else(|| format!("Contract name not found: {name}"))?;
        let removed = contracts.remove(index);
        self.save(&contracts)?;
        Ok(removed)
    }

    pub fn record_observation(
        &self,
        contract_id: &str,
        observation: &ContractExecutableObservation,
        metadata: &[ContractMetadataEntry],
    ) -> Result<(), String> {
        let observed = ContractObservation::from_chain(observation, metadata);
        let mut contracts = self.load()?;
        let mut changed = false;
        for contract in contracts
            .iter_mut()
            .filter(|contract| contract.contract_id == contract_id)
        {
            if contract.observed.as_ref() != Some(&observed) {
                contract.observed = Some(observed.clone());
                changed = true;
            }
        }
        if changed {
            self.save(&contracts)?;
        }
        Ok(())
    }
    pub fn verify_observation(
        &self,
        contract_id: &str,
        observation: &ContractExecutableObservation,
        metadata: &[ContractMetadataEntry],
    ) -> Result<(), String> {
        let observed = ContractObservation::from_chain(observation, metadata);
        let mut contracts = self.load()?;
        let mut changed = false;
        for contract in contracts
            .iter_mut()
            .filter(|contract| contract.contract_id == contract_id)
        {
            match &contract.observed {
                None => {
                    contract.observed = Some(observed.clone());
                    changed = true;
                }
                Some(previous) if previous.same_identity(&observed) => {
                    if previous.wasm_meta != observed.wasm_meta {
                        contract.observed = Some(observed.clone());
                        changed = true;
                    }
                }
                Some(previous) => {
                    return Err(format!(
                        "Saved contract {:?} changed executable from {} to {}. Inspect the deployed contract interface before invoking it again.",
                        contract.user.name,
                        previous.describe(),
                        observed.describe()
                    ));
                }
            }
        }
        if changed {
            self.save(&contracts)?;
        }
        Ok(())
    }
    fn load(&self) -> Result<Vec<SavedContract>, String> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let text = fs::read_to_string(&self.path)
            .map_err(|_| format!("Unable to read contract store: {}", self.path.display()))?;
        let value: serde_json::Value = serde_json::from_str(&text)
            .map_err(|_| format!("Unable to read contract store: {}", self.path.display()))?;
        let raw = if value.is_array() {
            let aliases: Vec<LegacyContractAlias> = serde_json::from_value(value)
                .map_err(|_| "Legacy contract alias store is malformed".to_owned())?;
            aliases
                .into_iter()
                .map(|alias| normalize_contract(&alias.name, &alias.contract_id, None))
                .collect::<Result<Vec<_>, _>>()?
        } else {
            let file: ContractStoreFile = serde_json::from_value(value)
                .map_err(|_| "Contract store is malformed".to_owned())?;
            if file.schema != CONTRACT_STORE_SCHEMA {
                return Err(format!(
                    "Unsupported contract store schema: {}",
                    file.schema
                ));
            }
            file.contracts
        };
        normalize_contracts(raw)
    }
    fn save(&self, contracts: &[SavedContract]) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .map_err(|_| format!("Unable to write contract store: {}", self.path.display()))?;
        }
        let file = ContractStoreFile {
            schema: CONTRACT_STORE_SCHEMA.to_owned(),
            contracts: contracts.to_vec(),
        };
        let text = serde_json::to_string_pretty(&file)
            .map_err(|_| format!("Unable to write contract store: {}", self.path.display()))?
            + "\n";
        let mut temporary_name: OsString = self.path.as_os_str().to_owned();
        temporary_name.push(".tmp");
        let temporary = PathBuf::from(temporary_name);
        let result = (|| {
            fs::write(&temporary, text)
                .map_err(|_| format!("Unable to write contract store: {}", self.path.display()))?;
            restrict_file(&temporary)?;
            #[cfg(windows)]
            if self.path.exists() {
                fs::remove_file(&self.path).map_err(|_| {
                    format!("Unable to write contract store: {}", self.path.display())
                })?;
            }
            fs::rename(&temporary, &self.path)
                .map_err(|_| format!("Unable to write contract store: {}", self.path.display()))?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }
}

impl ContractObservation {
    fn from_chain(
        observation: &ContractExecutableObservation,
        metadata: &[ContractMetadataEntry],
    ) -> Self {
        Self {
            executable: observation.kind.as_str().to_owned(),
            wasm_hash: observation.wasm_hash.clone(),
            wasm_meta: metadata
                .iter()
                .map(|entry| ContractWasmMetadata {
                    key: entry.key.clone(),
                    value: entry.value.clone(),
                })
                .collect(),
        }
    }

    fn same_identity(&self, other: &Self) -> bool {
        self.executable == other.executable && self.wasm_hash == other.wasm_hash
    }

    fn describe(&self) -> String {
        match &self.wasm_hash {
            Some(hash) => format!("{}:{hash}", self.executable),
            None => self.executable.clone(),
        }
    }
}

pub fn looks_like_contract_id(value: &str) -> bool {
    StrkeyContract::from_str(value).is_ok_and(|contract| contract.to_string() == value)
}
fn normalize_contracts(raw: Vec<SavedContract>) -> Result<Vec<SavedContract>, String> {
    let mut contracts = Vec::with_capacity(raw.len());
    let mut seen_names = Vec::with_capacity(raw.len());
    for value in raw {
        let contract = normalize_contract(
            &value.user.name,
            &value.contract_id,
            value.observed.as_ref(),
        )?;
        let key = name_key(&contract.user.name);
        if seen_names.iter().any(|item| item == &key) {
            return Err("Contract store contains duplicate names".to_owned());
        }
        seen_names.push(key);
        contracts.push(contract);
    }
    Ok(contracts)
}

fn normalize_contract(
    name: &str,
    contract_id: &str,
    observed: Option<&ContractObservation>,
) -> Result<SavedContract, String> {
    let name = normalize_name(name)?;
    let contract_id = contract_id.trim();
    if !looks_like_contract_id(contract_id) {
        return Err("Contract id must be a Stellar C... address".to_owned());
    }
    Ok(SavedContract {
        contract_id: contract_id.to_owned(),
        user: ContractUserMetadata { name },
        observed: observed.map(normalize_observation).transpose()?,
    })
}

fn normalize_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty()
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err("Contract name must use letters, numbers, '.', '-' or '_'".to_owned());
    }
    if matches!(
        name.to_ascii_lowercase().as_str(),
        "add" | "list" | "remove" | "invoke"
    ) {
        return Err(format!("Contract name is reserved: {name}"));
    }
    Ok(name.to_owned())
}

fn normalize_observation(observed: &ContractObservation) -> Result<ContractObservation, String> {
    let executable = observed.executable.trim().to_ascii_lowercase();
    if !matches!(
        executable.as_str(),
        "stellar_asset" | "wasm" | "external_ref"
    ) {
        return Err("Contract observation has an unknown executable type".to_owned());
    }
    let wasm_hash = observed
        .wasm_hash
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_ascii_lowercase);
    match executable.as_str() {
        "stellar_asset" if wasm_hash.is_some() || !observed.wasm_meta.is_empty() => {
            return Err(
                "Stellar Asset Contract observation must not contain Wasm facts".to_owned(),
            );
        }
        "wasm" | "external_ref" => {
            let Some(hash) = wasm_hash.as_deref() else {
                return Err("Contract Wasm observation is missing its hash".to_owned());
            };
            if hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                return Err("Contract Wasm observation has an invalid hash".to_owned());
            }
        }
        _ => {}
    }
    Ok(ContractObservation {
        executable,
        wasm_hash,
        wasm_meta: observed.wasm_meta.clone(),
    })
}

fn name_key_checked(name: &str) -> Result<String, String> {
    if name.trim().is_empty() {
        return Err("Contract name cannot be empty".to_owned());
    }
    Ok(name_key(name))
}
fn name_key(name: &str) -> String {
    name.trim().to_ascii_lowercase()
}

#[cfg(unix)]
fn restrict_file(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .map_err(|_| "Unable to protect contract store file".to_owned())
}

#[cfg(not(unix))]
fn restrict_file(_path: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use fresnica_client::{
        ContractExecutableKind, ContractExecutableObservation, ContractMetadataEntry,
    };
    use serde_json::json;

    use super::*;

    const CONTRACT: &str = "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAD2KM";

    fn store(label: &str, network: &str) -> ContractStore {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        ContractStore::for_home(
            &std::env::temp_dir().join(format!(
                "fresnica-contract-store-{label}-{}-{nonce}",
                std::process::id()
            )),
            network,
        )
    }

    fn wasm(hash_byte: u8) -> ContractExecutableObservation {
        ContractExecutableObservation {
            kind: ContractExecutableKind::Wasm,
            wasm_hash: Some(format!("{hash_byte:02x}").repeat(32)),
        }
    }

    fn meta(key: &str, value: &str) -> ContractMetadataEntry {
        ContractMetadataEntry {
            key: key.to_owned(),
            value: value.to_owned(),
        }
    }

    #[test]
    fn contracts_are_case_insensitive_and_network_scoped() {
        let home = store("scope", "mainnet")
            .path
            .parent()
            .unwrap()
            .to_path_buf();
        let mainnet = ContractStore::for_home(&home, "mainnet");
        let testnet = ContractStore::for_home(&home, "testnet");
        mainnet.add("Aqua", CONTRACT).unwrap();
        assert_eq!(mainnet.find("aqua").unwrap().unwrap().contract_id, CONTRACT);
        assert!(testnet.find("aqua").unwrap().is_none());
        assert!(mainnet.add("AQUA", CONTRACT).is_err());
    }
    #[test]
    fn contracts_reject_reserved_names_and_invalid_contract_ids() {
        let store = store("validation", "testnet");
        assert!(store.add("invoke", CONTRACT).is_err());
        assert!(store.add("add", CONTRACT).is_err());
        assert!(store.add("list", CONTRACT).is_err());
        assert!(store.add("remove", CONTRACT).is_err());
        assert!(store.add("bad name", CONTRACT).is_err());
        assert!(store.add("aqua", "not-a-contract").is_err());
    }

    #[test]
    fn legacy_alias_store_upgrades_on_first_write() {
        let store = store("legacy", "testnet");
        fs::create_dir_all(store.path.parent().unwrap()).unwrap();
        fs::write(
            &store.path,
            serde_json::to_string_pretty(&json!([{
                "name": "Aqua",
                "contract_id": CONTRACT
            }]))
            .unwrap(),
        )
        .unwrap();

        let loaded = store.list().unwrap();
        assert_eq!(loaded[0].user.name, "Aqua");
        assert!(loaded[0].observed.is_none());

        store
            .record_observation(CONTRACT, &wasm(0xab), &[meta("binver", "2.3.7")])
            .unwrap();
        let upgraded: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&store.path).unwrap()).unwrap();
        assert_eq!(upgraded["schema"], CONTRACT_STORE_SCHEMA);
        assert_eq!(upgraded["contracts"][0]["observed"]["executable"], "wasm");
        assert_eq!(
            upgraded["contracts"][0]["observed"]["wasm_meta"][0]["key"],
            "binver"
        );
    }
    #[test]
    fn version_one_observation_without_metadata_remains_readable() {
        let store = store("v1-no-meta", "testnet");
        fs::create_dir_all(store.path.parent().unwrap()).unwrap();
        fs::write(
            &store.path,
            serde_json::to_string_pretty(&json!({
                "schema": CONTRACT_STORE_SCHEMA,
                "contracts": [{
                    "contract_id": CONTRACT,
                    "user": {"name": "Aqua"},
                    "observed": {
                        "executable": "wasm",
                        "wasm_hash": "11".repeat(32)
                    }
                }]
            }))
            .unwrap(),
        )
        .unwrap();

        let observed = store.find("aqua").unwrap().unwrap().observed.unwrap();
        assert!(observed.wasm_meta.is_empty());
        assert_eq!(observed.wasm_hash.unwrap(), "11".repeat(32));
    }

    #[test]
    fn same_wasm_enriches_legacy_observation_metadata_without_upgrade_error() {
        let store = store("metadata", "testnet");
        store.add("Aqua", CONTRACT).unwrap();
        store
            .verify_observation(CONTRACT, &wasm(0x11), &[])
            .unwrap();

        store
            .verify_observation(
                CONTRACT,
                &wasm(0x11),
                &[meta("sep", "41"), meta("home_domain", "example.org")],
            )
            .unwrap();
        let observed = store.find("aqua").unwrap().unwrap().observed.unwrap();
        assert_eq!(observed.wasm_meta.len(), 2);
        assert_eq!(observed.wasm_meta[0].key, "sep");
        assert_eq!(observed.wasm_meta[1].value, "example.org");
    }

    #[test]
    fn changed_wasm_fails_closed_until_explicit_inspection_refreshes_store() {
        let store = store("upgrade", "testnet");
        store.add("Aqua", CONTRACT).unwrap();

        store
            .verify_observation(CONTRACT, &wasm(0x11), &[])
            .unwrap();
        let before = store.find("aqua").unwrap().unwrap();
        let first_hash = "11".repeat(32);
        assert_eq!(
            before.observed.as_ref().unwrap().wasm_hash.as_deref(),
            Some(first_hash.as_str())
        );

        let error = store
            .verify_observation(CONTRACT, &wasm(0x22), &[meta("binver", "3.0.0")])
            .unwrap_err();
        assert!(error.contains("changed executable"));
        assert!(error.contains("Inspect the deployed contract interface"));
        let unchanged = store.find("aqua").unwrap().unwrap();
        assert_eq!(unchanged.observed, before.observed);

        store
            .record_observation(CONTRACT, &wasm(0x22), &[meta("binver", "3.0.0")])
            .unwrap();
        store
            .verify_observation(CONTRACT, &wasm(0x22), &[meta("binver", "3.0.0")])
            .unwrap();
        assert_eq!(
            store
                .find("aqua")
                .unwrap()
                .unwrap()
                .observed
                .unwrap()
                .wasm_hash
                .unwrap(),
            "22".repeat(32)
        );
    }
}
