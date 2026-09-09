# Fresnica Terminal Current State

Status: **v0.3.0 released; one-shot System Auth is the active post-release slice. Shared Classic/Soroban signing coordination and the macOS high-trust provider boundary are implemented; signed macOS packaging and physical user-presence acceptance remain open.**

Last verified: 2026-09-09.

## Source of truth

- Current Terminal `main`: `d32e0755e39018ee04a5a30f86102e6d9c9539e9`; released preview `v0.3.0`. Main CI #109 and Release Terminal #86 passed.
- Active Terminal branch: `feat/terminal-system-auth`; current checkpoint `81b45728667070fbade73308300f38b3716aa22d`; exact upstream pin `ecdc6bf77741949e4f7043153143824ae78a1b44`.
- Released upstream Main baseline: `Fresnica/fresnica@be12ee185002cc41ac874fa3f969e19d99eaf63c`. Active upstream System Auth branch `feat/system-auth-sdk-boundary`: Rust runtime/product `f8434d8ecfcc5c3ab7d7f61f4566cd79f1cb50d8`; current Apple/provider head `ecdc6bf77741949e4f7043153143824ae78a1b44`.
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

Repository source, exact branch heads and CI remain authoritative if this file later drifts. Durable plugin namespace/trust decisions are authoritative in [`docs/plugin-architecture.md`](plugin-architecture.md); this CURRENT file records implementation state only.

Active System Auth architecture checkpoint:

- Strong Passphrase remains the wallet protection/recovery root; the 15-scalar minimum is unchanged. `wallet system-auth enable|disable NAME` require a fresh Passphrase. Reveal/Export remain fresh-Passphrase-only.
- Upstream runtime `f8434d8...` exposes exact-envelope `SystemAuthSlot`, structured final outcomes (`UnlockKey`, `PassphraseRequired`, `Cancelled`, fail-closed error), Classic signing coordination, Soroban detached-G authorization and final envelope signing, plus the Apple provider entrypoint. Core and SDK APIs did not change.
- Terminal `81b4572...` uses the same one-shot source for Payment, Trustline, SDEX, Anchor SEP-10/host payments and contract writes, and adds the signed macOS companion development path. Non-TTY invocation never activates System Auth; no CLI session/cache exists.
- OS/provider owns biometric retry and biometric -> device credential fallback. Explicit authentication exhaustion/unavailability may request a fresh Fresnica Passphrase; user cancellation aborts without a surprise password prompt; stale signer/envelope, wrong unlock key, malformed provider output or provider-integrity failure fail closed. Passphrase fallback drops System Auth providers for the retry.
- Exact enrollment identity remains `signer public key + SHA-256(canonical protected envelope)`, so re-protection/passphrase rotation cannot silently reuse enrollment.
- macOS uses a reserved high-trust sibling `FresnicaSystemAuth.app`, not PATH discovery. Only slot id is passed as argv; the verified 32-byte unlock key travels over private stdin/stdout pipes. The provider receives no Passphrase, mnemonic, S-key or generic signing authority. `fresnica-system-auth-provider` is reserved from ordinary plugin discovery.
- Apple provider policy is `deviceOwnerAuthentication` / `userPresence`, allowing OS biometric retries and device-passcode fallback. The EC domain key is created and retrieved in the Data Protection Keychain and is gated by `userPresence`. The development wrapper now uses a shared Xcode scheme and explicitly selects the current Mac as the signing destination so automatic provisioning can register it; unsigned CI never claims functional System Auth.
- Validation: upstream Required CI #144 PASS; Terminal local boundary/fmt/workspace Clippy `-D warnings`; 7 Anchor + 50 CLI + 2 CLI contract + 3 presentation + 21 TUI tests PASS. Release Terminal #92 PASS, including the macOS shared-scheme provider compile/package gate. Process-provider tests prove raw-key pipe transport and preservation of passphrase-required/cancel/fatal outcomes.
- Remaining acceptance: rerun the development installer on a real Mac so Xcode registers/provisions that Mac, then complete physical user-context Touch ID/Face ID/system-password enable/release tests. Windows and Linux providers remain separate later platform slices.

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
