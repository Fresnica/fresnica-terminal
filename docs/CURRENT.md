# Fresnica Terminal Current State

Status: **validated Draft stack remains unmerged/unreleased; Ledger Classic is accepted and the external-plugin work is active with its durable architecture now recorded separately**.

Last verified: 2026-09-08.

## Source of truth

- Current Terminal `main`: `ba7e24bbd3e6f527d4d56029a572ac8d6851015c`.
- Terminal v0.2.0 release commit: `a2485cad5d2d6048f8ffb6987597c2a3fca2670d`.
- Architecture-convergence product top: PR #19 `refactor/terminal-history-read-model@85fba1612ba7709a8040a0d1a1afc1011cd00d08`; cumulative Draft integration PR #21 is based on that validated tree plus this status record.
- Terminal #19 tree: `6f552cb7e9f1a4f12fea85309d9d4c9696a5364b`.
- Terminal Rust source pin: `Fresnica/fresnica@65a9da804ca16849e320d24365e6e839e62cf5d8`, the validated History implementation commit from upstream #167.
- Upstream #167 current head: `a63fc62a6289fc3b4696556eb436c9a0147feb96`. Its only change after the Terminal pin is the carried-forward `docs/capabilities/network.md` Rust-runtime scope correction; Rust source is unchanged.
- Upstream cumulative integration candidate: Draft PR #169, head `f01f6607bcd1cbf5e28de800ce215187bf23091f`, one commit directly on upstream `main`, with a tree byte-identical to #167 current top.
- Upstream Soroban capability Draft PR #171: `670f39dc6833af350059d5ad0280a3651dacf66d`; Required CI #102 / run `34074383876` passed after replacing the Fresnica-owned ABI value parser with official `soroban-spec-tools`.
- Terminal Soroban product Draft PR #25 final product head: `2bdf19dbea3b9b976fa2883a7cf26bbd366fff79`; CI #81 / run `34074999863`, Release Terminal #58 / run `34074999766`, and live Testnet native-SAC interface probe `34075404068` passed.
- Upstream read-only child Draft PR #172: `a4b0bd4eaa1a47e5f8cb8107ca264bffac6d444b`; Required CI #103 / run `34077833532` passed.
- Terminal read-only child Draft PR #26 code-gate head: `826eef6995943d06e20b011f98af426debeda5c1`; focused CLI tests and live empty-HOME Testnet read-only probe `34078192917` passed. This status record is synchronized in the final PR #26 tree; repository HEAD and CI remain authoritative for the docs-only follow-up head.

Repository source, exact branch heads and CI remain authoritative if this file later drifts. Durable plugin namespace/trust decisions are authoritative in [`docs/plugin-architecture.md`](plugin-architecture.md); this CURRENT file records implementation state only.

## Plugin architecture correction — 2026-09-08

The accepted architecture is dual-path, not Stellar-only:

- `fresnica-*`: Fresnica-native plugins for Fresnica-owned ecosystem integrations, including future Aqua/DeFi and SAINT consumers. This namespace may later receive a bounded public/session wallet context, but never wallet secrets or unrestricted signer authority.
- `stellar-*`, plus legacy `soroban-*`: compatibility path for consuming the existing Stellar CLI executable-plugin ecosystem without cloning Stellar CLI plugin search/registry/install.

For unknown commands the intended resolution is longest command-chain first, then `fresnica-*` -> `stellar-*` -> `soroban-*` for the same chain. Built-ins always win. Plugins remain separate from Signer Providers; writes must return through Fresnica review, authorization, signing, and submission safety.

Draft PR #32 `feat/terminal-stellar-plugin-dispatch@bd55984777de5e9058063dd6fb91580b69f471fa` proves the stronger PATH/listing/platform dispatcher for `stellar-*` and `soroban-*`. The follow-up `feat/terminal-fresnica-plugin-namespace` slice now restores `fresnica-*` on top of that dispatcher without adding a wallet-context ABI or signing callback. VPS validation passes repository boundary, rustfmt, CLI Clippy with warnings denied, 44 CLI unit tests, 2 CLI contract tests, and a real subprocess smoke proving Fresnica-first same-chain dispatch, exact argv forwarding, exit-code propagation, and deduplicated `plugin ls` output.

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

Desktop or another Rust product may consume `fresnica-client` semantic DTOs directly. Terminal-specific English wording, abbreviations and layout are not shared API. Mobile/Web are not required to route their Application Capabilities through the Rust `fresnica-client` runtime.

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

