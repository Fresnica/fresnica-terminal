# Fresnica Agent Guide

This file is the entry point for AI agents working with or on Fresnica Terminal.
Use runtime discovery before reading implementation code. Prefer existing machine capabilities over new code.

## What Fresnica is

Fresnica is a self-custody Stellar wallet whose reusable application capabilities are headless first. The terminal repository contains:

- `fresnica`: machine- and human-usable CLI;
- `fresnica-tui`: interactive terminal UI;
- `fresnica-*`: external executable plugins, including the bundled `fresnica-anchor`;
- documentation and product adapters over the shared Fresnica Rust capability layer.

The security/application ownership boundary is:

```text
fresnica-core    cryptographic and protocol security meaning
      |
fresnica-sdk     stable SDK/security boundary
      |
fresnica-client  reusable wallet/application capabilities
      |
fresnica-terminal
  CLI / TUI / plugins / presentation / packaging
```

Do not duplicate wallet, transaction, signing, authorization, or Soroban semantics in Terminal when they belong in `fresnica-client`.

## First action: discover, do not guess

Before writing code, ask the running product what it already supports:

```sh
fresnica capabilities --json
```

The `fresnica-capabilities-v1` response describes machine-ready operations, effects, runtime dependencies, JSON output, and any explicit noninteractive confirmation requirement.

For a Soroban contract, inspect the deployed Contract Spec instead of hard-coding an interface:

```sh
fresnica --network testnet contract C... --json
```

The versioned `fresnica-soroban-abi-v1` model describes functions, recursive ABI types, UDTs, and per-input composition support.

## Task decision tree

Use this order:

```text
Need to accomplish a Stellar/Fresnica task?
|
+-- Existing operation in `fresnica capabilities --json`?
|     `-- yes: call the public machine CLI; do not write a second implementation.
|
+-- Soroban contract operation?
|     `-- inspect `contract ... --json`, follow `composition`, then compose/simulate.
|
+-- New business/protocol orchestration over existing capabilities?
|     `-- create a `fresnica-<name>` plugin.
|
+-- Missing reusable wallet capability?
|     `-- implement at the shared `fresnica-client` boundary first.
|
`-- Needs secrets/signing/authorization?
      `-- authority stays inside Fresnica; never move it into a plugin or agent helper.
```

## Using Fresnica as an agent

Prefer JSON-producing commands and preserve their schemas. Do not scrape human text when a machine form exists.

Typical discovery:

```sh
fresnica capabilities --json
fresnica wallet list --json
fresnica account --json
fresnica balance --json
fresnica plugin ls --json
```

A write operation is not permission to bypass Fresnica policy. If a machine-ready operation advertises an explicit confirmation flag such as `-y`, use it only when the calling workflow has authority to perform the action. Never feed secrets through argv, environment values, logs, or JSON output.

## Soroban ABI Composer

Read [`docs/soroban-composer.md`](docs/soroban-composer.md) before constructing contract calls.

The fast path is:

```text
inspect deployed contract
  -> find function in `abi.functions`
  -> inspect each input's `composition`
  -> build typed arguments
  -> `--simulate --json`
  -> inspect semantic result/effects
  -> invoke through normal Fresnica review/authorization when a write is intended
```

Composition modes in `fresnica-soroban-abi-v1`:

- `typed_json`, `guided=true`: safe complete-domain ABI-guided JSON; use `--args-json`.
- `dynamic_scval_json`, `guided=false`: contains open-ended Soroban `Val`; use explicit tagged ScVal JSON and preserve `scval_xdr` identity.
- `scval_xdr_success_only`, `guided=false`: conservative `Result` boundary; exact success-value XDR is the supported expert path.
- `unsupported`, `guided=false`: do not invent an encoding. Stop or request a capability change.
- unknown future mode: treat as unguided/unsupported.

Do not implement a second Soroban ABI codec. Fresnica intentionally delegates ScVal encoding/normalization to the official Soroban spec tools with narrow fail-closed guards.

## Creating a plugin

Use a plugin only for business/protocol orchestration that is not already a single public Fresnica operation.

Human-oriented guide: [`docs/creating-plugins.md`](docs/creating-plugins.md).
Agent fast path: [`docs/creating-plugins-for-agents.md`](docs/creating-plugins-for-agents.md).
Canonical minimal example: [`examples/plugins/fresnica-xlm-balance`](examples/plugins/fresnica-xlm-balance/README.md).

Default plugin architecture:

```text
fresnica <name> ...
  -> PATH executable `fresnica-<name>`
  -> plugin calls `$FRESNICA_PLUGIN_HOST ... --json`
  -> Fresnica retains wallet semantics and authority
```

Before adding a private `__plugin-host` capability, prove that the public machine CLI cannot express the operation safely. `__plugin-host` is for bounded authority/state handoff, not a duplicate API surface.

## Plugin security invariants

A plugin or agent helper MUST NOT:

- read Fresnica wallet storage directly;
- request, persist, log, or forward Fresnica passphrases;
- access private keys, mnemonics, raw unlock material, or opened signer handles;
- become a generic `sign-xdr` service;
- choose Fresnica signers or bypass review/confirmation policy;
- duplicate an existing Fresnica machine operation as a private host RPC.

Plugins are ordinary local executables and are not OS-sandboxed. Treat plugin code as lower trust than the wallet host.

## Modifying Fresnica itself

Before changing source:

1. inspect the relevant existing capability and tests;
2. identify the owning layer from the architecture above;
3. make the smallest change at that owner;
4. keep CLI/TUI/plugin layers as presentation/orchestration adapters;
5. add a regression test for the behavior changed;
6. run local validation before considering CI.

Do not work directly on `main`. Keep shared-source revisions exact in `FRESNICA_REV`; changing the pin is an explicit compatibility change.

For broader architectural constraints read:

- [`docs/operation-foundation.md`](docs/operation-foundation.md)
- [`docs/plugin-architecture.md`](docs/plugin-architecture.md)
- [`docs/CURRENT.md`](docs/CURRENT.md)

## Testing

Prefer Testnet for network behavior and local repository tests for deterministic semantics.

For ordinary source changes:

```sh
bash scripts/validate-boundary.sh
cargo fmt --all --check
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
```

For a plugin, at minimum verify:

```text
[ ] executable is discoverable by `fresnica plugin ls --json`
[ ] `fresnica <name> ...` dispatches to it
[ ] host-provided network/context is used rather than re-created
[ ] existing wallet operations are called through machine JSON surfaces
[ ] invalid input returns non-zero
[ ] no secret or generic signing authority crosses the plugin boundary
[ ] Testnet smoke test passes when the plugin depends on network behavior
```

Do not claim a live end-to-end result unless the live path was actually executed.

## Documentation map

- [`README.md`](README.md): product/repository entry point.
- [`crates/cli/README.md`](crates/cli/README.md): complete CLI surface and machine contracts.
- [`docs/soroban-composer.md`](docs/soroban-composer.md): v0.6 Composer usage contract.
- [`docs/creating-plugins.md`](docs/creating-plugins.md): plugin author guide.
- [`docs/creating-plugins-for-agents.md`](docs/creating-plugins-for-agents.md): fast plugin workflow for agents.
- [`docs/plugin-architecture.md`](docs/plugin-architecture.md): plugin rationale and trust boundary.
- [`docs/anchor-plugin-spike.md`](docs/anchor-plugin-spike.md): complex real plugin example and bounded host re-entry evidence.
- [`docs/operation-foundation.md`](docs/operation-foundation.md): reusable-operation architecture.
- [`docs/CURRENT.md`](docs/CURRENT.md): current implementation/checkpoint state.
