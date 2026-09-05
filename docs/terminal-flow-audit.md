# Terminal Flow Conformance Audit

Status: active evidence record

Branch: `refactor/terminal-shared-foundation`

Baseline: `main@a742ef3130e455c9cbdbf42378d07f3e1f30153f`

This audit records evidence discovered while implementing the shared-foundation refactor. It does not define Fresnica semantics; Fresnica Capability / Flow / Security contracts remain authoritative and RefPython remains the executable laboratory for uncertain product semantics.

## Placement rule

For each Terminal responsibility:

- **A — presentation / I/O:** keep in CLI or TUI;
- **B — Terminal-local shared implementation:** extract only when CLI and TUI demonstrate a real common responsibility not already owned by `fresnica-client` / `fresnica-sdk`;
- **C — Fresnica upstream concern:** fix or clarify in the owning Fresnica contract/client/SDK layer first.

A new Terminal shared crate is not a goal by itself. If the existing Fresnica Rust client already provides the correct shared boundary, adding another forwarding layer is a regression in architecture rather than progress.

## Payment

Normative Fresnica shape:

```text
intent -> capability preparation -> immutable semantic review
       -> product confirmation -> authorization/signing
       -> exact prepared transaction submission -> result
```

Current Terminal evidence:

- CLI converts argv into `PaymentRequest`, calls `FresnicaClient::prepare_payment`, renders `PaymentReview`, asks for confirmation/passphrase, then calls `submit_payment`.
- TUI converts form state into the same `PaymentRequest`, calls the same `prepare_payment`, stores the same `PreparedPayment`, renders its review, asks for authorization, then calls the same `submit_payment`.
- Transaction construction, destination/CreateAccount selection, trustline/reserve checks, immutable prepared envelope and submission semantics already live in `fresnica-client`.

Classification:

- CLI argv parser, text review, confirmation prompt and hidden passphrase prompt: **A**.
- TUI form, mode transitions, review widget and passphrase dialog: **A**.
- Payment Capability implementation: already shared through `fresnica-client`; **do not wrap it in a new Terminal Flow layer**.
- Passphrase ownership currently requires CLI/TUI to create ordinary `String` copies before Rust-client submission: **C**, because the Rust client submission API owns the string unnecessarily.

Conclusion: Payment is the first extraction sample, but the evidence currently says **no new shared crate is justified for Payment**. The correct refactor is to repair the upstream secret boundary and keep CLI/TUI as independent presentation adapters over the existing shared Capability implementation.

## Trustline

Current CLI and TUI both construct `TrustlineRequest`, receive `PreparedTrustline`, review it, then call `submit_trustline`.

Classification mirrors Payment:

- CLI parser/render/confirm/prompt: **A**.
- TUI form/state/render/authorization interaction: **A**.
- Trustline semantics and transaction preparation/submission: already shared in `fresnica-client`.
- Owned passphrase at submission: **C**, same upstream Rust-client defect.

No additional Terminal abstraction is currently justified.

## DEX writes

Current CLI and TUI both use `OfferRequest`, `PreparedOffer` and `submit_offer` from `fresnica-client`. BUY/SELL orientation, offer operation selection, trustline preflight, reserve/fee checks, exact price semantics and prepared transaction behavior are already outside presentation code.

Classification:

- CLI command parser/review text/confirmation: **A**.
- TUI market/form/state/rendering: **A**.
- DEX Capability behavior: already shared in `fresnica-client`.
- Owned passphrase at submission: **C**, same upstream Rust-client defect.

No new Terminal DEX service layer should be created merely to make the directory structure look symmetrical.

## Read flows

Account, balance/history and DEX read semantics are already provided by `fresnica-client`; CLI/TUI mainly choose query inputs and render returned models.

Current conclusion: presentation remains local. Shared semantic models/services stay in `fresnica-client` unless later evidence exposes a genuine Terminal-only responsibility.

## Wallet lifecycle

CLI delegates core wallet lifecycle behavior to `fresnica_client::wallet` and storage/client APIs while owning terminal prompts, one-time secret/mnemonic presentation and command output.

This area still needs detailed Phase 3 review before any extraction. In particular, Terminal must not move cryptographic/envelope/passphrase semantics out of the owning Fresnica layers merely to share CLI/TUI code.

## Anchor

Anchor is different from Payment/Trustline/DEX:

- substantial SEP-10 / SEP-12 / SEP-24 / SEP-6 orchestration currently exists in the CLI;
- the Rust TUI does not currently provide the same Anchor product flow;
- RefPython is the normal laboratory for uncertain Anchor orchestration semantics.

Therefore Anchor is **not** a candidate for premature Terminal-wide extraction. Keep the current implementation local until a second Terminal consumer or contract evidence proves a stable common responsibility. If the CLI implementation reveals missing cross-platform semantics, feed that evidence back through RefPython / Capability / ADR rather than declaring the CLI implementation normative.

## Foundation defects confirmed

### 1. Transaction passphrase lifetime

The current Rust client submission methods accept owned `String` passphrases. Terminal therefore creates ordinary heap copies from its `Zeroizing<String>` / TUI passphrase state before submission. The signing-coordination path creates another owned copy before entering the stable SDK method.

Planned correction is deliberately narrow:

- Rust-client transaction submission/signing APIs borrow `&str`;
- Terminal callers stop cloning/`to_owned()` at the client boundary;
- the stable `FresnicaSdk` public method signature remains unchanged;
- at the unavoidable SDK ownership boundary, the SDK must put the owned passphrase under `Zeroizing` before any fallible envelope parsing.

This changes Rust implementation ownership, not Fresnica cryptographic semantics or Native/Binding API contracts.

### 2. `fresnica info` duplicate compatibility line

`command_info` currently prints `SDK/Core:   Rust (direct link)` twice. This is a user-visible regression that existing compatibility tests did not catch because they only asserted presence.

Fix locally in Terminal and add an exact-count CLI contract test.

## Current architectural conclusion

The original hypothesis was that CLI code would need to be extracted into a new Terminal-local shared Flow library before TUI migration. Phase 0 evidence narrows that hypothesis:

> For Payment, Trustline and DEX, the shared semantic implementation already exists in `fresnica-client`. CLI and TUI are already mostly presentation adapters. Adding a second shared forwarding layer would duplicate an existing boundary.

The refactor therefore proceeds by **removing proven defects and duplication, not by requiring a new crate**. A Terminal-local shared module/crate will be introduced only if later Wallet/Anchor/TUI evidence shows stable common behavior that neither belongs in presentation nor in Fresnica client/SDK.

## Next gates

1. Land the Rust-client passphrase-lifetime fix on its own Fresnica branch and validate it.
2. Update the Terminal refactor branch to the exact fixed Fresnica revision.
3. Remove Terminal passphrase copies and fix duplicate `info` output with a regression test.
4. Run full Terminal boundary/format/test/build/RefPython compatibility gates.
5. Continue CLI audit/hardening before any structural TUI migration.
