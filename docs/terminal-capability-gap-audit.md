# Terminal Capability Gap Audit

Status: P1 Classic ledger authorization visibility implemented and validated on feature branches — not merged or released

Audit baseline: `main@ba7e24bbd3e6f527d4d56029a572ac8d6851015c`

Release commit: `main@a2485cad5d2d6048f8ffb6987597c2a3fca2670d`

Current shared-source pin: `Fresnica/fresnica@7ea101bce034e85ace3260ff044c983bd5d33cdb` (`feat/ledger-authorization-review-snapshot`, not merged to upstream `main`)

This audit starts after the v0.1.1 shared-foundation refactor was closed. It does not reopen that refactor. Payment, Trustline and DEX write semantics belong to `fresnica-client`; CLI and TUI remain presentation adapters over that boundary.

## Decision rule

A missing Terminal surface is not automatically a missing Fresnica Capability.

Classify each gap as one of:

- **Product surface gap** — shared capability exists; Terminal should expose it without inventing new semantics.
- **Shared Rust capability gap** — stable semantics exist, but `fresnica-client` does not yet provide the reusable Rust implementation needed by CLI/TUI.
- **Candidate semantics** — RefPython/provider evidence exists but the product contract is not mature enough for Terminal to freeze a new generic interface.
- **Provider/platform work** — concrete hardware, browser or OS transport belongs behind a provider boundary rather than in generic Terminal semantics.

## Current Terminal coverage

The current CLI covers the main Classic wallet path:

- wallet lifecycle, watch-only and signer attachment;
- balance/assets and account history;
- contacts/destination resolution;
- reviewed Payment and Trustline writes;
- SDEX order book, offers, writes, trades, fills and candles;
- Anchor discovery, SEP-10, deposit/withdraw/status/customer flows;
- backup/restore and explicit reveal;
- cache-first exact Asset Discovery.

The Rust TUI covers wallet selection, balances/history, Send, Trustline and SDEX market/offer flows, plus cache-first exact asset selection for the asset fields used by those flows.

## Gap matrix

| Area | Shared evidence today | Terminal state | Classification | Priority |
| --- | --- | --- | --- | --- |
| Asset Discovery / Catalog | Defined Capability; mature RefPython cache-first catalog + picker; Rust client implementation merged | CLI discovery and TUI cache-first picker released in v0.2.0 | Complete product surface | **P0 — complete** |
| Classic ledger authorization visibility | Rust client exposes a prepared-transaction authorization snapshot and still refreshes authorization at submit | CLI/TUI review consumes the shared snapshot; no Terminal-side threshold/signer interpretation | Implemented product surface on feature branches | **P1 — feature-complete, unmerged** |
| Soroban invoke | RefPython simulation/review/submit semantics proven; Rust client has RPC/Soroban lifecycle | No CLI/TUI product surface | Candidate product-flow gap | **P1, after input/review contract is proven** |
| History / Activity | RefPython has richer grouped Activity semantics; catalog keeps maturity Defined | Terminal exposes provider-shaped recent history | Candidate semantics | P2 |
| SEP-53 / Dapp | Core/SDK SEP-53 is normative; generic Dapp request/session model remains Defined | No Terminal Dapp session surface | Candidate semantics | P2; do not invent generic transport |
| External Ledger signer | Provider-neutral boundary + real macOS/Testnet RefPython Ledger evidence | No Terminal hardware adapter | Provider/platform work | P2, demand-driven |
| Passkey / smart account | Concrete provider evidence is Testnet/provider-specific; product model is separate C-account path | No Terminal path | Provider/product work | Not current Terminal priority |
| Anchor TUI parity | CLI Anchor path exists; no second Terminal consumer | TUI has no Anchor flow | Product choice, not shared abstraction gap | Demand-driven |

## P0 — Asset Discovery / Catalog

The original friction was structural: Send, Trustline and DEX operate on exact asset identity but Terminal historically expected the user to already know and type `CODE:GISSUER`. RefPython demonstrated the safer product shape:

```text
cached exact identities
        +
optional remote catalog metadata
        +
selection
        +
always-available manual exact identity
```

The shared Capability fixes these invariants:

1. exact identity remains `XLM` or `CODE:GISSUER`;
2. code/name/domain never replaces issuer identity;
3. discovery is network-scoped;
4. remote provider failure must not block cached/manual selection;
5. discovery/catalog data is distinct from product recommendation/ranking.

### Shared Rust foundation — complete

Upstream PR #155 implemented the first independent Rust Asset Discovery boundary and squash-merged as:

