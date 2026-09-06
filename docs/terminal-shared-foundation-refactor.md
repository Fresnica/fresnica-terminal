# Terminal Shared Foundation Refactor

Status: complete — merged and released as Fresnica Terminal v0.1.1

Milestone branch: `refactor/terminal-shared-foundation` (merged via PR #3)

Baseline `main`: `a742ef3130e455c9cbdbf42378d07f3e1f30153f`

## Purpose

Refactor the already-working Fresnica Terminal implementation so CLI and TUI consume shared behavior from the **correct owning layer** while remaining conformant with Fresnica Capability / Flow / security contracts.

The work starts from existing CLI/TUI behavior. It does not invent a new cross-platform Fresnica Application layer, and it does not require a Terminal-local shared crate when `fresnica-client` / `fresnica-sdk` already provide the proven common boundary.

## Architectural authority

The authority order is:

```text
Fresnica Capability / Flow / Security contracts
                 |
RefPython reference semantics and candidate evidence
                 |
       Fresnica Terminal implementation
           /                 \
        CLI                   TUI
```

Rules:

1. Fresnica contracts define shared semantics and invariants.
2. RefPython remains the executable laboratory for uncertain product semantics and Application Flows. Terminal does not replace it.
3. Terminal is an independent production implementation of those contracts.
4. Shared behavior stays at its narrowest correct owner: Fresnica client/SDK when cross-platform semantics already live there; Terminal-local code only for proven Terminal-specific reuse.
5. Another product such as Fresnica Desktop may reuse proven Terminal code when it fits, but no platform is required to depend on `fresnica-terminal`.
6. Cross-platform evidence discovered here is classified before promotion:
   - semantic/invariant gap -> feed back to Fresnica contract / ADR / vectors;
   - reusable SDK/client capability gap -> feed back to Fresnica SDK/client;
   - implementation technique only -> remain local or be documented as practice, not promoted as normative architecture.
7. Security/Core/SDK-owned behavior remains in its owning Fresnica layer. Terminal must not reimplement cryptography, envelope semantics, signature verification, protocol authority, or platform credential policy.

## Target boundary

Evidence shows that Payment, Trustline and DEX already share their semantic implementation through `fresnica-client`.

```text
              Fresnica client / SDK
          shared semantics and prepared models
                 /             \
                /               \
              CLI               TUI
         argv / prompt      events / forms
         text rendering     ratatui rendering
                \               /
                 \             /
          optional Terminal-local sharing
          only for proven presentation-neutral
          Terminal-specific responsibility
```

CLI/TUI retain platform-specific interaction state. Shared Terminal code is extracted only when existing behavior demonstrates a stable common responsibility that is not already owned upstream.

Do not create a broad `application-flow` framework in advance. Crate/module naming and final boundaries must follow evidence from the existing code.

## Classification used during audit

Every relevant CLI/TUI responsibility is classified as one of:

### A. Presentation / I/O

Examples: argv parsing, hidden prompt, confirmation UI, stdout/stderr formatting, exit codes, terminal key events, ratatui widgets.

Keep these in CLI/TUI.

### B. Terminal-local shared implementation

Examples: presentation-neutral orchestration genuinely shared by CLI and TUI and not already supplied by `fresnica-client` / `fresnica-sdk`.

Extract only after the existing implementation proves the common boundary.

### C. Fresnica upstream concern

Examples: missing Capability semantics, SDK/client API gap, secret-lifetime/security boundary issue, missing language-neutral conformance vector.

Fix or clarify in the owning Fresnica project first, then update Terminal's exact source pin as required.

## Execution order

### Phase 0 - Conformance and boundary audit — complete

Mapped Terminal flows against Fresnica Capability / Flow contracts, RefPython reference semantics and the pinned Rust client / SDK behavior.

The audit established that Payment, Trustline and DEX semantics already live at the correct shared `fresnica-client` boundary. See `terminal-flow-audit.md`.

### Phase 1 - Foundation defects — complete

Resolved proven foundational defects without unrelated Core/SDK work:

- passphrase secret-lifetime ownership fixed at the Fresnica Rust client boundary;
- Terminal callers borrow the existing secret buffer;
- duplicate `fresnica info` compatibility line fixed;
- real-binary output regression coverage added;
- exact shared source pin updated to the fixing Fresnica commit.

### Phase 2 - Payment as the first boundary proof — complete, no new layer justified

Payment was used to test the original extraction hypothesis.

Actual evidence:

```text
CLI argv --------------------> PaymentRequest
                               |
                               v
                    FresnicaClient::prepare_payment
                               |
                         PreparedPayment
                               |
                    +----------+----------+
                    |                     |
              CLI review/prompt      TUI review/state
                    |                     |
                    +----------+----------+
                               |
                    FresnicaClient::submit_payment
```

The shared implementation already exists in `fresnica-client`. A Terminal-local Payment Flow wrapper would be a forwarding layer with no independent responsibility, so it is deliberately **not** created.

### Phase 3 - Expand only proven boundaries — complete

- **Trustline:** same proven client boundary as Payment; no Terminal service layer justified.
- **DEX writes:** same proven prepared-request/submission boundary; no Terminal service layer justified.
- **Read flows:** shared query semantics already in `fresnica-client`; presentation stays local.
- **Wallet lifecycle:** CLI presentation is isolated in its own module while wallet semantics remain upstream.
- **Anchor:** remains CLI-only orchestration, so no Terminal-wide extraction without a second consumer or stronger contract evidence.

A previous pattern is never applied mechanically when the next flow has different semantics.

### Phase 4 - CLI hardening — complete

The CLI is now primarily parsing, prompting, authorization interaction and rendering over the shared client boundary:

- wallet presentation moved out of the entrypoint without moving wallet semantics;
- redundant local transaction sign/submit forwarding removed;
- direct CLI `stellar-xdr` dependency removed;
- public `info` output has a real-binary regression test;
- the source pin moves with the exact upstream passphrase fix;
- no parser framework or command redesign was introduced.

The remaining large Anchor module is intentionally not split into a shared layer for symmetry: there is no second Terminal Anchor consumer and no stronger contract evidence requiring such an abstraction.

### Phase 5 - TUI responsibility and structure hardening — complete

The TUI was handled test-first. Before moving code, five high-value no-Horizon state-transition tests were added for local browse/form/cancel/watch-only behavior. Those tests run with an isolated local `FresnicaClient` and complement the existing Form -> Request tests.

Only after they passed, the former monolithic source was mechanically separated into natural presentation responsibilities:

```text
main.rs    415 lines  terminal lifecycle, options, tests
app.rs     487 lines  application state and key/effect orchestration
state.rs   328 lines  modes, forms and request construction
render.rs  642 lines  ratatui rendering and display helpers
```

The split deliberately does **not** introduce a state-machine framework, a new business-flow crate, or a feature redesign. Payment, Trustline and DEX continue to call the same `fresnica-client` capabilities.

The wider review also corrected one existing Offer-form help drift (`u/c` was advertised while `e/x` was implemented) and promoted clippy to the entire workspace:

```text
cargo clippy --workspace --all-targets --locked -- -D warnings
```

Staged validation passed after the split: workspace clippy, all workspace tests, both release builds and `git diff --check`.

### Phase 6 - Final integration and release — complete

The completed implementation was finalized through the repository's normal merge and release gates:

1. PR #3 (`refactor: establish Terminal shared capability foundation`) passed formal `CI #33` and `Release Terminal #20` on exact head `92f64e396b410d074d218dfec6ac5d58e8d410bb`, including RefPython compatibility and Linux/macOS/Windows package builds.
2. PR #3 was squash-merged to `main` as `fcf0e5b177283e2dfd0a07d1180f0fc7189ae2ec`; post-merge `CI #34` passed.
3. Compatibility impact was classified as patch-level: implementation hardening and bug fixes without public command-semantic breakage.
4. Release PR #4 changed only the CLI/TUI package versions, their two `Cargo.lock` entries, and the new immutable `releases/terminal-v0.1.1.json` marker. Formal `CI #35` and `Release Terminal #21` passed, including all three platform packages.
5. PR #4 was squash-merged to `main` as `88fe7067062521789044a643b6816cc70f77aeaa`; post-merge `CI #36` passed independently.
6. `Release Terminal #22` passed validate, Linux/macOS/Windows packaging and publish, creating the `v0.1.1` prerelease targeted exactly at `88fe7067062521789044a643b6816cc70f77aeaa`.

Published v0.1.1 asset digests recorded by GitHub Release metadata:

```text
fresnica-terminal-0.1.1-linux-x64.zip    064cb3469e4e5c4a7f008bcecf3aeeed18b17f88256e3d676ce10e89c4230743
fresnica-terminal-0.1.1-macos-arm64.zip   e0d8d609d584030fb93fc38416b7674d2015e0b0506c089e3fbb38f3bcfdb82a
fresnica-terminal-0.1.1-windows-x64.zip   1d7382592648be6950b016142b09e1927ba272f69a210ee9d18f6b379bd3417d
fresnica-terminal-release-manifest.json   23f14b34a4a3ac7183c7f3a1d1e12564a797113488b47bc6a955e72203409c94
SHA256SUMS                                 c4575d34812d7fbacdbaf048ee32adecc74546effee0985fa2e458d31cfcfac8
```

The release manifest was generated from the downloaded platform artifacts by the publish job and records version `0.1.1`, release commit `88fe7067062521789044a643b6816cc70f77aeaa`, and shared Fresnica source `9ba6f23cefe34e8d5940b311ec78f27eed982fe7`.

## Non-goals

- no new Fresnica Application framework/reference implementation;
- no requirement for Desktop/Mobile to depend on Terminal code;
- no new UI design during this refactor;
- no feature expansion disguised as refactoring;
- no broad Core/SDK cleanup beyond concrete upstream gaps exposed by this work;
- no mechanical rewrite into a new command/parser/state-machine framework;
- no premature shared abstraction for code used only once;
- no wrapper layer whose only job is forwarding to an existing `fresnica-client` capability.

## Drift guard

If implementation pressure conflicts with this document, do not silently broaden or redefine the architecture.

First determine whether new repository evidence changes one of the decisions above. If so, update this document with the evidence and rationale in the same development branch before proceeding.

The source, tests, Fresnica contracts and verified CI remain the final truth; this document exists to preserve the agreed direction, not to override stronger evidence.

## Definition of done — satisfied

All milestone conditions are satisfied:

- CLI and TUI consume shared semantics from the correct owning layer rather than maintaining competing business implementations;
- Terminal-local shared code exists only where evidence proves a real Terminal-specific common responsibility;
- both CLI and TUI remain independent presentation/I/O adapters;
- Terminal conforms to Fresnica contracts without creating a new cross-platform Application authority;
- upstream Fresnica gaps found during the work have been fixed or explicitly recorded at the owning layer;
- high-value TUI state/effect behavior is directly tested and the TUI source is split only along proven presentation seams;
- the full refactor branch passes final validation;
- the completed branch is merged to `main` once;
- a new verified Fresnica Terminal release is published from the merged result.
