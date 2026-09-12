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

Terminal v0.4.0 pins `6c3388c84993bf8dfd133638a7c32a37a0769e74`, the exact Fresnica Rust capability source with versioned Argon2id wallet protection and the System Authentication boundary. Active v0.5 development pins `54ceeec7de63931ed89219d1a3587cb289262687`, adding typed Contract Address names plus executable/Wasm observations carried through contract inspection and invocation results, with write preparation rechecking executable identity before review. Native SDK v0.3.0 remains the binary SDK baseline (Native Binding API 3 / Universal SDK API 5 / Core Client API 5); Terminal consumes `fresnica-client` / `fresnica-sdk` directly without changing that published Native/UniFFI ABI.

## Layout

```text
crates/cli/       fresnica command-line product
crates/anchor-plugin/  native Anchor plugin
crates/tui/       fresnica-tui interactive product
scripts/          repository-boundary validation
FRESNICA_REV      pinned shared Fresnica source revision
```

## Releases

Fresnica Terminal v0.4.0 is the integrated Main baseline. Active v0.5 work is unreleased and stays on feature branches until its capability and product gates are complete. A release contains `fresnica`, `fresnica-anchor`, and `fresnica-tui`.

Release publication remains marker-gated. The release workflow revalidates the repository boundary, locked workspace tests/builds, and Python CLI compatibility before publishing platform archives plus a manifest and SHA-256 checksums. Release binaries are built from the exact merge commit and retain the exact `FRESNICA_REV` source pin.

The CLI supports safe `-v` / `-vv` diagnostics. Verbose output exposes execution stages and version/network metadata, never the raw argument vector or hidden secret input.

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
