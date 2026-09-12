# Fresnica Terminal Current State

Status: **v0.4.0 is integrated on Main. v0.5 development is active and shifts the product target from a plugin-oriented extension base to a headless Operation Foundation equally usable by humans, agents, bots, scripts, plugins and native UIs. Soroban is the first proof surface.**

Last verified: 2026-09-12.

## Source of truth

- Current Terminal `main`: `420c09c3005ac82dc211b2170e9da546de373800`, squash integration of Fresnica Terminal v0.4.0 through PR #38.
- Active v0.5 branch: `feat/terminal-v0.5-contract-ux`. First direct-contract checkpoint: `7a761a6ec221698536bc47a3c26f8dfb190c8f87`. Current Operation Foundation product checkpoint: `9f301032b820cfca6a5d01ecb2ae626a7ec3b278`.
- Active v0.5 now pins shared Fresnica source `4dc85568be2115904bec32ced1afa8b5a26b5a86` from `feat/rust-client-contract-address-names`. The upstream line is intentionally incremental: `3ab24e0` adds typed local-name resolution for Contract Spec Address inputs; `6ad7d56` exposes contract executable kind and resolved Wasm hash during interface inspection; `8f15175` carries the same observation through read-only and transaction invoke results; `54ceeec` rechecks executable identity after real write preparation and fails closed if code changed during preparation; `373e194` reuses `soroban_spec_tools` to expose raw `contractmetav0` Wasm metadata; `4dc8556` derives versioned SEP-41 v0.5.1 evidence while keeping native-SAC identity, SEP-47 self-declaration and current interface compatibility separate.
- Terminal v0.2.0 release commit: `a2485cad5d2d6048f8ffb6987597c2a3fca2670d`.
- Architecture-convergence product top: PR #19 `refactor/terminal-history-read-model@85fba1612ba7709a8040a0d1a1afc1011cd00d08`; cumulative Draft integration PR #21 is based on that validated tree plus this status record.
- Terminal #19 tree: `6f552cb7e9f1a4f12fea85309d9d4c9696a5364b`.
- Historical #19 Terminal Rust source pin: `Fresnica/fresnica@65a9da804ca16849e320d24365e6e839e62cf5d8`, the validated History implementation commit from upstream #167.
- Upstream #167 current head: `a63fc62a6289fc3b4696556eb436c9a0147feb96`. Its only change after the Terminal pin is the carried-forward `docs/capabilities/network.md` Rust-runtime scope correction; Rust source is unchanged.
- Upstream cumulative integration candidate: Draft PR #169, head `f01f6607bcd1cbf5e28de800ce215187bf23091f`, one commit directly on upstream `main`, with a tree byte-identical to #167 current top.
- Upstream Soroban capability Draft PR #171: `670f39dc6833af350059d5ad0280a3651dacf66d`; Required CI #102 / run `34074383876` passed after replacing the Fresnica-owned ABI value parser with official `soroban-spec-tools`.
- Terminal Soroban product Draft PR #25 final product head: `2bdf19dbea3b9b976fa2883a7cf26bbd366fff79`; CI #81 / run `34074999863`, Release Terminal #58 / run `34074999766`, and live Testnet native-SAC interface probe `34075404068` passed.
- Upstream read-only child Draft PR #172: `a4b0bd4eaa1a47e5f8cb8107ca264bffac6d444b`; Required CI #103 / run `34077833532` passed.
- Terminal read-only child Draft PR #26 code-gate head: `826eef6995943d06e20b011f98af426debeda5c1`; focused CLI tests and live empty-HOME Testnet read-only probe `34078192917` passed. This status record is synchronized in the final PR #26 tree; repository HEAD and CI remain authoritative for the docs-only follow-up head.

Repository source, exact branch heads and CI remain authoritative if this file later drifts. [`docs/operation-foundation.md`](operation-foundation.md) is the v0.5+ architecture constraint for reusable wallet operations; [`docs/plugin-architecture.md`](plugin-architecture.md) remains authoritative for the lower-trust external-plugin boundary. This CURRENT file records implementation state only.

