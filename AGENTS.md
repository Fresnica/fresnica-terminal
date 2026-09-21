# AGENTS.md

Use this file as the starting point for automated work in Fresnica Terminal. Prefer the product's machine interfaces to source-code archaeology.

## Start here

Check what the installed CLI can already do:

```sh
fresnica capabilities --json
```

For Soroban work, inspect the deployed contract before writing an adapter:

```sh
fresnica --network testnet contract C... --json
```

If those two commands already expose the required operation and ABI, use them. Do not add a second implementation.

## Repository boundaries

```text
fresnica-core      cryptography and protocol security
fresnica-sdk       stable SDK/security boundary
fresnica-client    reusable wallet/application capabilities
fresnica-terminal  CLI, TUI, plugins, presentation, packaging
```

Wallet semantics belong in `fresnica-client`, not in CLI/TUI/plugin code. This includes transaction construction, authorization, signer selection, Soroban argument semantics, and submission rules.

Terminal consumes the exact shared-source revision in `FRESNICA_REV`. Changing that pin is a compatibility change.

## Using Fresnica from an agent

Use JSON output whenever it exists:

```sh
fresnica capabilities --json
fresnica wallet list --json
fresnica account --json
fresnica balance --json
fresnica plugin ls --json
```

Do not scrape human output when a JSON contract exists. A command advertising `-y` still requires the caller to have authority to perform the write; machine access does not bypass wallet policy.

Never place passphrases, mnemonics, private keys, or unlock material in argv, environment values, logs, fixtures, or JSON output.

## Soroban

See [`docs/soroban-composer.md`](docs/soroban-composer.md).

The normal flow is:

```text
inspect contract -> read ABI -> compose arguments -> simulate -> review effects -> invoke
```

Each ABI input has a `composition` field:

- `typed_json`, `guided=true`: use `--args-json`.
- `dynamic_scval_json`: use explicit tagged ScVal JSON and preserve `scval_xdr`.
- `scval_xdr_success_only`: use an exact successful ScVal through `--scval-xdr`.
- `unsupported` or an unknown future mode: stop rather than inventing an encoding.

Do not add another Soroban ABI codec. Fresnica uses the official spec tooling and adds only narrow validation around its unsafe or ambiguous edges.

## Plugins

Use a plugin for protocol or business orchestration over existing Fresnica capabilities. Do not create a plugin merely to rename one existing command.

Start with:

- [`docs/creating-plugins.md`](docs/creating-plugins.md) for the normal developer guide.
- [`docs/creating-plugins-for-agents.md`](docs/creating-plugins-for-agents.md) for the short agent workflow.
- [`examples/plugins/fresnica-xlm-balance`](examples/plugins/fresnica-xlm-balance/README.md) for the minimal working example.

A plugin is an executable named `fresnica-<name>`. For wallet operations it should call the host supplied in `FRESNICA_PLUGIN_HOST` and consume public `--json` interfaces.

Private `__plugin-host` operations are reserved for cases where a public operation cannot safely express a required authority/state handoff. They are not a second general API.

Plugins never own private keys, mnemonics, passphrases, unlock material, signer handles, signer selection, or generic `sign-xdr` authority.

## Changing Fresnica

Before editing:

1. locate the existing capability and tests;
2. identify the owning layer;
3. change that layer only;
4. add a regression test when behavior changes;
5. run the local gate.

Do not work directly on `main`. Do not move shared semantics into a presentation layer for convenience.

Relevant architecture documents:

- [`docs/operation-foundation.md`](docs/operation-foundation.md)
- [`docs/plugin-architecture.md`](docs/plugin-architecture.md)
- [`docs/CURRENT.md`](docs/CURRENT.md)

## Writing documentation

Write for a developer using the project, not for a model reading a prompt. Prefer commands, schemas, constraints, and concrete examples. Do not repeat the same rule in several files: guides explain how, architecture documents explain why, and `docs/CURRENT.md` records implementation state. Avoid motivational prose, artificial dialogue, and generic checklists unless the checklist is an actual release or test gate.

## Validation

For repository changes:

```sh
bash scripts/validate-boundary.sh
cargo fmt --all --check
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
```

Use Testnet for network behavior. Do not report a live E2E result unless that path was actually executed.

For a plugin, also verify discovery, dispatch, invalid-input exit status, and the relevant Testnet path.

## Documentation

- [`README.md`](README.md) — repository entry point.
- [`crates/cli/README.md`](crates/cli/README.md) — CLI surface and machine contracts.
- [`docs/soroban-composer.md`](docs/soroban-composer.md) — v0.6 Composer.
- [`docs/creating-plugins.md`](docs/creating-plugins.md) — plugin development.
- [`docs/plugin-architecture.md`](docs/plugin-architecture.md) — plugin boundary and rationale.
- [`docs/anchor-plugin-spike.md`](docs/anchor-plugin-spike.md) — advanced real plugin example.
- [`docs/CURRENT.md`](docs/CURRENT.md) — current implementation state.
