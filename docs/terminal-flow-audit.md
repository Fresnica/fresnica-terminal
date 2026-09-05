# Terminal Flow Conformance Audit

Status: active evidence record

Branch: `refactor/terminal-shared-foundation`

Baseline: `main@a742ef3130e455c9cbdbf42378d07f3e1f30153f`

Current shared-source pin: `Fresnica/fresnica@9ba6f23cefe34e8d5940b311ec78f27eed982fe7`

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
- The Rust-client submission chain now borrows the passphrase; CLI and TUI keep ownership at their presentation boundary and do not create ordinary `String` copies for submission.

Classification:

- CLI argv parser, text review, confirmation prompt and hidden passphrase prompt: **A**.
- TUI form, mode transitions, review widget and passphrase dialog: **A**.
- Payment Capability implementation: already shared through `fresnica-client`; **do not wrap it in a new Terminal Flow layer**.
- Passphrase ownership defect: **C, resolved upstream** at `9ba6f23cefe34e8d5940b311ec78f27eed982fe7`.

Conclusion: Payment was the first boundary sample, and the evidence says **no new Terminal shared crate is justified for Payment**. The correct refactor is to consume the existing shared Capability implementation and keep CLI/TUI as independent presentation adapters.

## Trustline

CLI and TUI both construct `TrustlineRequest`, receive `PreparedTrustline`, review it, then call `submit_trustline`.

Classification mirrors Payment:

- CLI parser/render/confirm/prompt: **A**.
- TUI form/state/render/authorization interaction: **A**.
- Trustline semantics and transaction preparation/submission: already shared in `fresnica-client`.
- Passphrase ownership defect: **C, resolved upstream** with the same borrowed submission boundary.

No additional Terminal abstraction is currently justified.

## DEX writes

CLI and TUI both use `OfferRequest`, `PreparedOffer` and `submit_offer` from `fresnica-client`. BUY/SELL orientation, offer operation selection, trustline preflight, reserve/fee checks, exact price semantics and prepared transaction behavior are already outside presentation code.

Classification:

- CLI command parser/review text/confirmation: **A**.
- TUI market/form/state/rendering: **A**.
- DEX Capability behavior: already shared in `fresnica-client`.
- Passphrase ownership defect: **C, resolved upstream** with the same borrowed submission boundary.

No new Terminal DEX service layer should be created merely to make the directory structure look symmetrical.

## Read flows

Account, balance/history and DEX read semantics are already provided by `fresnica-client`; CLI/TUI mainly choose query inputs and render returned models.

Current conclusion: presentation remains local. Shared semantic models/services stay in `fresnica-client` unless later evidence exposes a genuine Terminal-only responsibility.

## Wallet lifecycle

CLI delegates wallet lifecycle behavior to `fresnica_client::wallet` and storage/client APIs while owning terminal prompts, one-time secret/mnemonic presentation and command output.

The CLI wallet presentation has been mechanically isolated into `crates/cli/src/wallet.rs`; this is a presentation-module extraction, not a new semantic layer. Core wallet protection, envelope, signer and passphrase behavior remains in the owning Fresnica layers.

TUI wallet state and rendering still require Phase 5 responsibility review before any structural extraction.

## Anchor

Anchor remains different from Payment/Trustline/DEX:

- substantial SEP-10 / SEP-12 / SEP-24 / SEP-6 orchestration exists in the CLI;
- the Rust TUI does not currently provide the same Anchor product flow;
- RefPython is the normal laboratory for uncertain Anchor orchestration semantics.

Therefore Anchor is **not** a candidate for premature Terminal-wide extraction. Keep the current implementation local until a second Terminal consumer or contract evidence proves a stable common responsibility. If the CLI implementation reveals missing cross-platform semantics, feed that evidence back through RefPython / Capability / ADR rather than declaring the CLI implementation normative.

## Foundation findings and resolution

### 1. Transaction passphrase lifetime — resolved