## v0.5 Operation Foundation

The v0.5 main objective is no longer “build a plugin framework.” Fresnica is building a wallet operation base whose business capabilities are headless first and can be consumed by humans, agents, bots, scripts, plugins, TUI and future native clients without duplicating transaction semantics or weakening authorization.

Soroban is the first proof because Contract Spec, simulation and typed Address semantics provide a strong machine-readable substrate. The implementation sequence is: direct contract use -> versioned Contract Store with chain observations -> wallet/contact/contract name resolution -> machine inspection -> stable bounded host capabilities -> only then thin SEP-41 or ecosystem consumers.

Plugins remain ordinary lower-trust orchestration processes. They should become cheaper as the Operation Foundation improves; they must not become alternate wallet implementations.

Current Terminal implementation upgrades the former flat network-scoped contract alias file in place to `fresnica-contract-store-v1`. Legacy arrays remain readable and migrate on first write. Saved contracts retain exact `C...` identity plus user name and proven observations: executable identity, optional resolved `wasm_hash`, and raw `contractmetav0` Wasm key/value facts. Derived snapshots are explicitly versioned and retain their evidence rather than collapsing it into a trust flag; the first such snapshot is SEP-41 v0.5.1 with separate native-SAC, SEP-47 declaration, and current-interface compatibility evidence. Raw Wasm metadata is self-declared evidence, not a trust conclusion: it is preserved for agents and higher layers but is not promoted to protocol/domain/version claims. First inspect/invoke initializes the observation; later invocation fails closed before authorization/signing if executable identity changes, while metadata enrichment for the same Wasm is allowed and explicit inspect refreshes the reviewed observation. `contract add/list/remove --json` are machine-readable. Contract Address arguments may resolve referenced wallet, contact, or saved-contract names; raw Stellar addresses win and referenced cross-namespace conflicts fail closed.

The Contract Store commands are local operations and remain usable even when Horizon/RPC configuration is invalid. Machine `contract list --json` uses `fresnica-contract-list-v1`, separate from the on-disk Store schema. Final local validation through the SEP-41 derived-capability slice: rustfmt and `git diff --check` PASS; workspace Clippy `-D warnings` PASS; workspace tests PASS (Anchor 7, CLI 70, CLI integration 3, presentation 3, TUI 21); workspace release build PASS; repository boundary PASS at upstream `4dc85568...`. Upstream rust-client validation is 208/208 tests PASS. A fresh-home live Testnet probe of native XLM SAC `CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC` returned and persisted `SEP-41 0.5.1: native_sac=true, sep47_declared=false, interface_compatible=true`.

Active Device Unlock architecture checkpoint:

