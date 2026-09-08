use std::io::{self, Write};

use fresnica_client::{
    AuthorizationScope, AuthorizationThreshold, ClassicOperationKind,
    ExternalEd25519SigningProvider, FresnicaClient, LedgerAuthorizationSnapshot,
    LedgerSignerAvailability, LedgerSignerKind,
};

pub(crate) use fresnica_client::{network_passphrase, parse_transaction_xdr};

pub fn render_authorization_review(snapshot: &LedgerAuthorizationSnapshot) {
    for line in authorization_review_lines(snapshot) {
        println!("{line}");
    }
}

fn authorization_review_lines(snapshot: &LedgerAuthorizationSnapshot) -> Vec<String> {
    let mut lines = vec![format!("Authorization: {}", authorization_status(snapshot))];
    for account in &snapshot.accounts {
        lines.push(format!(
            "  {}: required {} · satisfied {} · local {} · remaining {}",
            account.account_id,
            account.required_weight,
            account.satisfied_weight,
            account.local_available_weight,
            account.remaining_weight
        ));
        for usage in &account.uses {
            lines.push(format!(
                "    {}: {} threshold {}",
                scope_label(&usage.scope),
                threshold_label(usage.threshold),
                usage.required_weight
            ));
        }
        for signer in &account.signers {
            lines.push(format!(
                "    {} {} {} (weight {})",
                availability_label(signer.availability),
                signer_kind_label(&signer.condition.kind),
                signer.condition.key,
                signer.weight
            ));
        }
    }
    for signer in &snapshot.extra_signers {
        lines.push(format!(
            "  extra: {} {} {}",
            availability_label(signer.availability),
            signer_kind_label(&signer.condition.kind),
            signer.condition.key
        ));
    }
    lines.push(format!("Prepared tx: {}", snapshot.transaction_hash));
    lines
}

fn authorization_status(snapshot: &LedgerAuthorizationSnapshot) -> &'static str {
    if snapshot.satisfied {
        "already satisfied"
    } else if snapshot.locally_satisfiable {
        "local signing ready"
    } else {
        "external authorization required"
    }
}

fn availability_label(availability: LedgerSignerAvailability) -> &'static str {
    match availability {
        LedgerSignerAvailability::Satisfied => "satisfied",
        LedgerSignerAvailability::LocalEd25519 => "local",
        LedgerSignerAvailability::UnavailableLocally => "external",
    }
}

fn signer_kind_label(kind: &LedgerSignerKind) -> &'static str {
    match kind {
        LedgerSignerKind::Ed25519PublicKey => "Ed25519",
        LedgerSignerKind::PreauthorizedTransaction => "preauth-tx",
        LedgerSignerKind::HashX => "Hash-X",
        LedgerSignerKind::Ed25519SignedPayload => "signed-payload",
    }
}

fn threshold_label(threshold: AuthorizationThreshold) -> &'static str {
    match threshold {
        AuthorizationThreshold::Low => "low",
        AuthorizationThreshold::Medium => "medium",
        AuthorizationThreshold::High => "high",
    }
}

fn scope_label(scope: &AuthorizationScope) -> String {
    match scope {
        AuthorizationScope::TransactionSource => "transaction source".to_owned(),
        AuthorizationScope::Operation { index, kind } => {
            format!("operation {} {}", index + 1, operation_kind_label(*kind))
        }
    }
}

fn operation_kind_label(kind: ClassicOperationKind) -> &'static str {
    match kind {
        ClassicOperationKind::CreateAccount => "CreateAccount",
        ClassicOperationKind::Payment => "Payment",
        ClassicOperationKind::ManageSellOffer => "ManageSellOffer",
        ClassicOperationKind::ManageBuyOffer => "ManageBuyOffer",
        ClassicOperationKind::ChangeTrust => "ChangeTrust",
        ClassicOperationKind::ManageData => "ManageData",
        ClassicOperationKind::InvokeHostFunction => "InvokeHostFunction",
        ClassicOperationKind::BumpSequence => "BumpSequence",
    }
}

const LOCAL_PASSPHRASE_REQUIRED: &str =
    "Fresnica passphrase is required for selected local software signers";

pub fn submit_with_classic_signers<T>(
    client: &FresnicaClient,
    mut submit: impl FnMut(Option<&str>, &[ExternalEd25519SigningProvider]) -> Result<T, String>,
) -> Result<T, String> {
    let providers = crate::ledger::external_signing_providers(client)?;
    match submit(None, &providers) {
        Err(error) if error == LOCAL_PASSPHRASE_REQUIRED => {
            let passcode = crate::prompt_hidden("Fresnica passphrase: ")?;
            submit(Some(passcode.as_str()), &providers)
        }
        result => result,
    }
}

pub fn confirm_submission() -> Result<bool, String> {
    print!("Submit this transaction? [y/N] ");
    io::stdout()
        .flush()
        .map_err(|error| format!("unable to write prompt: {error}"))?;
    let mut answer = String::new();
    io::stdin()
        .read_line(&mut answer)
        .map_err(|error| format!("unable to read confirmation: {error}"))?;
    Ok(matches!(
        answer.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use fresnica_client::{
        AccountAuthorizationSnapshot, AuthorizationUse, LedgerSignerCondition,
        WeightedLedgerSignerSnapshot,
    };

    #[test]
    fn authorization_review_explains_transaction_specific_local_capacity() {
        let snapshot = LedgerAuthorizationSnapshot {
            transaction_hash: "0123456789abcdef".to_owned(),
            accounts: vec![AccountAuthorizationSnapshot {
                account_id: "GACCOUNT".to_owned(),
                required_weight: 2,
                satisfied_weight: 1,
                local_available_weight: 1,
                remaining_weight: 0,
                uses: vec![AuthorizationUse {
                    scope: AuthorizationScope::Operation {
                        index: 0,
                        kind: ClassicOperationKind::Payment,
                    },
                    threshold: AuthorizationThreshold::Medium,
                    required_weight: 2,
                }],
                signers: vec![WeightedLedgerSignerSnapshot {
                    condition: LedgerSignerCondition {
                        kind: LedgerSignerKind::Ed25519PublicKey,
                        key: "GSIGNER".to_owned(),
                    },
                    weight: 1,
                    availability: LedgerSignerAvailability::LocalEd25519,
                }],
            }],
            extra_signers: Vec::new(),
            satisfied: false,
            locally_satisfiable: true,
        };

        let lines = authorization_review_lines(&snapshot);

        assert_eq!(lines[0], "Authorization: local signing ready");
        assert!(lines
            .iter()
            .any(|line| line.contains("required 2 · satisfied 1 · local 1 · remaining 0")));
        assert!(lines
            .iter()
            .any(|line| line == "    operation 1 Payment: medium threshold 2"));
        assert!(lines
            .iter()
            .any(|line| line == "    local Ed25519 GSIGNER (weight 1)"));
        assert_eq!(lines.last().unwrap(), "Prepared tx: 0123456789abcdef");
    }
}
