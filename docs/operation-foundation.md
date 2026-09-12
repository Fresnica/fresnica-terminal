# Fresnica Operation Foundation

Status: **accepted architecture constraint for v0.5 and later**

Last updated: 2026-09-12.

## Purpose

Fresnica is not a Stellar CLI wrapper and its CLI is not merely a terminal-shaped wallet UI. Fresnica provides wallet operations that humans, agents, bots, scripts, plugins and native UIs can all consume through the same semantic and security boundaries.

The main architectural constraint is:

> Business capabilities must exist headlessly before they are expressed through a UI or ecosystem integration.

A feature is not complete merely because a CLI/TUI/mobile screen can perform it. The reusable semantic operation must have a presentation-independent owner, deterministic inputs and outputs, and a path that can be exercised without GUI automation.

This document is above [`plugin-architecture.md`](plugin-architecture.md). Plugins are one consumer of the Operation Foundation, not the center of the architecture.

## First-class consumers

The architecture must treat these as legitimate peers:

- human CLI users;
- agents and coding agents;
- unattended bots and automation;
- shell/Python/other scripts;
- Fresnica plugins;
- TUI, desktop and native mobile UI adapters.

Human usability and machine usability are not competing modes. Human presentation may be concise and explanatory while machine surfaces remain structured and stable, but both must consume the same underlying capability semantics.

## Ownership and headless rule

Shared business semantics belong at the narrowest correct Fresnica owner:

```text
Core / SDK               cryptography, identities, signing invariants
        |
fresnica-client          wallet/application capabilities and semantic DTOs
        |
Operation surfaces       CLI machine/human output, plugin host, native UI adapters
```

Do not move business logic into a CLI parser, TUI state machine, plugin, Swift view, or Kotlin screen merely because that is the first consumer.

Do not create forwarding frameworks when `fresnica-client`, SDK or Core already owns the needed capability. A new abstraction must have an independent responsibility proven by at least two concrete consumers or by a security/protocol boundary.

The CLI is the executable interface to these capabilities. It is allowed to own argv parsing, terminal prompting, human rendering and process exit semantics; it is not the canonical implementation of Payment, Contract execution, authorization or signing.

## Operation lifecycle

Where the protocol permits it, machine-operable actions should expose the following lifecycle:

```text
discover / resolve
        -> inspect
        -> prepare / simulate
        -> review / authorize
        -> execute
        -> observe
```
Read-only discovery and inspection must not require wallet secrets or transaction authorization. A write must not become easier to authorize merely because its caller is an agent or script.

For noninteractive operation, machine-readable output must remain one deliberate schema document on stdout. When a write requires explicit noninteractive approval, the caller must opt into the existing policy (`-y` today) after the operation has been classified as a write. `-y` never bypasses signer authentication, authorization rules or transaction validation.

## Machine contract

Capabilities intended for automation must converge on:

- structured JSON for inspect/read/execute results where practical;
- stable semantic field names rather than rendered terminal text;
- meaningful process exit status;
- deterministic validation errors that do not require screen scraping;
- exact network and asset/account/contract identity in machine output;
- no secrets, passphrases, unlock keys or bearer credentials in diagnostics;
- no requirement to drive a TUI, browser or mobile simulator to exercise business logic.

Human output may evolve independently as presentation. Machine JSON must not be produced by parsing human text.

The machine surface must also be discoverable without prose scraping. `fresnica capabilities --json` is the versioned local inventory for operations that already have deliberate JSON contracts. Capability discovery must not require a wallet, HOME directory, Horizon or RPC initialization. The inventory must stay conservative: an operation is listed only after its machine output and noninteractive policy are implemented and tested.

## Capabilities, not secrets

Agents, scripts and plugins request bounded wallet capabilities. They do not receive generic signing authority.

The host remains authoritative for:

- wallet/signer identity;
- authorization and threshold evaluation;
- simulation and transaction construction;
- review and explicit policy gates;
- Device Unlock / Fresnica Passphrase / hardware signer coordination;
- signature verification;
- submission and uncertain-submission recovery.
Generic `sign-xdr`, decrypted-wallet export and signer-provider handles are therefore not Operation Foundation capabilities.

## Wallet knowledge and Contract Store

A wallet may persist public knowledge that makes later operations safer and cheaper to understand. For Soroban this begins with a network-scoped Contract Store.

The Store is not just an alias map or an RPC cache. It separates information by provenance:

```text
user             local names and deliberate user choices
observed         facts read from the selected Stellar network
external         information supplied by a registry/plugin/domain when introduced
derived          conclusions computed from identified source facts
```

v0.5 must add only fields backed by real evidence. The first observed facts are the deployed contract identity and executable/Wasm identity already available while loading the Contract Spec. Raw `contractmetav0` key/value entries embedded in that Wasm may also be retained as self-declared observed facts with explicit Wasm provenance. They must not be promoted to trusted protocol generation, home domain, application identity or similar derived claims until Fresnica has a concrete verification source for that stronger conclusion. Derived capability snapshots must record the standard/version they evaluated and keep distinct evidence distinct; for SEP-41, native SAC identity, SEP-47 self-declaration and Contract Spec interface compatibility are separate facts rather than one trusted-token boolean.

