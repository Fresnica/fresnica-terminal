# Terminal Shared Foundation Refactor

Status: active

Branch: `refactor/terminal-shared-foundation`

Baseline `main`: `a742ef3130e455c9cbdbf42378d07f3e1f30153f`

## Purpose

Refactor the already-working Fresnica Terminal implementation so the CLI and TUI share Terminal-local implementation code while remaining conformant with Fresnica's Capability / Flow / security contracts.

The work starts from existing CLI behavior. It does not invent a new cross-platform Fresnica Application layer.

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
4. Code shared by CLI and TUI is a Terminal implementation asset, not an official Fresnica Application reference implementation.
5. Another product such as Fresnica Desktop may reuse proven Terminal code when it fits, but no platform is required to depend on `fresnica-terminal`.
6. Cross-platform evidence discovered here is classified before promotion:
   - semantic/invariant gap -> feed back to Fresnica contract / ADR / vectors;
   - reusable SDK capability gap -> feed back to Fresnica SDK/client;
   - implementation technique only -> remain local or be documented as practice, not promoted as normative architecture.
7. Security/Core/SDK-owned behavior remains in its owning Fresnica layer. Terminal must not reimplement cryptography, envelope semantics, signature verification, protocol authority, or platform credential policy.

## Target boundary

CLI and TUI should become I/O and presentation adapters around Terminal-local shared implementation where real duplication exists.

```text
Fresnica client / SDK
         |
Terminal-local shared implementation
         |
   +-----+-----+
   |           |
  CLI         TUI
argv/prompt   events/forms
text output   ratatui render
```

CLI/TUI may retain platform-specific interaction state. Shared code is extracted only when existing behavior demonstrates a stable common responsibility.

Do not create a broad `application-flow` framework in advance. Crate/module naming and final boundaries must follow evidence from the existing code.

## Classification used during audit

Every relevant CLI/TUI responsibility must be classified as one of:

### A. Presentation / I/O

Examples: argv parsing, hidden prompt, confirmation UI, stdout/stderr formatting, exit codes, terminal key events, ratatui widgets.

Keep these in CLI/TUI.

### B. Terminal-local shared implementation

Examples: presentation-neutral request/review/result models or orchestration genuinely shared by CLI and TUI.

Extract only after the existing implementation proves the common boundary.

### C. Fresnica upstream concern

Examples: missing Capability semantics, SDK/client API gap, secret-lifetime/security boundary issue, missing language-neutral conformance vector.

Fix or clarify in the owning Fresnica project first, then update Terminal's exact source pin as required.

## Execution order

### Phase 0 - Conformance and boundary audit

Before structural edits, map current Terminal flows against:

- Fresnica Capability / Flow contracts;
- RefPython reference semantics;
- current pinned Rust client / SDK behavior.

Produce a concrete classification of presentation code, Terminal-local shared code, and upstream gaps.

### Phase 1 - Foundation defects

Handle already-proven foundational defects before extraction:

- passphrase secret-lifetime ownership issue at the correct Fresnica Rust client boundary;
- Terminal callers updated to avoid unnecessary secret copies after the upstream API permits it;
- duplicate `fresnica info` compatibility line regression;
- high-value regression tests for public behavior.

No unrelated Core/SDK refactor.

### Phase 2 - Payment as the first extraction sample

Use the existing send/payment path because it is currently the clearest flow.

Goal:

```text
CLI input
 -> Terminal-local shared payment implementation
 -> structured review/result
 -> CLI confirmation/authorization/rendering
```

Requirements:

- preserve existing CLI behavior and public contract;
- conform to Fresnica payment/security semantics;
- do not move CLI presentation into shared code;
- prove behavior with focused tests before expanding the pattern.

### Phase 3 - Expand only proven boundaries

After Payment is stable, evaluate in order:

1. Trustline;
2. wallet lifecycle/read flows;
3. DEX;
4. Anchor.

Each flow is independently audited and verified. A previous extraction pattern is not applied mechanically if the next flow has different semantics.

### Phase 4 - CLI hardening complete

Before TUI migration:

- CLI business-flow duplication should be removed where justified;
- CLI remains primarily parsing, prompting, authorization interaction and rendering;
- exact/public output behavior has regression coverage where compatibility matters;
- shared implementation has focused unit/conformance tests;
- clippy baseline is clean enough to establish an appropriate gate without hiding existing warnings.

### Phase 5 - TUI migration

Only after the shared Terminal foundation is proven through CLI:

- replace duplicated TUI business-flow implementation with shared Terminal code;
- preserve TUI-specific state machine, event handling and rendering where appropriate;
- add direct tests for important `state + event -> state/effect` behavior;
- split the large TUI source file only along responsibilities revealed by the migration, not according to a preselected framework.

### Phase 6 - Final integration and release

The refactor branch remains the development target through the full milestone.

Before merge:

- review the exact branch diff against the recorded baseline;
- run repository boundary validation;
- formatter check;
- workspace tests with lockfile;
- focused compatibility/conformance tests;
- release builds for CLI and TUI;
- clippy gate if the baseline has been deliberately cleaned and accepted;
- verify no relay/probe/temp files remain;
- verify documentation matches the final architecture.

Only after the full refactor is complete and final CI is green:

1. open/finalize the PR from `refactor/terminal-shared-foundation` to `main`;
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
- no premature shared abstraction for code used only once.

## Drift guard

If implementation pressure conflicts with this document, do not silently broaden or redefine the architecture.

First determine whether new repository evidence changes one of the decisions above. If so, update this document with the evidence and rationale in the same development branch before proceeding.

The source, tests, Fresnica contracts and verified CI remain the final truth; this document exists to preserve the agreed direction, not to override stronger evidence.

## Definition of done

This milestone is complete when:

- CLI and TUI share the Terminal-local implementation that evidence shows should be shared;
- both remain independent presentation/I/O adapters rather than competing business implementations;
- Terminal conforms to Fresnica contracts without creating a new cross-platform Application authority;
- upstream Fresnica gaps found during the work have been fixed or explicitly recorded at the owning layer;
- the full refactor branch passes final validation;
- the completed branch is merged to `main` once;
- a new verified Fresnica Terminal release is published from the merged result.
