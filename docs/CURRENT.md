# Fresnica Terminal Current State

Status: **architecture-convergence milestone implemented and validated on a stacked Draft PR line; not merged to `main` and not released**.

Last verified: 2026-09-06.

## Source of truth

- Released baseline: `main@ba7e24bbd3e6f527d4d56029a572ac8d6851015c` with Terminal v0.2.0 released from `a2485cad5d2d6048f8ffb6987597c2a3fca2670d`.
- Current Terminal product head: PR #19 `refactor/terminal-history-read-model@85fba1612ba7709a8040a0d1a1afc1011cd00d08`.
- Current Terminal tree: `6f552cb7e9f1a4f12fea85309d9d4c9696a5364b`.
- Current shared Fresnica source pin: upstream PR #167 `refactor/rust-client-history-read-model@65a9da804ca16849e320d24365e6e839e62cf5d8`.
- Both Terminal #19 and upstream #167 remain Draft/open intentionally.

Repository source, exact branch heads and CI remain authoritative if this file later drifts.

## What this milestone was solving

The post-v0.2.0 work was not a feature race. It removed proven ownership leaks between Terminal presentation code and the reusable Rust capability client without inventing a second Application layer.

The target boundary is now:

```text
provider / protocol data
        |
        v
fresnica-client semantic capability models
        |
        +---------------------------+
        |                           |
        v                           v
Terminal machine output      Terminal human presentation
Fresnica-owned schema        CLI / TUI wording and compaction
```

Desktop or another Rust product may consume `fresnica-client` semantic DTOs directly. Terminal-specific English wording, abbreviations and layout are not shared API.

## Implemented Draft stack

The current Terminal stack is intentionally linear and unmerged:

1. **#11 — Classic authorization review**
   - consumes the shared prepared-transaction authorization snapshot;
   - CLI/TUI do not recalculate signer weights or thresholds;
   - submit remains authoritative and refreshes current ledger authorization.
2. **#12 — Network endpoint profile**
   - `--horizon-url` / `FRESNICA_HORIZON_URL` resolve into shared `NetworkProfile`;
   - Terminal does not construct provider gateways directly.
3. **#13 — Typed Account read model**
   - removes raw provider Account JSON from the Terminal boundary;
   - `account --json` is a deliberate Terminal/Fresnica machine schema derived from typed state.
4. **#14 — Typed Balance read model**
   - CLI/TUI consume `AssetBalance` / `BalanceAsset`;
   - exact asset identity and liabilities remain semantic data, not presentation parsing.
5. **#15 — Local/network initialization split**
   - `info`, `contact` and `wallet` remain usable without network-provider configuration;
   - network commands still fail closed on invalid network configuration.
6. **#16 — Watch-only registration ownership**
   - watch-only record construction moved behind the Rust Client Account/Wallet capability;
   - Terminal no longer directly depends on the SDK merely to assemble the record.
7. **#17 — Application passcode policy ownership**
   - current terminal/reference store-wide passcode execution moved out of CLI prompt code into Rust Client wallet helpers;
   - the policy remains implementation-specific, not a cross-platform normative contract.
8. **#18 — Restore signer compatibility ownership**
   - protected-signer restore revalidation moved behind the Rust Client wallet boundary;
   - path, prompt, rename, overwrite and output remain Terminal responsibilities.
9. **#19 — Typed History read model and Terminal presentation**
   - TUI stores `Vec<HistoryOperation>` rather than provider JSON;
   - CLI/TUI render typed History semantics;
   - a thin `fresnica-terminal-presentation` crate owns shared human-only History formatting;
   - `history --json` is generated directly from typed DTOs and does not pass through human presentation;
   - full account/asset identity remains available to other Rust consumers such as a future Desktop product.

The corresponding upstream Fresnica Rust-client work is also a stacked Draft line through #167. Do not repin these Terminal branches to unrelated upstream heads without first checking their direct stack dependency and rerunning the exact gates.

## Validation at the current top

Terminal #19 exact head `85fba1612ba7709a8040a0d1a1afc1011cd00d08` has passed:

- disposable exact-tree verifier run `34034699994`;
- formal CI #58 / run `34034975664`: repository boundary, rustfmt, workspace clippy with warnings denied, workspace tests, CLI/TUI release builds and RefPython compatibility;
- Release Terminal #37 / run `34034975655`: validate, Linux x64 package, macOS arm64 package, Windows x64 package and Windows release-binary smoke;
- PR publish step correctly skipped.

Upstream #167 exact head `65a9da804ca16849e320d24365e6e839e62cf5d8` passed Required CI #89 / run `34034280333`.

No merge or release is part of this milestone closeout.

## Self-check conclusions

The current top tree was re-audited after #19 against the shared-foundation placement rule.

- Account, Balance and History no longer require Terminal to interpret provider-shaped read records.
- DEX order book, offers, pair trades, account fills and candles consume typed `fresnica-client` snapshots/models; Terminal owns parsing and rendering only.
- Anchor still contains substantial CLI-only orchestration and JSON input handling. This is an acknowledged exception, not a newly discovered ownership regression: there is still no second Terminal Anchor consumer or stronger shared contract evidence that justifies extracting another layer.
- The History `1..=200` limit is enforced by `FresnicaClient::history()` itself. Terminal's matching argument guard is user-facing validation rather than a provider-ownership leak. A test name still mentions Horizon page size; that wording alone does not justify a code PR.
- CLI and TUI authorization rendering intentionally differ in full-key versus compact-key presentation. Do not generalize the new History presentation crate into a universal formatter merely for symmetry.

No new blocking implementation defect was found in this audit.

## Milestone boundary

The current architecture-convergence plan is **implemented**. Continuing to add refactor PRs merely because more code can be moved would violate the evidence-driven boundary used for this work.

The following are separate product/capability decisions, not unfinished cleanup in this milestone:

- richer transaction-grouped Activity, cache/cursors, spam classification and enrichment;
- Soroban invoke product input/review UX;
- generic Dapp/SEP-53 session transport;
- external Ledger hardware integration;
- Passkey / smart-account product paths;
- Anchor TUI parity;
- Horizon-to-RPC/provider migration beyond the semantic boundaries already prepared.

## Next decision point

**Hold merge and release.** The next step is an integration-disposition review, not another automatic feature/refactor slice.

That review should verify, in order:

1. the upstream Fresnica Draft stack through #167 still has the intended parent chain and green exact-head gates;
2. the Terminal Draft stack #11 -> #19 still has the intended parent chain and green exact-head gates;
3. the cumulative diff still matches the architecture contracts and contains no verifier/probe product artifacts;
4. compatibility/release impact is classified from the cumulative behavior, not from individual PR count;
5. only then decide whether to integrate the stack, reorganize it, or keep selected parts unmerged.

Until that decision is made, do not merge to `main`, publish a new Terminal release, start Soroban merely because the lower-level capability exists, or broaden the presentation crate without concrete duplicate responsibility that has the same presentation requirements.
