use fresnica_client::{fund_testnet_wallet, WalletStorage};

pub fn command_fund(
    storage: &WalletStorage,
    network: &str,
    arguments: &[String],
) -> Result<(), String> {
    let wallet = parse_wallet_option(arguments)?;
    let record = storage.resolve(wallet.as_deref())?;

    crate::diagnostics::stage("Friendbot: request testnet funding");
    let result = fund_testnet_wallet(network, &record)?;
    let mut message = format!("Funded wallet \"{}\" on testnet", record.name);
    if let Some(hash) = result.transaction_hash {
        message.push_str("; transaction ");
        message.push_str(&hash);
    }
    println!("{message}");
    Ok(())
}

fn parse_wallet_option(arguments: &[String]) -> Result<Option<String>, String> {
    match arguments {
        [] => Ok(None),
        [flag, name] if flag == "--wallet" && !name.trim().is_empty() => Ok(Some(name.clone())),
        _ => Err("usage: fresnica wallet testnet-fund [--wallet NAME]".to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wallet_option_matches_python_cli_shape() {
        assert_eq!(parse_wallet_option(&[]).unwrap(), None);
        assert_eq!(
            parse_wallet_option(&["--wallet".to_owned(), "alpha".to_owned()]).unwrap(),
            Some("alpha".to_owned())
        );
    }
}
