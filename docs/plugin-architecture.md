# Fresnica Terminal Plugin Architecture

Status: **Accepted architecture; first native consumer validated**
Last reviewed: 2026-09-08

## Decision

Fresnica Terminal has two complementary external-command paths. They share a
process-oriented executable convention but have different trust contracts.

1. **Fresnica-native plugins** use `fresnica-*`. They are Fresnica-owned ecosystem
   integrations and may use narrowly defined semantic host re-entry.
2. **Stellar ecosystem plugins** use `stellar-*`, with legacy `soroban-*`
   compatibility. Fresnica consumes that executable ecosystem without cloning Stellar
   CLI registry/search/install behavior.

Plugins and signer providers remain separate extension mechanisms.

## Distribution naming convention

Executable names are the discovery contract and do not constrain implementation
language. Python packages should expose:

```text
Fresnica-native:  package fresnica_<name>  -> executable fresnica-<name>
Stellar current:  package stellar_<name>   -> executable stellar-<name>
Stellar legacy:   package soroban_<name>   -> executable soroban-<name>
```

`fresnica-tui` is a reserved companion product binary, not a plugin. It is excluded
from plugin discovery and dispatch even though its executable name shares the native
prefix.

## Command resolution

Built-in Fresnica commands win. Unknown commands use longest matching command chain
first and, for the same chain, namespace precedence:

```text
fresnica-<command-chain>
stellar-<command-chain>
soroban-<command-chain>
```

PATH discovery, argv forwarding, inherited stdio and child exit-status propagation
remain intentionally compatible with Stellar CLI external commands.

Anchor is now a real example of the normal rule: the CLI no longer contains an
`anchor` built-in, so `fresnica anchor ...` resolves to the bundled
`fresnica-anchor` executable. There is no special Anchor shadow route.

## Trust and wallet-context boundary

A plugin is a lower-trust external process for query, ecosystem orchestration, build
and proposal capabilities. Fresnica must not inject or pass:

- private keys or mnemonic material;
- Fresnica passphrases or raw unlock material;
- decrypted wallet state;
- opened Ledger/HSM handles;
- signer-provider objects;
- unrestricted transaction-signing capability.

The first real consumer proves that native plugins can need **bounded semantic host
re-entry**. The host remains responsible for validation, authorization, user review,
signer selection, signature verification and submission safety.

Current Anchor evidence includes four kinds of semantic interaction:

1. public wallet/network context;
2. read-only asset receive readiness;
3. Anchor-specific SEP-10 authentication returning only a short-lived token;
4. a payment proposal whose transaction is reconstructed and interactively approved
   by Fresnica.

These operations justify the architecture direction, not a generic host RPC. Their
current env/JSON wire is still experimental; see
[`docs/anchor-plugin-spike.md`](anchor-plugin-spike.md).

A plugin that needs an on-chain write must propose intent/material back to Fresnica.
It does not get to choose Fresnica's signer or bypass confirmation. In particular,
`fresnica-anchor status --pay` rejects `-y`/`--yes`; the host performs the normal
payment review interactively.

Signer Providers are higher-trust signing capabilities coordinated by Fresnica.
Ledger, HSM, secure-enclave and future passkey-style signers belong to that separate
model and are not CLI plugins.

## Ecosystem intent

`stellar-*` compatibility lets Fresnica reuse useful Stellar CLI ecosystem tools before
a Fresnica plugin community exists. `fresnica-*` serves a different purpose: Fresnica
can ship wallet-aware integrations without moving protocol/product-specific logic into
Core or the main Terminal binary.

Aqua-style DeFi logic belongs outside Core and may be a Fresnica-native plugin while
Fresnica remains wallet/security authority. SAINT can follow the same consumer model
once its standalone bounded contract is ready; SAINT itself should not depend on
private Fresnica plugin internals.

## Deliberate non-goals

This architecture does not currently require:

- plugin registry/search/install/update;
- in-process extension ABI;
- unrestricted host RPC;
- generic `sign-xdr` callback;
- signer-provider access from plugins;
- a stable SDK wrapping the current experimental process wire.

Do not infer those features from Anchor's bounded callbacks.

## Implementation checkpoint

Historical executable-dispatch evidence:

- Draft PR #32 `feat/terminal-stellar-plugin-dispatch@bd55984777de5e9058063dd6fb91580b69f471fa` validated Stellar/legacy dispatch;
- `feat/terminal-fresnica-plugin-namespace@26f5e63af4fd7b9d297b8a654ad0ad6c47f487ce` restored `fresnica-*` precedence without a host contract;
- initial Anchor consumer product `7d47c2ff006af85d437a8311d03e13d6d12081e7` proved the first bounded host interaction.

Current validated Anchor parity checkpoint:

- Terminal `feat/terminal-anchor-plugin-parity@f700075628ae0381d0f5e77604eca1f6041ff292`;
- tree `f5cf5dc59c32096071ed7a922547ada575b10582`;
- upstream `feat/rust-client-anchor-explicit-domain@a43ae377eea9346f48151c4d5ef596717fed2454`;
- old CLI Anchor built-in removed;
- `discover/auth/deposit/withdraw/status/customer` owned by `fresnica-anchor`;
- SEP-24-first/SEP-6-fallback preserved through shared client selectors;
- receive preflight, SEP-10 and interactive payment review stay host-owned;
- release package includes `fresnica-anchor` beside CLI/TUI;
- full local gates and official Testnet deposit settlement passed;
- no Main merge, GitHub CI run or release workflow triggered for this milestone.

Physical Ledger SEP-10 and live withdrawal settlement remain acceptance evidence, not
claimed results.

## Documentation/source-of-truth rule

This file records durable plugin architecture. `docs/CURRENT.md`, handoffs and PR notes
record implementation state and should link here instead of redefining the trust model.

If implementation only proves part of a future abstraction, write **not stable yet**.
Do not silently promote one consumer's wire details into a general SDK.
