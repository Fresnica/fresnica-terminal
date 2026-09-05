# Terminal Shared Foundation Refactor

Status: active — foundation evidence complete; CLI/TUI hardening remains

Branch: `refactor/terminal-shared-foundation`

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

Evidence now shows that Payment, Trustline and DEX already share their semantic implementation through `fresnica-client`.

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

### Phase 0 - Conformance and boundary audit — complete for foundation flows

Map Terminal flows against:

- Fresnica Capability / Flow contracts;
- RefPython reference semantics;
- current pinned Rust client / SDK behavior.

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

### Phase 3 - Expand only proven boundaries — foundation review complete

- **Trustline:** same proven client boundary as Payment; no Terminal service layer justified.
- **DEX writes:** same proven prepared-request/submission boundary; no Terminal service layer justified.
- **Read flows:** shared query semantics already in `fresnica-client`; presentation stays local.
- **Wallet lifecycle:** CLI presentation is isolated in its own module while wallet semantics remain upstream; TUI responsibility review remains.
- **Anchor:** currently CLI-only orchestration, so no Terminal-wide extraction without a second consumer or stronger contract evidence.

A previous pattern is never applied mechanically when the next flow has different semantics.

### Phase 4 - CLI hardening

Before structural TUI work:

- keep CLI primarily parsing, prompting, authorization interaction and rendering;
- remove only proven forwarding/ownership duplication;
- keep exact/public output regression coverage where compatibility matters;
- keep the direct client/SDK dependency boundary explicit;
- enforce a deliberate clippy gate rather than hiding warnings.

Current evidence already removed the redundant CLI transaction sign/submit wrapper and direct `stellar-xdr` dependency, isolated wallet presentation, and established `clippy -D warnings`. The remaining gate is final CLI responsibility/diff review, not a parser-framework rewrite.

### Phase 5 - TUI responsibility and structure hardening

Do not assume that TUI needs a new shared business-flow layer: Payment, Trustline and DEX already call the same `fresnica-client` capabilities as CLI.

Proceed test-first:

1. identify important `state + event -> state/effect` transitions;
2. add direct tests around those transitions before moving code;
3. separate natural presentation responsibilities such as state/forms/update/render only where the current file demonstrates a stable seam;
4. preserve TUI behavior and shared-client semantics exactly;
5. do not introduce a state-machine framework or new feature while splitting files.

### Phase 6 - Final integration and release

The refactor branch remains the development target through the full milestone.

A **draft PR may be used as a CI/integration surface while development continues**, but it is not a merge signal.

Before finalizing/merging:

- review the exact branch diff against the recorded baseline;
- run repository boundary validation;
- formatter check;
- workspace tests with lockfile;
- focused compatibility/conformance tests;
- release builds for CLI and TUI;
- clippy gate;
- verify no relay/probe/temp files remain;
- verify documentation matches the final architecture.

Only after the full refactor is complete and final CI is green:

1. mark/finalize the PR from `refactor/terminal-shared-foundation` to `main`;
2. merge according to repository convention;
3. verify the resulting `main` SHA and post-merge CI;
4. select the next Terminal version from the actual compatibility impact;
5. publish and verify the new release artifacts/checksums.

Do not use `main` as the working target before this completion gate.

## Non-goals

- no new Fresnica Application framework/reference implementation;
- no requirement for Desktop/Mobile to depend on Terminal code;
- no new UI design during the foundation/CLI phases;
- no feature expansion disguised as refactoring;
- no broad Core/SDK cleanup beyond concrete upstream gaps exposed by this work;
- no mechanical rewrite into a new command/parser/state-machine framework;
- no premature shared abstraction for code used only once;
- no wrapper layer whose only job is forwarding to an existing `fresnica-client` capability.

## Drift guard

If implementation pressure conflicts with this document, do not silently broaden or redefine the architecture.

First determine whether new repository evidence changes one of the decisions above. If so, update this document with the evidence and rationale in the same development branch before proceeding.

The source, tests, Fresnica contracts and verified CI remain the final truth; this document exists to preserve the agreed direction, not to override stronger evidence.

## Definition of done

This milestone is complete when:

- CLI and TUI consume shared semantics from the correct owning layer rather than maintaining competing business implementations;
- Terminal-local shared code exists only where evidence proves a real Terminal-specific common responsibility;
- both CLI and TUI remain independent presentation/I/O adapters;
- Terminal conforms to Fresnica contracts without creating a new cross-platform Application authority;
- upstream Fresnica gaps found during the work have been fixed or explicitly recorded at the owning layer;
- high-value TUI state/effect behavior is directly tested and the TUI source is split only along proven presentation seams;
- the full refactor branch passes final validation;
- the completed branch is merged to `main` once;
- a new verified Fresnica Terminal release is published from the merged result.
