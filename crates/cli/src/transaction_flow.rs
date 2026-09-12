use std::io::{self, Write};

use fresnica_client::{
    AuthorizationScope, AuthorizationThreshold, ClassicOperationKind,
    ExternalEd25519SigningProvider, FresnicaClient, LedgerAuthorizationSnapshot,
    LedgerSignerAvailability, LedgerSignerKind, SystemAuthUnlockProvider,
    LOCAL_SOFTWARE_PASSPHRASE_REQUIRED,
};
use serde_json::{json, Value};
use zeroize::Zeroizing;

pub(crate) use fresnica_client::{network_passphrase, parse_transaction_xdr};

pub fn render_authorization_review(snapshot: &LedgerAuthorizationSnapshot) {
    for line in authorization_review_lines(snapshot) {
        println!("{line}");
    }
}

pub fn authorization_json(snapshot: &LedgerAuthorizationSnapshot) -> Value {
    let accounts = snapshot
        .accounts
        .iter()
        .map(|account| {
            let uses = account
                .uses
                .iter()
                .map(|usage| {
                    json!({
                        "scope": authorization_scope_json(&usage.scope),
                        "threshold": threshold_machine_label(usage.threshold),
                        "required_weight": usage.required_weight,
                    })
                })
                .collect::<Vec<_>>();
            let signers = account
                .signers
                .iter()
                .map(|signer| {
                    json!({
                        "kind": signer_kind_machine_label(&signer.condition.kind),
                        "key": signer.condition.key.as_str(),
                        "weight": signer.weight,
                        "availability": availability_machine_label(signer.availability),
                    })
                })
                .collect::<Vec<_>>();
            json!({
                "account_id": account.account_id.as_str(),
                "required_weight": account.required_weight,
                "satisfied_weight": account.satisfied_weight,
                "local_available_weight": account.local_available_weight,
                "remaining_weight": account.remaining_weight,
                "uses": uses,
                "signers": signers,
            })
        })
        .collect::<Vec<_>>();
    let extra_signers = snapshot
        .extra_signers
        .iter()
        .map(|signer| {
            json!({
                "kind": signer_kind_machine_label(&signer.condition.kind),
                "key": signer.condition.key.as_str(),
                "availability": availability_machine_label(signer.availability),
            })
        })
        .collect::<Vec<_>>();
    json!({
        "status": authorization_status_machine(snapshot),
        "satisfied": snapshot.satisfied,
        "locally_satisfiable": snapshot.locally_satisfiable,
        "transaction_hash": snapshot.transaction_hash.as_str(),
        "accounts": accounts,
        "extra_signers": extra_signers,
    })
}

fn authorization_scope_json(scope: &AuthorizationScope) -> Value {
    match scope {
        AuthorizationScope::TransactionSource => json!({"kind": "transaction_source"}),
        AuthorizationScope::Operation { index, kind } => json!({
            "kind": "operation",
            "operation_index": index,
            "operation": operation_kind_machine_label(*kind),
        }),
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

fn authorization_status_machine(snapshot: &LedgerAuthorizationSnapshot) -> &'static str {
    if snapshot.satisfied {
        "already_satisfied"
    } else if snapshot.locally_satisfiable {
        "local_signing_ready"
    } else {
        "external_authorization_required"
    }
}

fn availability_label(availability: LedgerSignerAvailability) -> &'static str {
    match availability {
        LedgerSignerAvailability::Satisfied => "satisfied",
        LedgerSignerAvailability::LocalEd25519 => "local",
        LedgerSignerAvailability::UnavailableLocally => "external",
    }
}

fn availability_machine_label(availability: LedgerSignerAvailability) -> &'static str {
    match availability {
        LedgerSignerAvailability::Satisfied => "satisfied",
        LedgerSignerAvailability::LocalEd25519 => "local_ed25519",
        LedgerSignerAvailability::UnavailableLocally => "unavailable_locally",
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

fn signer_kind_machine_label(kind: &LedgerSignerKind) -> &'static str {
    match kind {
        LedgerSignerKind::Ed25519PublicKey => "ed25519",
        LedgerSignerKind::PreauthorizedTransaction => "preauth_tx",
        LedgerSignerKind::HashX => "hash_x",
        LedgerSignerKind::Ed25519SignedPayload => "ed25519_signed_payload",
    }
}

fn threshold_label(threshold: AuthorizationThreshold) -> &'static str {
    match threshold {
        AuthorizationThreshold::Low => "low",
        AuthorizationThreshold::Medium => "medium",
        AuthorizationThreshold::High => "high",
    }
}

fn threshold_machine_label(threshold: AuthorizationThreshold) -> &'static str {
    threshold_label(threshold)
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

fn operation_kind_machine_label(kind: ClassicOperationKind) -> &'static str {
    match kind {
        ClassicOperationKind::CreateAccount => "create_account",
        ClassicOperationKind::Payment => "payment",
        ClassicOperationKind::ManageSellOffer => "manage_sell_offer",
        ClassicOperationKind::ManageBuyOffer => "manage_buy_offer",
        ClassicOperationKind::ChangeTrust => "change_trust",
        ClassicOperationKind::ManageData => "manage_data",
        ClassicOperationKind::InvokeHostFunction => "invoke_host_function",
        ClassicOperationKind::BumpSequence => "bump_sequence",
    }
}