`Fresnica/fresnica@5f5dc1715fd5a449f538afb6a643100075341732`

Evidence:

- `AssetCatalogEntry` preserves exact `AssetId` identity plus optional domain/name/organization/source metadata;
- `FresnicaClient::asset_catalog(limit, refresh)` provides network-scoped cache-first access;
- XLM remains native identity and issued identities are revalidated through the existing `AssetId` parser;
- mainnet refresh is bounded and provider choice remains an implementation detail;
- provider failure or unusable remote results fall back to valid cache;
- testnet/non-mainnet never imports mainnet recommendations;
- corrupt cache fails explicitly instead of silently replacing identity data;
- provider/cache results are de-duplicated by exact identity;
- Required CI for PR #155 passed and post-merge `Main bundle #65` passed;
- the squash commit is GitHub Verified.

No Core, SDK, Native/binding or new application-Flow authority was added.

### Terminal CLI surface — released

Terminal pins the exact merged upstream SHA and provides:

```text
fresnica asset discover [--limit N] [--cached] [--json]
```

Compatibility rules:

- historical top-level `assets` remains the alias for `balance`;
- all Send/Trustline/DEX/Anchor exact `CODE:GISSUER` inputs continue to work unchanged;
- discovery output always exposes exact identity; optional metadata is secondary;
- `--cached` prevents remote refresh;
- JSON output exposes network, refresh state and catalog entries.

### Terminal TUI surface — released

The Rust TUI remains presentation-only and consumes the same shared catalog:

- `/` on a focused asset/base/counter field opens a picker from local cache immediately;
- `r` explicitly refreshes the bounded catalog;
- Up/Down or `j/k` selects and Enter applies the full exact identity;
- Esc closes the picker and preserves the manually typed field value;
- Trustline selection excludes XLM because native assets do not have trustlines;
- Send, Trustline, market Base/Counter and offer Base/Counter reuse the same TUI picker state;
- manual exact identity entry remains available at all times.

This follows RefPython's cache-first UX evidence without introducing a background-worker abstraction solely for parity. Search is deliberately deferred from the first Rust TUI slice; the catalog is bounded to 50 and manual exact entry remains authoritative.

Disposable TUI probe run `34010167207` passed 19/19 focused tests, workspace clippy with warnings denied, workspace tests, CLI/TUI release builds and exact diff-check. No probe workflow or patch script entered the product tree.

### Terminal integration and release evidence

Feature PR #8:

- clean feature head: `3acf9e2586684c64759307361b0fe5c4cd042d3f`;
- CI #42 and Release Terminal #24 passed, including Linux/macOS/Windows packaging and Windows smoke;
- squash merge: `861ce5a21dbb73a667e57f8b3478e96bbe0bb530`;
- merge commit is GitHub Verified;
- post-merge CI #43 passed.

Release PR #9:

- release head: `cc4afbda0adbef2e0af8c0d91545604cc8246270`;
- CI #44 and Release Terminal #25 passed, including Windows smoke;
- squash merge / v0.2.0 release commit: `a2485cad5d2d6048f8ffb6987597c2a3fca2670d`;
- release commit is GitHub Verified;
- post-merge CI #45 passed;
- Release Terminal #26 passed and published prerelease tag `v0.2.0` targeting exactly `a2485cad5d2d6048f8ffb6987597c2a3fca2670d`.

Published v0.2.0 SHA-256:

```text
b957de4e007ed03df7edfdb414036db7a53ab8f29b52fcc912292be9680e6a4b  fresnica-terminal-0.2.0-linux-x64.zip
00a804548ea6d26ec2d009a5995a0f18b9c4d38529657c20bdef8aa0f967584f  fresnica-terminal-0.2.0-macos-arm64.zip
9de90ce2b4d888dae243eba6472f195e066b7e2c048c7b21b97d1704dad1ccc3  fresnica-terminal-0.2.0-windows-x64.zip
c6979a5ff77325e353026a7b8727e3416164f0505a9dbb239e002b58a24c233f  fresnica-terminal-release-manifest.json
80a69fdb3edd3a18017929773e63d8f6034084cb6f20d13dcd5e643e50cbaf2a  SHA256SUMS
```

The three platform ZIPs were independently downloaded from the exact Release #26 workflow artifacts and re-hashed. The publish job's deterministic manifest/checksum procedure was then reproduced from those exact ZIP hashes, the release commit and `FRESNICA_REV`; the reconstructed manifest and `SHA256SUMS` have the same byte lengths and SHA-256 digests reported by GitHub Release.