- Fresnica Passphrase remains the protection/recovery root. Device Unlock is convenience-only for routine protected-software signing. Reveal/export/protection changes and enable/disable continue to require a fresh Fresnica Passphrase.
- New protection recommends at least 15 Unicode characters but no longer enforces that length. A shorter non-empty Passphrase is accepted only after an explicit offline-guessing warning. New protected envelopes use upstream Argon2id v2 (64 MiB, 3 iterations, 1 lane, random 128-bit salt) plus AES-256-GCM; legacy Scrypt v1 envelopes remain readable and are never silently migrated.
- The exact-envelope Device Unlock contract remains signer public key + SHA-256(canonical protected envelope) -> one 32-byte `WalletUnlockKey`. Re-protection changes that slot and invalidates the old enrollment.
- Terminal exposes `wallet device-unlock enable|disable|status NAME`. Non-TTY CLI never activates Device Unlock. `-y` may skip Fresnica transaction confirmation but never platform authentication.
- Write commands do not inspect or present Device Unlock as a product choice. The signing layer automatically uses System Authentication for an enrolled protected signer; otherwise it asks for the Fresnica Passphrase. User cancellation fails closed. Platform unavailability may explicitly require the Fresnica Passphrase.
- Authentication state is lazy and process-local: the first protected signer use may authenticate; successful authentication is reused for the rest of that CLI process. This lets Soroban detached authorization and final envelope signing share one authentication. Process exit destroys the state.
- macOS uses LocalAuthentication plus the user's Login Keychain. Existing physical acceptance remains PASS.
- macOS update-enrollment migration is integrated in v0.4.0. Final physical Mac acceptance passed the agreed prompt budget: stale migration requires two native system prompts and the following normal Send requires one. The path is frozen for v0.4 unless new evidence proves a defect.
- Windows uses Windows Hello `IUserConsentVerifierInterop` plus Credential Manager. Existing physical acceptance remains PASS. `device-unlock enable` performs a real Hello verification before the Fresnica Passphrase.
- Linux uses Polkit `auth_self` as `DeviceAuthenticator` and the user's existing default Secret Service collection as `DeviceSecretStore`. Fresnica never creates, locks or unlocks a keyring.
- Linux embeds the Polkit policy template in the `fresnica` binary. Each Linux UID gets its own action and file: `com.fresnica.device-unlock.authenticate.<uid>` and `/usr/share/polkit-1/actions/com.fresnica.device-unlock.<uid>.policy`. `enable` installs or atomically refreshes it through the current Fresnica binary using the platform privilege prompt.
- `disable` removes the UID policy only after the last Fresnica Device Unlock enrollment in that user's Secret Service is removed. Different users and different `FRESNICA_HOME` values therefore do not break each other. Failed first-time enable performs the same unused-system-support cleanup so a failed enrollment does not leave a policy behind.
- Linux fresh-auth semantics use a non-interactive Polkit preflight before allowing user interaction. An action already authorized without fresh authentication is not accepted as Fresnica System Authentication; Fresnica falls back to the Passphrase instead. No `auth_self_keep` is used.
- VPS policy-loader smoke PASS: a synthetic UID policy installed by the current binary under `/usr/share/polkit-1/actions` was immediately visible through `pkaction` as `any=no`, `inactive=no`, `active=auth_self`, owned `root:root` mode `0644`, then removed with no residue.
- Final local deterministic gate for product `9d3e4e0...`: repository boundary PASS; rustfmt/`git diff --check` PASS; workspace Clippy `-D warnings` PASS; workspace tests PASS (Anchor 7, CLI 53, CLI contract 2, presentation 3, TUI 21); Linux release builds PASS for all three binaries and all report `0.4.0`; Python-to-Rust CLI compatibility 5/5 PASS against exact upstream `6c3388c...`; root/wallet/Anchor help ownership smoke PASS; Windows CLI release cross-build PASS through the release workflow's `cargo xwin` path. Native macOS cross-build cannot be meaningfully executed on this VPS because no Apple SDK/toolchain is installed; previous physical macOS acceptance remains the platform evidence until the v0.4.0 tree receives a Mac smoke.
- Historical Linux fprintd and strong setuid/helper System Auth branches remain architecture/security evidence only; neither design is part of the v0.4.0 product.

## v0.4.0 integrated baseline

- Product version is `0.4.0` for `fresnica`, `fresnica-anchor`, `fresnica-tui`, and `fresnica-terminal-presentation`. Release marker: `releases/terminal-v0.4.0.json`.
- Exact upstream source is `6c3388c84993bf8dfd133638a7c32a37a0769e74`. Native SDK baseline remains v0.3.0 / Native Binding API 3 / Universal SDK API 5 / Core Client API 5; this Terminal release does not redefine that binary ABI.
- Root help is intentionally shallow. Anchor remains a bundled native plugin and owns `fresnica anchor --help`; the root CLI no longer duplicates Anchor subcommand syntax. Wallet details similarly live under `fresnica wallet --help`.
- v0.4.0 was integrated to Main through PR #38 as `420c09c3005ac82dc211b2170e9da546de373800`. Its final pre-merge product head was `fcf3332952c8127feccfb324b2d9c0ff74faaced`; CI, Release Terminal validation and physical macOS acceptance passed before integration. This document does not use GitHub publication/tag state as the v0.5 development baseline.

