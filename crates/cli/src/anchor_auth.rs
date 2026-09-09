use std::collections::BTreeSet;

use fresnica_client::{
    exchange_anchor_sep10_challenge, prepare_anchor_sep10_challenge, satisfied_ed25519_conditions,
    sep10_authorization_plan, sign_needed_with_ed25519_providers, AnchorCapabilities,
    FresnicaClient, LedgerSignerCondition, LedgerSignerKind, WalletRecord,
};
use zeroize::Zeroizing;

use crate::transaction_flow::{
    network_passphrase, parse_transaction_xdr, submit_with_classic_signers,
};

pub(crate) fn authenticate_anchor_sep10(
    client: &FresnicaClient,
    record: &WalletRecord,
    network: &str,
    home_domain: &str,
    capabilities: &AnchorCapabilities,
) -> Result<Zeroizing<String>, String> {
    crate::diagnostics::stage("anchor SEP-10: fetch account authorization state");
    let ledger_account = client.ledger_account(&record.address)?;
    let authorization = sep10_authorization_plan(ledger_account.as_ref(), &record.address)?;
    crate::diagnostics::stage("anchor SEP-10: request and validate challenge");
    let challenge =
        prepare_anchor_sep10_challenge(network, &record.address, home_domain, capabilities)?;
    let mut envelope = parse_transaction_xdr(challenge.transaction_xdr())?;
    let mut satisfied =
        satisfied_ed25519_conditions(&authorization, &envelope, network_passphrase(network)?)?;
    satisfied.remove(&LedgerSignerCondition {
        kind: LedgerSignerKind::Ed25519PublicKey,
        key: challenge.server_signing_key().to_owned(),
    });
    let excluded = BTreeSet::from([challenge.server_signing_key().to_owned()]);
    crate::diagnostics::stage("anchor SEP-10: sign required local or external conditions");
    submit_with_classic_signers(client, |passcode, system_auth, providers| {
        sign_needed_with_ed25519_providers(
            client.storage(),
            &authorization,
            &satisfied,
            &excluded,
            1,
            network,
            &mut envelope,
            passcode,
            system_auth,
            providers,
        )
    })?;
    crate::diagnostics::stage("anchor SEP-10: exchange signed challenge");
    exchange_anchor_sep10_challenge(network, &challenge, &authorization, &envelope)
}