## P1 — Ledger authorization visibility

The upstream audit confirmed that the authorization semantics already belonged to the shared Rust client. `LedgerAuthorizationPlan` is transaction-specific, operation-source-aware and typed-signer-aware; the shared submit path independently reloads current ledger authorization before coordinating signatures. RefPython remains narrower and is not the normative source for this capability.

The missing contract was review evidence, not another signing model. Upstream feature commit:

`Fresnica/fresnica@7ea101bce034e85ace3260ff044c983bd5d33cdb`

adds `LedgerAuthorizationSnapshot`, bound to the prepared transaction hash. It reports, per required source account:

- required authorization weight;
- weight already satisfied by authorization material on the envelope;
- additional locally available software-Ed25519 weight;
- weight still remaining after local capability;
- the transaction/operation authorization uses that produced the requirement;
- typed signer condition availability: already satisfied, local Ed25519, or unavailable locally;
- mandatory transaction-level `extraSigners` with the same typed availability classification.

Payment, Trustline and SDEX Offer reviews carry this snapshot directly. Their prepare paths reuse the Horizon account state already fetched for transaction construction/preflight; P1 does not add a second Horizon request solely for review.

Terminal consumes that shared evidence without reconstructing ledger semantics. CLI review prints the full transaction hash and signer keys. TUI review shows the same required/satisfied/local/remaining weights and typed conditions, compacting long public keys/hashes only for display width. Example shape:

```text
Authorization: local signing ready
  G...: required 2 · satisfied 0 · local 2 · remaining 0
    operation 1 Payment: medium threshold 2
    local Ed25519 G... (weight 1)
    local Ed25519 G... (weight 1)
Prepared tx: <transaction hash>
```

The snapshot is deliberately review-only. Submission does **not** trust it: `sign_and_submit` reloads current Horizon authorization and recomputes the plan before selecting local signatures. This preserves the state-change boundary between review and submit.

No Core, SDK, Native/binding, generic signer-provider abstraction or Terminal-local multisig model was added. RefPython was not widened merely to manufacture parity.

Disposable Terminal validation run `34014521234` passed the repository boundary check, locked workspace format/clippy/tests, CLI/TUI release builds and Rust CLI ↔ RefPython compatibility. The disposable verifier is removed when the validated product patch is collapsed; it is not part of the product tree.

## P1 — Soroban invoke

The technical substrate is substantial: RefPython proved simulation/assembly/review/authorization/submission and `fresnica-client` exposes the Rust RPC/Soroban lifecycle. Terminal should nevertheless not freeze an arbitrary CLI syntax for raw `ScVal` arguments merely because the lower layer exists.

Before implementation, prove a stable product input/review shape for common contract argument types or an explicit expert XDR mode. Reintroducing direct `stellar-xdr` parsing into the CLI would reverse the v0.1.1 boundary cleanup unless the responsibility is deliberately placed in the shared Rust client.

## Explicit non-goals carried forward

- no second Payment/Trustline/DEX Flow layer;
- no generic Dapp transport/session framework;
- no universal smart-account/provider abstraction;
- no hardware HID code in Core or generic Rust capability semantics;
- no code-only asset identity;
- no removal of exact/manual asset entry;
- no recommendation/ranking engine disguised as the catalog.

## P0 completion gate — satisfied

1. shared Rust catalog merged and verified;
2. CLI and cache-first TUI surfaces validated and merged;
3. no temporary verifier files entered the product tree;
4. feature PR, release PR, post-merge CI and cross-platform Release workflows passed;
5. v0.2.0 targets the exact GitHub-Verified release commit;
6. published platform ZIP hashes were independently rechecked against Release #26 artifacts;
7. release manifest and `SHA256SUMS` were deterministically reconstructed and matched GitHub's published digests.

## P1 implementation gate — satisfied on feature branches

1. upstream authorization API and RefPython evidence audited before Terminal implementation;
2. shared Rust review snapshot implemented and tested without changing submit-time refresh semantics;
3. Payment, Trustline and SDEX Offer reviews carry transaction-specific authorization evidence;
4. CLI/TUI render the shared snapshot without recalculating thresholds or signer sufficiency;
5. Terminal is pinned to the exact upstream feature commit while both changes remain unmerged;
6. repository boundary, format, clippy, tests, release builds and Python compatibility passed;
7. no disposable verifier files remain in either feature tree.

Next decision point: **hold merge/release and review the two P1 feature branches**. Do not start the Soroban invoke surface merely because P1 is implemented; decide the integration disposition first.