pub fn with_software_signer_authorization<T>(
    client: &FresnicaClient,
    mut authorize: impl FnMut(Option<&str>, &[SystemAuthUnlockProvider]) -> Result<T, String>,
) -> Result<T, String> {
    let device_unlock_providers = crate::device_unlock::one_shot_providers(client)?;
    crate::diagnostics::stage("signing: resolve authorization providers");
    if !device_unlock_providers.is_empty() {
        crate::diagnostics::stage("signing: device unlock provider available");
    }
    submit_with_authorization_sources(
        &device_unlock_providers,
        &[],
        || crate::prompt_hidden("Fresnica passphrase: "),
        |passphrase, device_unlock, _| authorize(passphrase, device_unlock),
    )
}

pub fn submit_with_classic_signers<T>(
    client: &FresnicaClient,
    submit: impl FnMut(
        Option<&str>,
        &[SystemAuthUnlockProvider],
        &[ExternalEd25519SigningProvider],
    ) -> Result<T, String>,
) -> Result<T, String> {
    let external_providers = crate::ledger::external_signing_providers(client)?;
    let device_unlock_providers = crate::device_unlock::one_shot_providers(client)?;
    submit_with_authorization_sources(
        &device_unlock_providers,
        &external_providers,
        || crate::prompt_hidden("Fresnica passphrase: "),
        submit,
    )
}

fn submit_with_authorization_sources<T>(
    device_unlock_providers: &[SystemAuthUnlockProvider],
    external_providers: &[ExternalEd25519SigningProvider],
    mut prompt_passphrase: impl FnMut() -> Result<Zeroizing<String>, String>,
    mut submit: impl FnMut(
        Option<&str>,
        &[SystemAuthUnlockProvider],
        &[ExternalEd25519SigningProvider],
    ) -> Result<T, String>,
) -> Result<T, String> {
    match submit(None, device_unlock_providers, external_providers) {
        Err(error) if error == LOCAL_SOFTWARE_PASSPHRASE_REQUIRED => {
            let passphrase = prompt_passphrase()?;
            // A fresh passphrase is the higher-authority fallback. Do not also
            // use Device Unlock for other selected software signers.
            submit(Some(passphrase.as_str()), &[], external_providers).map_err(device_unlock_error)
        }
        result => result.map_err(device_unlock_error),
    }
}

fn device_unlock_error(error: String) -> String {
    error
        .replace("System authentication", "Device unlock")
        .replace("system-auth", "device-unlock")
}

pub fn confirm_submission() -> Result<bool, String> {
    print!("Sign and submit this transaction? [y/N] ");
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
        AccountAuthorizationSnapshot, AuthorizationUse, LedgerSignerCondition, SystemAuthRelease,
        WeightedLedgerSignerSnapshot,
    };

    const SIGNER: &str = "GDLVVGABQKYQVN6VJP7NHSLEA45A5YLS6PNKMIZFV4BBU2HXA5IRVHUR";

    #[test]
    fn passphrase_fallback_drops_device_unlock_providers() {
        let system =
            SystemAuthUnlockProvider::new(SIGNER, |_| SystemAuthRelease::Cancelled).unwrap();
        let mut attempts = 0usize;
        let result = submit_with_authorization_sources(
            &[system],
            &[],
            || Ok(Zeroizing::new("correct horse battery staple".to_owned())),
            |passphrase, device_unlock, external| {
                attempts += 1;
                assert!(external.is_empty());
                if attempts == 1 {
                    assert!(passphrase.is_none());
                    assert_eq!(device_unlock.len(), 1);
                    Err(LOCAL_SOFTWARE_PASSPHRASE_REQUIRED.to_owned())
                } else {
                    assert_eq!(passphrase, Some("correct horse battery staple"));
                    assert!(device_unlock.is_empty());
                    Ok("submitted")
                }
            },
        )
        .unwrap();
        assert_eq!(result, "submitted");
        assert_eq!(attempts, 2);
    }

    #[test]
    fn device_unlock_failure_does_not_silently_downgrade_to_passphrase() {
        let system =
            SystemAuthUnlockProvider::new(SIGNER, |_| SystemAuthRelease::Cancelled).unwrap();
        let error = submit_with_authorization_sources::<()>(
            &[system],
            &[],
            || panic!("provider failure must not prompt for passphrase"),
            |passphrase, device_unlock, _| {
                assert!(passphrase.is_none());
                assert_eq!(device_unlock.len(), 1);
                Err("System authentication for signer failed: cancelled".to_owned())
            },
        )
        .unwrap_err();
        assert_eq!(error, "Device unlock for signer failed: cancelled");
    }

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

        let value = authorization_json(&snapshot);
        assert_eq!(value["status"], "local_signing_ready");
        assert_eq!(
            value["accounts"][0]["uses"][0]["scope"]["operation"],
            "payment"
        );
        assert_eq!(
            value["accounts"][0]["signers"][0]["availability"],
            "local_ed25519"
        );
        assert_eq!(value["transaction_hash"], "0123456789abcdef");
    }
}
