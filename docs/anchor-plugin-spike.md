# Anchor native-plugin consumer

Status: **Validated native consumer; host wire remains experimental, not a frozen plugin SDK**
Last verified: 2026-09-08

## Purpose

Anchor is the first real `fresnica-*` consumer. It exists to discover the minimum
wallet/plugin interaction contract from a real Stellar workflow rather than designing
a generic plugin ABI first.

The product command remains:

```text
fresnica anchor ...
        |
        v
fresnica-anchor
```

Anchor protocol/product orchestration lives in the external `fresnica-anchor`
executable. Fresnica remains the wallet, authorization, review, signer-selection,
secret-handling and submission authority.

## Exact checkpoint

- Terminal branch: `feat/terminal-anchor-plugin-parity`.
- validated product commit: `f700075628ae0381d0f5e77604eca1f6041ff292`.
- product tree: `f5cf5dc59c32096071ed7a922547ada575b10582`.
- parent spike/docs checkpoint: `09d7da83f087a5bb707c8e12ec16af8d48efd311`.
- pinned Fresnica source: `a43ae377eea9346f48151c4d5ef596717fed2454`.
- upstream branch: `feat/rust-client-anchor-explicit-domain`.
- no Main merge, product PR, GitHub CI run, or release workflow was triggered for
  this parity milestone; VPS-local deterministic validation is the evidence source.

The upstream consumer-driven additions are deliberately small:

1. explicit-home-domain Anchor discovery while preserving issuer-derived discovery;
2. bounded provider-aware Ed25519 signing with satisfied/excluded/minimum-signature
   controls for SEP-10;
3. a read-only payment receive preflight that reuses the existing Payment capability's
   trustline authorization and receiving-capacity semantics.

## Runtime model

Stellar-compatible `stellar-*` / legacy `soroban-*` children keep the Stellar CLI
external-command behavior and receive no Fresnica-native host context.

A `fresnica-*` child receives only experimental process coordination:

```text
FRESNICA_PLUGIN_API=1
FRESNICA_PLUGIN_HOST=<absolute Fresnica executable>
FRESNICA_PLUGIN_NETWORK=<selected network>
FRESNICA_HOME=<selected Fresnica home>
FRESNICA_HORIZON_URL=<explicit override, when present>
FRESNICA_RPC_URL=<explicit override, when present>
```

These values are not signer capabilities. `FRESNICA_PLUGIN_API=1` is a protocol
marker, not a credential.

The Anchor consumer currently proves four bounded host operations:

```text
__plugin-host context [--wallet NAME]
__plugin-host anchor-receive ASSET [--wallet NAME]
__plugin-host anchor-auth ASSET --home-domain DOMAIN [--wallet NAME]
__plugin-host anchor-payment
```

- `context` exposes selected public wallet/network identity only.
- `anchor-receive` is read-only and checks trustline presence, full authorization and
  at least one stroop of receiving capacity using shared Payment semantics.
- `anchor-auth` performs SEP-10 challenge validation/signing/exchange inside Fresnica
  and returns only the short-lived Anchor bearer token.
- `anchor-payment` accepts a bounded payment proposal but Fresnica reconstructs the
  payment, renders its normal semantic review, selects its own signer and requires
  interactive confirmation. A native plugin cannot use `-y`/`--yes` to bypass this
  review.

There is still no generic host RPC, `sign-xdr`, signer-provider bridge, private-key
access, mnemonic access, Fresnica-passphrase access, decrypted-wallet access or
opened Ledger/HSM handle.

## Signing boundary

SEP-10 stays fully Fresnica-owned:

```text
resolve wallet / authorization state
        |
fetch + validate SEP-10 challenge
        |
exclude Anchor server signing key
        |
require at least one client Ed25519 proof
        |
Fresnica Signing Coordination
  -> protected software signer and/or external provider (Ledger)
  -> SDK verifies returned signature
        |
exchange signed challenge
        |
short-lived SEP-10 bearer token
        |
fresnica-anchor
```

The plugin never learns which local signer satisfied the request. Provider-level tests
cover external-provider selection, excluded keys and minimum-signature proof. A
physical Ledger was not attached to this VPS, so physical-device SEP-10 remains a
separate acceptance item.

## Command parity

`fresnica-anchor` now owns the complete previous Anchor command surface:

- explicit-domain `discover`;
- SEP-10 `auth`;
- `deposit` and `withdraw` with shared SEP-24-first / SEP-6-fallback selection;
- `status` with explicit SEP-24/SEP-6 selection when the Anchor advertises both;
- SEP-12 `customer` GET/PUT including scalar and file fields;
- withdrawal payment handoff through mandatory Fresnica-hosted interactive review.

The old CLI built-in `crates/cli/src/anchor.rs` has been removed. `anchor` now follows
the normal unknown-command resolver and reaches `fresnica-anchor`; there is no special
Anchor shadow/dispatch path left in the CLI.

