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
- validated parity product: `f700075628ae0381d0f5e77604eca1f6041ff292`.
- immediate SEP-6 compatibility product: `8c33baaf837d078ed090aba6310a125c788fc027`.
- parity product tree: `f5cf5dc59c32096071ed7a922547ada575b10582`.
- immediate SEP-6 product tree: `6711b5bc956d094399b218a396085896d6217f71`.
- parent spike/docs checkpoint: `09d7da83f087a5bb707c8e12ec16af8d48efd311`.
- pinned Fresnica source: `07be0fb4fedbb448ab1538305c1a336378724a43`.
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

## Legacy/programmatic SEP-6 evidence — fchain.io

A live compatibility probe against `fchain.io` confirmed a useful real-world boundary:

- SEP-1 advertises `TRANSFER_SERVER=https://api.fchain.io`; no SEP-24 server and no SEP-10 path are advertised for this flow.
- `/info` advertises XRP deposit/withdraw through the legacy SEP-6 `type=crypto` / `dest` / `dest_extra` shape.
- live deposit instructions return an XRPL custody address plus a mandatory Destination Tag.
- live withdrawal instructions return the Anchor Stellar `account_id`, `memo_type=hash`, memo, fees and minimum amount immediately, with no transaction id.
- `/transaction` and `/transactions` currently return 404 and `/info.transactions.enabled=false`.

This is an older but historically standard SEP-6 usage pattern. Modern SEP-6 later tightened the transaction lifecycle (notably v3.11.0 requiring wallets to obtain `amount_in` from `/transaction` before payment) and deprecated `type` / `dest` / `dest_extra` in favor of `funding_method` plus SEP-12. Fresnica therefore uses modern fields when advertised but keeps legacy `/info`-driven compatibility instead of treating deprecated fields as unsupported.

The consumer-driven fix adds generic **immediate SEP-6 withdrawal** handling: when a withdrawal response directly contains `account_id` and optional memo fields, and the user explicitly supplied `amount`, the plugin converts those instructions into the same host-owned payment proposal used by async `status --pay`. Human mode enters Fresnica's mandatory interactive payment review; `--json` never submits a payment. There is no `fchain.io` domain special case.

Actual mainnet settlement was not executed in this environment because that requires real funded assets and signing authority. Live evidence covers the real Anchor discovery/instruction endpoints; deterministic tests cover the immediate-response-to-host-proposal boundary.

### SEP-59 relationship

Draft SEP-59 addresses a different resource: a reusable **inbound external account/instrument** (crypto address, address+memo/tag, IBAN, virtual account) bound to a wallet. It explicitly exists because repeatedly representing a reusable receiving account as a SEP-6 transaction makes transaction/account lifecycle and reconciliation ambiguous. It is not a condemnation of anchors that historically reused SEP-6 deposit addresses.

The fchain XRP custody address + per-user Destination Tag is conceptually close to SEP-59's reusable crypto-address-plus-memo example, but fchain is not a SEP-59 implementation: SEP-59 has its own account lifecycle and authentication requirements. SEP-59 is also inbound-only, so SEP-6 remains relevant for withdrawals.

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

Terminal parity product `f700075628ae0381d0f5e77604eca1f6041ff292`, plus immediate SEP-6 compatibility `8c33baaf837d078ed090aba6310a125c788fc027`:

- repository boundary PASS at upstream pin `07be0fb4...`;
- rustfmt PASS;
- workspace Clippy `-D warnings` PASS;
- Anchor plugin tests 7/7 PASS;
- CLI unit tests 44/44 PASS;
- CLI contract tests 2/2 PASS;
- presentation tests 3/3 PASS;
- TUI tests 20/20 PASS;
- local release builds PASS for all three binaries;
- local packaged `plugin ls` PASS (`anchor` only);
- packaged live Testnet Anchor discovery PASS;
- complete reference SEP-10 -> SEP-24 deposit -> Stellar settlement PASS;
- live SEP-12 read PASS.

Upstream `07be0fb4...` passed the full rust-client suite: 189 tests, including the fchain-shaped immediate SEP-6 withdrawal response.

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