Active Anchor native-plugin checkpoint:

- Terminal branch: `feat/terminal-anchor-plugin-parity`; validated parity product `f700075628ae0381d0f5e77604eca1f6041ff292`; immediate SEP-6 compatibility product `8c33baaf837d078ed090aba6310a125c788fc027`.
- Pinned upstream experiment: `Fresnica/fresnica@07be0fb4fedbb448ab1538305c1a336378724a43` on `feat/rust-client-anchor-explicit-domain`.
- The old CLI `anchor.rs` built-in and special Anchor dispatch route are removed; `fresnica anchor ...` now uses normal unknown-command dispatch to the bundled `fresnica-anchor`.
- Full Testnet deposit evidence: SEP-24 `56e480db-4c09-433b-95aa-e7269b89cd1a` completed to Stellar tx `c3819ef01a0d53f969c156621fd35fa4b88430758e24923a9b8f1e2f269f0b99`; Fresnica then read `1.11` SRT.
- Live SEP-12 read returned `NEEDS_INFO` with 47 required fields. Physical Ledger SEP-10 and live withdrawal settlement remain unclaimed acceptance items.
- Exact host contract, receive-preflight evidence and packaging smoke: [`docs/anchor-plugin-spike.md`](anchor-plugin-spike.md).
- Live fchain.io compatibility proves the legacy/programmatic SEP-6 branch: deposit returns XRPL address + mandatory Destination Tag; withdrawal returns immediate Stellar `account_id + hash memo` without a transaction id; Fresnica now maps that immediate response into the same interactive host payment review when the user supplied an explicit amount. `/transaction(s)` currently return 404, so fchain remains a legacy SEP-6 subset rather than a current full transaction-lifecycle implementation.
- SEP-59 is recorded as a complementary inbound reusable-account model, not a replacement for SEP-6 withdrawals and not a reason to reject historical reusable-address behavior.
- This milestone has no product PR, Main merge, GitHub CI run or release workflow invocation. Local deterministic validation is the gate.

Active Classic transaction-lifetime checkpoint:

- Terminal branch `feat/terminal-classic-tx-timeout`; lifetime product slice `cbab7acc87477d01459805f2ca6b058589b7495f`; final v0.3.0 convergence continues on the same branch and now pins upstream `main@be12ee185002cc41ac874fa3f969e19d99eaf63c`.
- The 300-second Classic TimeBounds default introduced for interactive signing remains unchanged, but `FresnicaClient` now owns an explicit per-client override. Payment, Trustline and SDEX builders consume it; public/default builders and Soroban keep the 300-second default.
- CLI/TUI expose `--tx-timeout SECONDS` and `FRESNICA_TX_TIMEOUT_SECONDS`; zero is rejected. Payment/Trustline/SDEX reviews show the exact lifetime before signing.
- Native `fresnica-*` dispatch propagates an explicit CLI timeout as Fresnica host policy. Anchor host-owned withdrawal payments therefore use the same Classic lifetime without giving the plugin signing authority.
- `PENDING_TTL_SECONDS=210` remains a separate uncertain-submission reconciliation window and is not changed by this option. Soroban transaction/auth/resource lifetimes are outside this slice.
- Final v0.3.0 local gate after namespace convergence: upstream rust-client 190/190; Terminal boundary/fmt/workspace Clippy `-D warnings` PASS; 7 Anchor plugin + 42 CLI + 2 CLI contract + 3 presentation + 21 TUI tests PASS; all three 0.3.0 release binaries build and report aligned versions; package smoke lists only `anchor` and ignores a synthetic `stellar-*` executable; Python reference CLI compatibility 5/5 PASS with the release workflow's uv 0.12.5/Python 3.11 setup; published v0.2.0 -> local v0.3.0 watch-wallet/default/contact storage upgrade smoke PASS. No development CI/release workflow was triggered.

## Plugin architecture correction — 2026-09-08

The accepted v0.3.0 product architecture is Fresnica-native only:

