# Fresnica Terminal

Fresnica Terminal contains the terminal-facing Fresnica products:

- `fresnica` — CLI for scripts and direct wallet operations;
- `fresnica-tui` — interactive terminal UI;
- `fresnica-anchor` — bundled Anchor plugin discovered through `PATH`.

They share the same wallet semantics, Rust capability layer, and release toolchain.

## Architecture boundary

This repository owns command parsing, terminal interaction, rendering, confirmation, local orchestration, and packaging.

Shared security and wallet semantics remain in [`Fresnica/fresnica`](https://github.com/Fresnica/fresnica):

- `fresnica-core` owns cryptographic/protocol security meaning;
- `fresnica-sdk` exposes the stable security/application SDK boundary;
- `fresnica-client` is the reusable Rust Application Capability reference implementation.

Terminal code must not depend on `fresnica-core` directly. Shared Rust dependencies are Git-pinned in the workspace root to the exact commit recorded in [`FRESNICA_REV`](FRESNICA_REV). Updating that revision is an explicit compatibility change and must pass this repository's full CI.

This repository was extracted from `Fresnica/fresnica` at source commit `8c06bce3fb51ac04e4e94c41d3a99c5c6db77b03`. The active shared-source baseline is independent of that historical extraction point and is always the exact commit recorded in `FRESNICA_REV`.

Main is currently v0.5.0. v0.6 adds the Soroban ABI Composer and the documentation needed to use it from scripts, plugins, and agents. The exact shared Fresnica revision is recorded in [`FRESNICA_REV`](FRESNICA_REV).

Native SDK v0.3.0 remains the binary SDK baseline (Native Binding API 3 / Universal SDK API 5 / Core Client API 5).

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

A release contains `fresnica`, `fresnica-anchor`, and `fresnica-tui`. v0.6 is currently being prepared from feature branches.

Release publication remains marker-gated. The release workflow revalidates the repository boundary, locked workspace tests/builds, and Python CLI compatibility before publishing platform archives plus a manifest and SHA-256 checksums. Release binaries are built from the exact merge commit and retain the exact `FRESNICA_REV` source pin.

The CLI supports safe `-v` / `-vv` diagnostics. Verbose output exposes execution stages and version/network metadata, never the raw argument vector or hidden secret input.

Machine-ready operations can be discovered without opening wallet storage or contacting providers:

```bash
fresnica capabilities --json
```

The versioned `fresnica-capabilities-v1` inventory reports exact CLI/source versions plus JSON-capable operation IDs, usage, effects and runtime dependencies. It intentionally omits commands that do not yet have a deliberate machine-output contract.

## Documentation

- [`AGENTS.md`](AGENTS.md) — instructions for automated work in this repository.
- [`docs/soroban-composer.md`](docs/soroban-composer.md) — Soroban ABI inspection and argument composition.
- [`docs/creating-plugins.md`](docs/creating-plugins.md) — plugin development.
- [`docs/creating-plugins-for-agents.md`](docs/creating-plugins-for-agents.md) — short plugin workflow for agents.
- [`examples/plugins/fresnica-xlm-balance`](examples/plugins/fresnica-xlm-balance/README.md) — minimal plugin example.
- [`docs/plugin-architecture.md`](docs/plugin-architecture.md) — plugin design and trust boundary.
- [`docs/operation-foundation.md`](docs/operation-foundation.md) — capability ownership.
- [`docs/CURRENT.md`](docs/CURRENT.md) — current implementation state.

For automation, start with `fresnica capabilities --json`. For Soroban, inspect the deployed contract with `fresnica ... contract C... --json`.

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
