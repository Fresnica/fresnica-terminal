# Fresnica Terminal

Fresnica Terminal is the native terminal product repository for Fresnica. It contains both terminal surfaces:

- `fresnica` — command-line interface for scripting and direct wallet operations;
- `fresnica-tui` — interactive terminal UI;
- `fresnica-anchor` — Fresnica-native Anchor plugin discovered by `fresnica` through PATH.

These terminal products intentionally live together because they share the same Rust Application Capability layer, wallet storage semantics, release toolchain and compatibility contract.

## Architecture boundary

This repository owns terminal product behavior: command parsing, terminal interaction, rendering, confirmation, local product orchestration and packaging.

Shared security and wallet semantics remain in [`Fresnica/fresnica`](https://github.com/Fresnica/fresnica):

- `fresnica-core` owns cryptographic/protocol security meaning;
- `fresnica-sdk` exposes the stable security/application SDK boundary;
- `fresnica-client` is the reusable Rust Application Capability reference implementation.

Terminal code must not depend on `fresnica-core` directly. Shared Rust dependencies are Git-pinned in the workspace root to the exact commit recorded in [`FRESNICA_REV`](FRESNICA_REV). Updating that revision is an explicit compatibility change and must pass this repository's full CI.

This repository was extracted from `Fresnica/fresnica` at source commit `8c06bce3fb51ac04e4e94c41d3a99c5c6db77b03`. The active shared-source baseline is independent of that historical extraction point and is always the exact commit recorded in `FRESNICA_REV`.

The integrated Main baseline is v0.5.0. Active v0.6 development pins the exact Fresnica shared-source revision recorded in [`FRESNICA_REV`](FRESNICA_REV) and defines the release scope as **Soroban ABI Composer + documentation**: recursive `fresnica-soroban-abi-v1` contract semantics, typed/dynamic composition guidance, exact ScVal identity where JSON is insufficient, and human/AI-agent documentation for using and extending Fresnica without duplicating wallet authority. Native SDK v0.3.0 remains the binary SDK baseline (Native Binding API 3 / Universal SDK API 5 / Core Client API 5); Terminal consumes `fresnica-client` / `fresnica-sdk` directly without changing that published Native/UniFFI ABI.

## Layout

```text
crates/cli/             fresnica command-line product
crates/anchor-plugin/   native Anchor plugin
crates/tui/             fresnica-tui interactive product
docs/                   architecture and user/developer guides
examples/plugins/       canonical external-plugin examples
AGENTS.md               AI-agent project/workflow entry point
scripts/                repository-boundary validation
FRESNICA_REV            pinned shared Fresnica source revision
```

## Releases

Fresnica Terminal v0.5.0 is the integrated Main baseline. v0.6 is the active release candidate on feature branches and combines the frozen Soroban ABI Composer contract with a documentation closeout for human and AI-agent consumers. A release contains `fresnica`, `fresnica-anchor`, and `fresnica-tui`.

Release publication remains marker-gated. The release workflow revalidates the repository boundary, locked workspace tests/builds, and Python CLI compatibility before publishing platform archives plus a manifest and SHA-256 checksums. Release binaries are built from the exact merge commit and retain the exact `FRESNICA_REV` source pin.

The CLI supports safe `-v` / `-vv` diagnostics. Verbose output exposes execution stages and version/network metadata, never the raw argument vector or hidden secret input.

Automation can discover the currently machine-ready operation surface without opening wallet storage or network providers:

```bash
fresnica capabilities --json
```

The versioned `fresnica-capabilities-v1` inventory reports exact CLI/source versions plus JSON-capable operation IDs, usage, effects and runtime dependencies. It intentionally omits commands that do not yet have a deliberate machine-output contract.

## Documentation

Start here according to the task:

- [`AGENTS.md`](AGENTS.md) — project map, decision tree, safety invariants, and workflows for AI agents.
- [`docs/soroban-composer.md`](docs/soroban-composer.md) — v0.6 Soroban ABI Composer machine contract and usage.
- [`docs/creating-plugins.md`](docs/creating-plugins.md) — human-oriented plugin quick start and security boundary.
- [`docs/creating-plugins-for-agents.md`](docs/creating-plugins-for-agents.md) — compressed plugin workflow for AI agents.
- [`examples/plugins/fresnica-xlm-balance`](examples/plugins/fresnica-xlm-balance/README.md) — minimal canonical plugin example.
- [`docs/plugin-architecture.md`](docs/plugin-architecture.md) — rationale and lower-trust plugin boundary.
- [`docs/operation-foundation.md`](docs/operation-foundation.md) — reusable capability ownership.
- [`docs/CURRENT.md`](docs/CURRENT.md) — exact current implementation/checkpoint state.

The intended discovery order for automation is `fresnica capabilities --json` first, then contract inspection (`fresnica ... contract C... --json`) for Soroban work. Prefer runtime machine contracts over guessing from implementation details.

## Build and test

```bash
bash scripts/validate-boundary.sh
cargo test --workspace --all-targets
cargo build --release -p fresnica-cli --bin fresnica
cargo build --release -p fresnica-anchor-plugin --bin fresnica-anchor
cargo build --release -p fresnica-tui --bin fresnica-tui
```

Run the products:

```bash
target/release/fresnica --help
PATH="$PWD/target/release:$PATH" target/release/fresnica plugin ls
target/release/fresnica-tui --network testnet
```

CI also checks the CLI against the Python reference compatibility suite from the exact same pinned `Fresnica/fresnica` revision.
