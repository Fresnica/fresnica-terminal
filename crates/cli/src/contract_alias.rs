use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use stellar_strkey::Contract as StrkeyContract;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContractAlias {
    pub name: String,
    pub contract_id: String,
}

pub struct ContractAliasStore {
    path: PathBuf,
}

impl ContractAliasStore {
    pub fn for_home(home: &Path, network: &str) -> Self {
        Self {
            path: home.join(format!("contracts-{network}.json")),
        }
    }

    pub fn list(&self) -> Result<Vec<ContractAlias>, String> {
        let mut aliases = self.load()?;
        aliases.sort_by_key(|alias| name_key(&alias.name));
        Ok(aliases)
    }

    pub fn find(&self, name: &str) -> Result<Option<ContractAlias>, String> {
        let key = name_key_checked(name)?;
        Ok(self
            .load()?
            .into_iter()
            .find(|alias| name_key(&alias.name) == key))
    }

    pub fn add(&self, name: &str, contract_id: &str) -> Result<ContractAlias, String> {
        let alias = normalize_alias(name, contract_id)?;
        let mut aliases = self.load()?;
        let key = name_key(&alias.name);
        if aliases
            .iter()
            .any(|existing| name_key(&existing.name) == key)
        {
            return Err(format!("Contract alias already exists: {}", alias.name));
        }
        aliases.push(alias.clone());
        self.save(&aliases)?;
        Ok(alias)
    }

    pub fn remove(&self, name: &str) -> Result<ContractAlias, String> {
        let key = name_key_checked(name)?;
        let mut aliases = self.load()?;
        let index = aliases
            .iter()
            .position(|alias| name_key(&alias.name) == key)
            .ok_or_else(|| format!("Contract alias not found: {name}"))?;
        let removed = aliases.remove(index);
        self.save(&aliases)?;
        Ok(removed)
    }

    fn load(&self) -> Result<Vec<ContractAlias>, String> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let text = fs::read_to_string(&self.path)
            .map_err(|_| format!("Unable to read contract aliases: {}", self.path.display()))?;
        let raw: Vec<ContractAlias> = serde_json::from_str(&text)
            .map_err(|_| format!("Unable to read contract aliases: {}", self.path.display()))?;
        let mut aliases = Vec::with_capacity(raw.len());
        let mut seen = Vec::with_capacity(raw.len());
        for value in raw {
            let alias = normalize_alias(&value.name, &value.contract_id)
                .map_err(|_| "Contract alias store is malformed".to_owned())?;
            let key = name_key(&alias.name);
            if seen.iter().any(|item| item == &key) {
                return Err("Contract alias store contains duplicate names".to_owned());
            }
            seen.push(key);
            aliases.push(alias);
        }
        Ok(aliases)
    }

    fn save(&self, aliases: &[ContractAlias]) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|_| {
                format!("Unable to write contract aliases: {}", self.path.display())
            })?;
        }
        let text = serde_json::to_string_pretty(aliases)
            .map_err(|_| format!("Unable to write contract aliases: {}", self.path.display()))?
            + "\n";
        let mut temporary_name: OsString = self.path.as_os_str().to_owned();
        temporary_name.push(".tmp");
        let temporary = PathBuf::from(temporary_name);
        let result = (|| {
            fs::write(&temporary, text).map_err(|_| {
                format!("Unable to write contract aliases: {}", self.path.display())
            })?;
            restrict_file(&temporary)?;
            #[cfg(windows)]
            if self.path.exists() {
                fs::remove_file(&self.path).map_err(|_| {
                    format!("Unable to write contract aliases: {}", self.path.display())
                })?;
            }
            fs::rename(&temporary, &self.path).map_err(|_| {
                format!("Unable to write contract aliases: {}", self.path.display())
            })?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }
}

pub fn looks_like_contract_id(value: &str) -> bool {
    StrkeyContract::from_str(value).is_ok_and(|contract| contract.to_string() == value)
}

fn normalize_alias(name: &str, contract_id: &str) -> Result<ContractAlias, String> {
    let name = name.trim();
    if name.is_empty()
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err("Contract alias must use letters, numbers, '.', '-' or '_'".to_owned());
    }
    if matches!(
        name.to_ascii_lowercase().as_str(),
        "add" | "list" | "remove" | "invoke"
    ) {
        return Err(format!("Contract alias is reserved: {name}"));
    }
    let contract_id = contract_id.trim();
    if !looks_like_contract_id(contract_id) {
        return Err("Contract id must be a Stellar C... address".to_owned());
    }
    Ok(ContractAlias {
        name: name.to_owned(),
        contract_id: contract_id.to_owned(),
    })
}

fn name_key_checked(name: &str) -> Result<String, String> {
    if name.trim().is_empty() {
        return Err("Contract alias cannot be empty".to_owned());
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
        .map_err(|_| "Unable to protect contract alias file".to_owned())
}

#[cfg(not(unix))]
fn restrict_file(_path: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    const CONTRACT: &str = "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAD2KM";

    fn store(label: &str, network: &str) -> ContractAliasStore {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        ContractAliasStore::for_home(
            &std::env::temp_dir().join(format!(
                "fresnica-contract-alias-{label}-{}-{nonce}",
                std::process::id()
            )),
            network,
        )
    }

    #[test]
    fn aliases_are_case_insensitive_and_network_scoped() {
        let home = store("scope", "mainnet")
            .path
            .parent()
            .unwrap()
            .to_path_buf();
        let mainnet = ContractAliasStore::for_home(&home, "mainnet");
        let testnet = ContractAliasStore::for_home(&home, "testnet");
        mainnet.add("Aqua", CONTRACT).unwrap();
        assert_eq!(mainnet.find("aqua").unwrap().unwrap().contract_id, CONTRACT);
        assert!(testnet.find("aqua").unwrap().is_none());
        assert!(mainnet.add("AQUA", CONTRACT).is_err());
    }

    #[test]
    fn aliases_reject_reserved_names_and_invalid_contract_ids() {
        let store = store("validation", "testnet");
        assert!(store.add("invoke", CONTRACT).is_err());
        assert!(store.add("add", CONTRACT).is_err());
        assert!(store.add("list", CONTRACT).is_err());
        assert!(store.add("remove", CONTRACT).is_err());
        assert!(store.add("bad name", CONTRACT).is_err());
        assert!(store.add("aqua", "not-a-contract").is_err());
    }
}