The original Rust client submission methods accepted owned `String` passphrases, forcing Terminal to create ordinary heap copies from `Zeroizing<String>` / TUI passphrase state. Signing coordination created another owned copy before entering the stable SDK method.

Resolved upstream at `Fresnica/fresnica@9ba6f23cefe34e8d5940b311ec78f27eed982fe7`:

- Rust-client transaction submission/signing APIs borrow `&str`;
- Terminal Payment / Trustline / DEX callers borrow their existing secret buffers;
- TUI zeroizes its passphrase buffer immediately after the submit call returns;
- the stable `FresnicaSdk` public signature remains unchanged;
- the unavoidable owned SDK input is placed under `Zeroizing` before fallible parsing.

This changes Rust implementation ownership, not Fresnica cryptographic semantics or Native/Binding API contracts.

### 2. `fresnica info` duplicate compatibility line — resolved

The extracted CLI `command_info` emits `SDK/Core:   Rust (direct link)` once. `crates/cli/tests/cli_contract.rs` executes the real binary against an isolated home and asserts an exact count of one.

### 3. CLI transaction helper duplication — resolved for the proven case

After write flows moved onto `FresnicaClient::{prepare_*,submit_*}`, `transaction_flow.rs` still retained direct `stellar_xdr` types and a local sign/submit forwarding wrapper. The CLI now delegates transaction parsing and network-passphrase semantics directly to `fresnica-client`; the direct CLI `stellar-xdr` dependency was removed.

No broader parser/framework rewrite was introduced.

### 4. Release marker coupling — resolved as CI semantics

The release workflow previously required the immutable marker for the already-published current version to match every later source pin on any PR. That made ordinary post-release development incompatible with immutable historical release metadata.

PR validation now uses the current source pin without rewriting historical markers. Strict marker/source equality is required when a version changes or a release marker changes; publish still occurs only from a release-marker push to `main`.

## Current architectural conclusion

The original hypothesis was that CLI code would need to be extracted into a new Terminal-local shared Flow library before TUI migration. Phase 0 evidence narrowed that hypothesis:

> For Payment, Trustline and DEX, the shared semantic implementation already exists in `fresnica-client`. CLI and TUI are already mostly presentation adapters. Adding a second shared forwarding layer would duplicate an existing boundary.

The refactor therefore proceeds by **removing proven defects and duplication, not by requiring a new crate**. A Terminal-local shared module/crate will be introduced only if later Wallet/Anchor/TUI evidence shows stable common behavior that neither belongs in presentation nor in Fresnica client/SDK.

## Foundation gates completed

1. Rust-client passphrase-lifetime fix landed upstream at exact SHA `9ba6f23cefe34e8d5940b311ec78f27eed982fe7`.
2. Terminal exact source pin and lockfile moved to that SHA.
3. Payment / Trustline / DEX / TUI submission callers no longer clone the passphrase into ordinary `String` values.
4. Duplicate `info` output is fixed with a real-binary regression test.
5. CLI wallet presentation is isolated without moving wallet semantics out of Fresnica.
6. Direct CLI `stellar-xdr` dependency and redundant sign/submit forwarding were removed.
7. CLI clippy is promoted to a `-D warnings` CI gate.
8. Boundary, formatting, workspace tests, release builds and RefPython CLI compatibility passed on the exact product cleanup before the final PR integration run.

## Next gates

1. Finish CLI responsibility review; do not invent a Payment/Trustline/DEX forwarding layer where `fresnica-client` already owns the behavior.
2. Audit the TUI as `state + event -> effect` presentation code over the same shared client capabilities.
3. Add high-value state/effect tests before mechanical TUI file splitting.
4. Split the TUI only along responsibilities proven by the audit (state/forms/update/render), preserving behavior.
5. Run the full final branch gates, remove all temporary artifacts, then merge once and decide the next Terminal release from the completed compatibility impact.