The corresponding upstream Fresnica Rust-client work is a stacked Draft line through #167. One parent-layer drift was found during closeout: #157 received a documentation-only Network runtime-scope correction after #159 had already forked. That exact correction was carried to #167 top as `a63fc62a6289fc3b4696556eb436c9a0147feb96`; Required CI #92 passed. No other parent-head drift was found in #159 -> #167.

## Soroban Contract Invoke P1

The Soroban product work is implemented as bounded Draft milestones rather than an architecture-cleanup continuation. The initial ordered `TYPE:VALUE` proposal was discarded after comparison with Stellar CLI's fully-typed contract model. The deployed contract specification is the source of truth, and the second slice follows Stellar CLI's current default-send simulation behavior rather than inventing a Fresnica-specific view command.

The resulting boundary is:

```text
on-chain Contract Spec / Stellar RPC
        |
        v
fresnica-client
  ContractInterface + named input validation + ScVal conversion
        |
        v
Terminal
  Fresnica options -- FUNCTION --named value
  dynamic help + review + confirmation + machine JSON
```

Upstream #171 resolves Stellar Asset Contract, Wasm, and CAP-85 external-reference specifications through the official Stellar RPC/spec crates and exposes provider-neutral function/parameter metadata. ABI value parsing, complex types, option omission, sanitized ABI names, examples, and normalized JSON conversion are delegated to official `soroban-spec-tools`; Fresnica no longer maintains a parallel `ScSpecTypeDef -> ScVal` parser. Terminal #25 consumes those DTOs without importing XDR/spec parsing and retains only wallet selection, dynamic product help, simulation-backed review, confirmation, signing coordination, pending-safety, and output semantics.

Upstream #172 adds the same default send decision used by current Stellar CLI: preliminary simulation sends only when it observes a read-write footprint, a published contract event, or authorization. Otherwise it returns `ContractReadResult` decoded through the same Contract Spec without resolving a wallet, sequence, passphrase, fee, pending record, or submission. Terminal #26 consumes that shared outcome, permits machine read-only calls without `-y`, and still requires `-y` after a write outcome is known. It also removes a real duplicate network operation from #25: actual invocation no longer fetches the Contract Spec once for an unused interface and again during preparation; interface discovery is now help-only.

No TUI contract surface, SEP-53/Dapp transport, hardware signer work, Main merge, or release is part of this milestone.

## Validation at the current tops

Terminal #19 exact head `85fba1612ba7709a8040a0d1a1afc1011cd00d08` has passed:

- disposable exact-tree verifier run `34034699994`;
- formal CI #58 / run `34034975664`: repository boundary, rustfmt, workspace clippy with warnings denied, workspace tests, CLI/TUI release builds and RefPython compatibility;
- Release Terminal #37 / run `34034975655`: validate, Linux x64 package, macOS arm64 package, Windows x64 package and Windows release-binary smoke;
- PR publish step correctly skipped.

Upstream History implementation commit `65a9da804ca16849e320d24365e6e839e62cf5d8` passed Required CI #89 / run `34034280333`.

Upstream #167 current head `a63fc62a6289fc3b4696556eb436c9a0147feb96` passed Required CI #92 / run `34038000875` after the carried-forward architecture document correction.

Upstream Soroban #171 exact head `670f39dc6833af350059d5ad0280a3651dacf66d` passed Required CI #102 / run `34074383876`, including compatibility, SDK boundary, rustfmt and Rust capability compile/tests with the official `soroban-spec-tools` dependency.

Terminal Soroban #25 exact head `2bdf19dbea3b9b976fa2883a7cf26bbd366fff79` passed CI #81 / run `34074999863`, Release Terminal #58 / run `34074999766` across Linux x64, macOS arm64 and Windows x64 with Windows binary smoke, and live Testnet native-SAC interface probe `34075404068`; publish correctly skipped.

Upstream read-only #172 exact head `a4b0bd4eaa1a47e5f8cb8107ca264bffac6d444b` passed Required CI #103 / run `34077833532`. Terminal #26 code-gate head `826eef6995943d06e20b011f98af426debeda5c1` passed focused CLI tests and live Testnet run `34078192917`, which used a brand-new empty HOME with no Fresnica wallet and returned native-SAC `balance` as `kind=read_only`, decoded a non-null result, kept `submission=null`, rendered `Submitted: no`, and never prompted for a Fresnica passphrase. Final docs-synchronized exact-head CI and Release Terminal are the closing gates.