- `fresnica-*`: external executable plugins for Fresnica ecosystem integrations. They may receive narrowly bounded Fresnica host context/policy but never wallet secrets or unrestricted signer authority.
- `stellar-*` / legacy `soroban-*`: **not auto-dispatched**. The earlier compatibility prototype proved reusable executable-dispatch mechanics, but its developer-tool identity/config model does not justify appearing as a wallet-aware Fresnica command.

Unknown commands use longest matching `fresnica-*` command chain. Built-ins always win. Plugins remain separate from Signer Providers; writes must return through Fresnica review, authorization, signing and submission safety.

Historical Draft #32 `feat/terminal-stellar-plugin-dispatch@bd55984777de5e9058063dd6fb91580b69f471fa` remains evidence for PATH/platform/argv/exit-code mechanics. The native namespace and Anchor consumer turned those mechanics into the actual product: `fresnica-anchor` proves bounded host re-entry for public context, receive readiness, SEP-10 authentication and interactive payment proposals while review, software/Ledger signer selection and signature verification stay inside Fresnica. `fresnica-tui` remains a reserved companion binary and is excluded from plugin discovery.

Official Testnet evidence includes completed SEP-10 token exchange, official SEP-24 browser UI, successful Stellar settlement, typed Fresnica balance confirmation and live SEP-12 read. Physical Ledger SEP-10 and live withdrawal settlement remain acceptance items rather than claimed results.


## v0.3.0 release convergence

- Version: `0.3.0` for `fresnica`, `fresnica-anchor`, `fresnica-tui`, and the shared presentation crate.
- Release marker: `releases/terminal-v0.3.0.json`.
- Exact Fresnica source: `be12ee185002cc41ac874fa3f969e19d99eaf63c`; this is the integrated upstream Main commit and contains no Core/SDK/native-binding product changes beyond the existing Native SDK 0.3.0 ABI baseline.
- Product plugin namespace: `fresnica-*` only. The former automatic `stellar-*` / `soroban-*` fallback is historical architecture evidence and is intentionally not shipped.
- Release package contract: `fresnica`, `fresnica-anchor`, `fresnica-tui`; standalone version/help metadata is available for all shipped executables.
- Repository rules require PR integration, required checks and verified Main commits. Upstream is already integrated through PR #180; Terminal will use a squash integration PR so the v0.3.0 release marker is part of one single-parent verified Main commit.

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
- The previous Anchor exception is closed: `fresnica-anchor` is the real second process boundary over shared Anchor protocol functions, the old CLI Anchor built-in is removed, and wallet authorization/signing remains host-owned through bounded semantic callbacks.
- Official-reuse audit: the Fresnica-owned primitive Soroban ABI parser, manual ContractInstance decode, deprecated Wasm helper path, and Terminal's duplicate invoke-time Contract Spec fetch have been removed.
- Official-reuse audit: `rs-stellar-rpc-client` already supplies `send_transaction` and polling, but its high-level error path does not preserve Fresnica's safety-critical distinction between explicit rejection and an uncertain submission that may already have been accepted. Keep Fresnica pending/reconciliation policy until the official client exposes enough structured transport/submission state.
- Official-reuse audit: community `soroban-client` / `stellar-baselib` overlaps Classic Payment/ChangeTrust/offer construction, but adopting it wholesale would also import a second keypair/signing/crypto and transaction ownership layer. Current direct SDF `stellar-xdr` construction remains the smaller boundary; use the community SDK as a possible conformance oracle, not a runtime replacement, unless its ownership/error model changes.
- Official-reuse audit: the official Wallet SDK covers SEP-1/10/6/12/24/38 but currently has no Rust implementation. The Anchor plugin spike is now the concrete second-process consumer that justifies extraction of Terminal-only orchestration; shared protocol validation remains in `fresnica-client`, while the native plugin owns Anchor command flow and re-enters Fresnica only for bounded wallet capabilities.
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
- expansion/physical acceptance of hardware signing beyond the currently validated Classic paths, including physical Ledger SEP-10 through the Anchor plugin host;
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