Machine-output compatibility keeps the previous public field names (`domain` for
transfer/status and `anchor` for SEP-12 customer output). Explicit `home_domain` is an
input/identity improvement, not an excuse to churn the existing machine schema.

## Official Testnet evidence

Reference Anchor:

```text
home domain: testanchor.stellar.org
asset: SRT:GCDNJUBQSX7AJWLJACMJ7I4BC3Z47BQUTMHEICZLE6MU4KQBRYG5JY6B
reference UI: anchor-ref-ui-testanchor.stellar.org
```

### Discovery and SEP-10

A real `fresnica anchor ...` invocation resolved the PATH `fresnica-anchor`, discovered
live SEP-10/SEP-24/SEP-6/SEP-12 capabilities and completed SEP-10 challenge signing
and token exchange through the Fresnica host.

### Deposit end to end

The first reference deposit intentionally exposed a product precondition:

- SEP-24 id `3450d81a-987f-496a-8b23-de71611f12d1`;
- Reference UI submission succeeded;
- Anchor settlement failed because the Testnet wallet had no SRT trustline.

A normal Fresnica `trust add` then created the SRT trustline:

- trustline transaction
  `aaf21c9c6de68de87b2261cad5625fe375b1251d6e0a5737fb70f61ca9bf005a`.

The second deposit completed the whole chain:

- SEP-24 id `56e480db-4c09-433b-95aa-e7269b89cd1a`;
- Reference UI amount `1.23` USD;
- Anchor output `1.11` SRT;
- final Anchor status `completed`;
- Stellar settlement transaction
  `c3819ef01a0d53f969c156621fd35fa4b88430758e24923a9b8f1e2f269f0b99`;
- Fresnica's own typed balance read model reported exactly `1.11` SRT afterward.

This proves:

```text
Fresnica wallet
  -> native plugin dispatch
  -> host receive/auth boundary
  -> SEP-10
  -> SEP-24 interactive URL
  -> official reference browser UI
  -> Anchor processing
  -> Stellar settlement
  -> Fresnica balance read model
```

After the first failed deposit, a consumer-driven receive preflight was added. A fresh
funded Testnet wallet without SRT now fails before SEP-10 with `Destination has no
trustline ...`; the wallet with the valid SRT trustline passes the read-only preflight.
No trustline is ever auto-created by the plugin.

### SEP-12

A live `anchor customer` read against the reference Anchor succeeded and returned
`NEEDS_INFO` with 47 required fields and zero provided fields. No real identity/KYC
data was submitted.

### Withdrawal boundary

The withdrawal payment path is implemented as an interactive host-owned payment
proposal and covered by local tests/gates. A live withdrawal settlement was not
executed in this VPS session. Do not claim withdrawal settlement evidence from this
milestone.

## Packaging evidence

The release package contract now builds and ships three binaries together:

```text
fresnica
fresnica-anchor
fresnica-tui
```

A local release-package smoke put those three files in one PATH directory. `fresnica
plugin ls` reports only `anchor`, and the packaged release binaries successfully ran
live Testnet Anchor discovery. `fresnica-tui` is a reserved companion product and is
explicitly excluded from plugin discovery/dispatch.

## Validation

Terminal exact product commit `f700075628ae0381d0f5e77604eca1f6041ff292`:

- repository boundary PASS at upstream pin `a43ae377...`;
- rustfmt PASS;
- workspace Clippy `-D warnings` PASS;
- Anchor plugin tests 7/7 PASS;
- CLI unit tests 43/43 PASS;
- CLI contract tests 2/2 PASS;
- presentation tests 3/3 PASS;
- TUI tests 20/20 PASS;
- local release builds PASS for all three binaries;
- local packaged `plugin ls` PASS (`anchor` only);
- packaged live Testnet Anchor discovery PASS;
- complete reference SEP-10 -> SEP-24 deposit -> Stellar settlement PASS;
- live SEP-12 read PASS.

Upstream `a43ae377...` passed the full rust-client suite: 188 tests.

## Anti-drift rule / next boundary

The first native consumer is now strong enough to validate the **direction** of bounded
semantic host re-entry. It is not evidence for a generic plugin SDK.

Do not reintroduce the Anchor built-in, Anchor-specific dispatch shadowing, plugin
search/registry/install, unrestricted host RPC, generic signing, or signer-provider
access. Do not freeze the current env/JSON wire solely because Anchor works; a second
native consumer should confirm which parts are truly general.

Remaining acceptance evidence is narrow:

1. physical Ledger SEP-10 on a machine with a device;
2. a live withdrawal/payment completion when explicitly safe and desired;
3. a second real native consumer (SAINT or another bounded integration) before
   promoting the current process wire into a stable plugin SDK.