A stored friendly name must never replace exact chain identity. Human output may prefer the name; review and machine output must retain the resolved `C...` contract address.

Contract upgrades are wallet-relevant state changes. When a known Contract ID resolves to a different executable/Wasm identity during normal invocation, Fresnica must fail closed before authorization/signing and require explicit inspection before refreshing the stored observation. User naming is preserved, while higher layers must re-evaluate compatibility instead of silently trusting stale knowledge.

Store persistence is an implementation detail. `contract list --json` and other machine surfaces define product schemas separately from the on-disk format so storage can evolve without breaking automation.

## Identity resolution

Wallet operations should accept stable local names where the protocol type makes the intended identity unambiguous.

For Soroban Contract Spec `Address` / `MuxedAddress` positions, Fresnica may resolve names from wallet, contact and saved-contract stores. Raw valid Stellar addresses always win and cannot be shadowed. Conflicting local names must fail as ambiguous rather than choosing silently.

External protocol output may already contain a typed Soroban `ScVal`. Fresnica may accept that value as a bounded argument encoding, but encoded input is never an alternate ABI or authorization path: it must decode successfully, validate against the current Contract Spec parameter type, normalize into the same semantic review representation, and then use the same simulation/review/authorization/signing pipeline as ordinary textual/JSON arguments.
Name resolution is presentation/convenience, not authorization. The resolved transaction still passes through normal simulation, Soroban authorization and signer selection.

## Plugin relationship

A plugin is an operation consumer and orchestrator. It may contribute protocol knowledge, command grammar, remote-service integration and presentation. It must not reimplement wallet security or transaction authority merely to integrate a Dapp.

The desired end state permits a useful plugin to be a small shell, Python, Node or native executable that composes stable Fresnica operations. Language choice is not part of the trust model.

The current `fresnica-*` executable namespace and bounded plugin-host re-entry remain valid evidence. Ordinary reusable operations should use the same public machine CLI available to agents and scripts through `FRESNICA_PLUGIN_HOST`; do not mirror `contract`, `token`, payment or other headless operations into a second private plugin-host RPC surface. Private `__plugin-host` re-entry is for exceptional boundaries that need host-only state or authority, such as ephemeral authentication material or mandatory host-owned proposal review.

A Testnet shell-plugin proof confirmed this rule: a plugin consisting only of `$FRESNICA_PLUGIN_HOST --network "$FRESNICA_PLUGIN_NETWORK" token XLM balance OWNER --json` reused the complete resolver/SEP-41/Contract pipeline without a plugin-specific API. This is the desired cost model for future AI-generated integrations.

Do not freeze the current env/JSON wire into a universal Plugin SDK merely because this reuse works. Plugin packaging, manifests and installation systems may evolve. Capability and authorization boundaries are the durable contract.

## UI relationship

CLI, TUI, Swift, Kotlin or another native client may each have platform-appropriate interaction code. Sharing one UI implementation is not an architectural goal.

They should instead share Fresnica semantic capabilities and conformance expectations. Duplicating native presentation is acceptable; duplicating transaction semantics, authorization policy or cryptographic logic is not.

## v0.5 proof sequence

v0.5 uses Soroban as the first demanding proof of this foundation:

1. make deployed contracts directly usable through Contract Spec-driven operations;
2. evolve the flat contract-name map into a versioned Store with real chain observations and migration from the current format;
3. resolve wallet/contact/contract names only for typed contract Address inputs;
4. expose useful Store and contract inspection as machine-readable output;
5. detect executable/Wasm changes and recheck write preparation before review;
6. reuse the same simulation/review/authorization/signing path for human and machine execution;
7. expose a zero-state versioned inventory of machine-ready operations;
8. only after the foundation is stable, prove it with a thin standard or ecosystem consumer such as SEP-41 token operations or Aqua.

## Non-goals

Do not broaden v0.5 merely to make the foundation look complete:

- no Stellar CLI build/deploy/upload clone without a wallet-use case;
- no generic in-process plugin framework;
- no speculative universal fact graph for Contract Store metadata;
- no generic signing RPC;
- no agent-only bypass around human security invariants;
- no duplicate business implementation for CLI, TUI or mobile;
- no requirement that plugins use Rust or link Fresnica libraries.

## Definition of done for a new operation

A new wallet operation is architecturally complete when its reusable semantic behavior can be exercised headlessly, its exact identities and relevant review state are available to machine consumers, write authorization remains host-owned, and each UI/plugin adapter contains only interaction/orchestration responsibility appropriate to that consumer.

If an implementation can only be validated through a visual UI, or an agent must scrape prose to understand its result, the operation foundation is incomplete.

## Anti-drift rule

When a convenient implementation conflicts with these rules, do not silently put business authority back into a UI or plugin. First identify the missing Fresnica capability and implement it at the narrowest correct owner.

When a new consumer proves that an existing boundary is too narrow, evolve the boundary from evidence. Do not pre-build abstractions for imagined consumers.