No merge or release publication is part of this milestone closeout.

## Self-check conclusions

The current top tree was re-audited after #19 against the shared-foundation placement rule.

- Account, Balance and History no longer require Terminal to interpret provider-shaped read records.
- DEX order book, offers, pair trades, account fills and candles consume typed `fresnica-client` snapshots/models; Terminal owns parsing and rendering only.
- Anchor still contains substantial CLI-only orchestration and JSON input handling. This is an acknowledged exception, not a newly discovered ownership regression: there is still no second Terminal Anchor consumer or stronger shared contract evidence that justifies extracting another layer.
- Official-reuse audit: the Fresnica-owned primitive Soroban ABI parser, manual ContractInstance decode, deprecated Wasm helper path, and Terminal's duplicate invoke-time Contract Spec fetch have been removed.
- Official-reuse audit: `rs-stellar-rpc-client` already supplies `send_transaction` and polling, but its high-level error path does not preserve Fresnica's safety-critical distinction between explicit rejection and an uncertain submission that may already have been accepted. Keep Fresnica pending/reconciliation policy until the official client exposes enough structured transport/submission state.
- Official-reuse audit: community `soroban-client` / `stellar-baselib` overlaps Classic Payment/ChangeTrust/offer construction, but adopting it wholesale would also import a second keypair/signing/crypto and transaction ownership layer. Current direct SDF `stellar-xdr` construction remains the smaller boundary; use the community SDK as a possible conformance oracle, not a runtime replacement, unless its ownership/error model changes.
- Official-reuse audit: the official Wallet SDK covers SEP-1/10/6/12/24/38 but currently has no Rust implementation. Terminal Anchor therefore remains an interactive product state machine over Client-owned protocol validation; extract only when a second Rust product proves the shared orchestration.
- Low-priority cleanup: Terminal Friendbot still hard-codes the public Testnet endpoint even though Stellar RPC `getNetwork` exposes `friendbotUrl`; move endpoint discovery behind Client when Friendbot is next touched.
- The History `1..=200` limit is enforced by `FresnicaClient::history()` itself. Terminal's matching argument guard is user-facing validation rather than a provider-ownership leak. A test name still mentions Horizon page size; that wording alone does not justify a code PR.
- CLI and TUI authorization rendering intentionally differ in full-key versus compact-key presentation. Do not generalize the new History presentation crate into a universal formatter merely for symmetry.
- The cumulative Terminal diff from `main` to #19 is purely ahead, contains only expected product/document/formal-boundary files and no disposable verifier artifacts.
- The cumulative upstream diff is also purely ahead. The only discovered stacked-branch ancestry drift was the #157 documentation correction described above, and it is now present at the validated top.

No new blocking implementation defect remains from this audit.

## Milestone boundary

The architecture-convergence plan is **implemented**. Continuing to add refactor PRs merely because more code can be moved would violate the evidence-driven boundary used for this work.

The following are separate product/capability decisions, not unfinished cleanup in this milestone:

- richer transaction-grouped Activity, cache/cursors, spam classification and enrichment;
- generic Dapp/SEP-53 session transport;
- external Ledger hardware integration;
- Passkey / smart-account product paths;
- Anchor TUI parity;
- Horizon-to-RPC/provider migration beyond the semantic boundaries already prepared.

## Integration disposition

Do **not** merge the long stacked Draft PRs one by one as the primary integration path. Their purpose is architecture reasoning, surgical implementation and exact-slice CI evidence; the #157 -> #159 documentation fork demonstrates why a long mutable stack is a poor final merge unit.

Preferred integration shape:

1. preserve the original stacked Draft PRs as architecture evidence;
2. keep cumulative architecture candidates #169 (upstream) and #21 (Terminal) as the pre-Soroban integration baseline;
3. keep Soroban #171/#172 and Terminal #25/#26 as bounded child Draft milestones until the integration decision;
4. if upstream #169/#171/#172 are integrated, repin the Terminal cumulative candidate once to the final upstream revision rather than churning dependency SHAs through intermediate Draft heads;
5. rerun exact-head Terminal gates after that final repin;
6. only then decide Main merge and release timing.

This avoids both stacked ancestry drift and pointless dependency-SHA churn during review.

Until that decision is made, do not merge the Soroban Drafts into Main or publish a new Terminal release. Preserve #171/#172 and #25/#26 as bounded evidence, and do not broaden the presentation/shared layers without concrete duplicate responsibility.
