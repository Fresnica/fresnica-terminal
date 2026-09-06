# Terminal Capability Gap Audit

Status: active implementation evidence — Asset Discovery P0 in progress

Baseline: `main@8136ab0cc6090cc3bb611e77f0cf3f434c77c3db`

Current shared-source pin: `Fresnica/fresnica@5f5dc1715fd5a449f538afb6a643100075341732`

This audit starts after the v0.1.1 shared-foundation refactor was closed. It does not reopen that refactor. Payment, Trustline and DEX write semantics already belong to `fresnica-client`; CLI and TUI remain presentation adapters over that boundary.

## Decision rule

A missing Terminal surface is not automatically a missing Fresnica Capability.

Classify each gap as one of:

- **Product surface gap** — shared capability exists; Terminal should expose it without inventing new semantics.
- **Shared Rust capability gap** — stable semantics exist, but `fresnica-client` does not yet provide the reusable Rust implementation needed by CLI/TUI.
- **Candidate semantics** — RefPython/provider evidence exists but the product contract is not mature enough for Terminal to freeze a new generic interface.
- **Provider/platform work** — concrete hardware, browser or OS transport belongs behind a provider boundary rather than in generic Terminal semantics.

## Current Terminal coverage

The current CLI already covers the main Classic wallet path:

- wallet lifecycle, watch-only and signer attachment;
- balance/assets and account history;
- contacts/destination resolution;
- reviewed Payment and Trustline writes;
- SDEX order book, offers, writes, trades, fills and candles;
- Anchor discovery, SEP-10, deposit/withdraw/status/customer flows;
- backup/restore and explicit reveal.

The Rust TUI already covers wallet selection, balances/history, Send, Trustline and SDEX market/offer flows. Its README lagged behind this implementation and is corrected in this branch.

## Gap matrix

| Area | Shared evidence today | Terminal state | Classification | Priority |
| --- | --- | --- | --- | --- |
| Asset Discovery / Catalog | Defined Capability; mature RefPython cache-first catalog + picker; Rust client implementation merged | CLI discovery implemented; TUI picker under validation | Product surface gap | **P0 — active** |
| Classic ledger authorization visibility | Rust client plans exact per-transaction authorization and coordinates local Ed25519 multisig | Submission can use local signer records, but review does not explain required/satisfied signer conditions | Product/API review gap | **P1** |
| Soroban invoke | RefPython simulation/review/submit semantics proven; Rust client has RPC/Soroban lifecycle | No CLI/TUI product surface | Candidate product-flow gap | **P1, after input/review contract is proven** |
| History / Activity | RefPython has richer grouped Activity semantics; catalog keeps maturity Defined | Terminal exposes provider-shaped recent history | Candidate semantics | P2 |
| SEP-53 / Dapp | Core/SDK SEP-53 is normative; generic Dapp request/session model remains Defined | No Terminal Dapp session surface | Candidate semantics | P2; do not invent generic transport |
| External Ledger signer | Provider-neutral boundary + real macOS/Testnet RefPython Ledger evidence | No Terminal hardware adapter | Provider/platform work | P2, demand-driven |
| Passkey / smart account | Concrete provider evidence is Testnet/provider-specific; product model is separate C-account path | No Terminal path | Provider/product work | Not current Terminal priority |
| Anchor TUI parity | CLI Anchor path exists; no second Terminal consumer | TUI has no Anchor flow | Product choice, not shared abstraction gap | Demand-driven |

## P0 — Asset Discovery / Catalog

Current friction is structural: Send, Trustline and DEX operate on exact asset identity but Terminal historically expected the user to already know and type `CODE:GISSUER`. RefPython demonstrated the safer product shape:

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

### Terminal CLI surface — implemented and branch-validated

Terminal now pins the exact merged upstream SHA and adds:

```text
fresnica asset discover [--limit N] [--cached] [--json]
```

Compatibility rules:

- historical top-level `assets` remains the alias for `balance`;
- all Send/Trustline/DEX/Anchor exact `CODE:GISSUER` inputs continue to work unchanged;
- discovery output always exposes exact identity; optional metadata is secondary;
- `--cached` prevents remote refresh;
- JSON output exposes network, refresh state and catalog entries.

The CLI patch passed boundary validation, workspace tests, workspace clippy with `-D warnings`, CLI/TUI release builds and diff-check before its product commit was pushed.

### Terminal TUI surface — validating

The Rust TUI implementation remains presentation-only and consumes the same shared catalog. The intended first slice is:

- `/` on a focused asset/base/counter field opens a picker from local cache immediately;
- `r` explicitly refreshes the bounded catalog;
- Up/Down or `j/k` selects and Enter applies the full exact identity;
- Esc closes the picker and preserves the manually typed field value;
- Trustline selection excludes XLM because native assets do not have trustlines;
- Send, Trustline, market Base/Counter and offer Base/Counter reuse the same TUI picker state;
- manual exact identity entry remains available at all times.

This follows RefPython's cache-first UX evidence without introducing a background-worker abstraction solely for parity. Search is deliberately deferred from the first Rust TUI slice; the catalog is bounded to 50 and manual exact entry remains authoritative.

## P1 — Ledger authorization visibility

The Rust client already supports local Classic multisig better than Terminal currently communicates. Authorization is transaction-specific, not a generic `canSign(account)` property.

A future review improvement should expose an immutable/snapshot authorization summary associated with the prepared transaction, while submission still refreshes ledger authorization before signing. The UI should be able to explain, for example:

```text
required medium threshold: 2
already satisfied: 0
local Ed25519 capability available: signer A + signer B
remaining unsupported condition: none
```

This needs careful upstream API design because signer/ledger state can change between review and submit. Do not bolt a generic account signer list onto Terminal and call it transaction authorization.

## P1 — Soroban invoke

The technical substrate is substantial: RefPython proved simulation/assembly/review/authorization/submission and `fresnica-client` exposes the Rust RPC/Soroban lifecycle. Terminal should nevertheless not freeze an arbitrary CLI syntax for raw `ScVal` arguments merely because the lower layer exists.

Before implementation, prove a stable product input/review shape for common contract argument types or an explicit expert XDR mode. Reintroducing direct `stellar-xdr` parsing into the CLI would reverse the v0.1.1 boundary cleanup unless the responsibility is deliberately placed in the shared Rust client.

## Explicit non-goals for P0

- no second Payment/Trustline/DEX Flow layer;
- no generic Dapp transport/session framework;
- no universal smart-account/provider abstraction;
- no hardware HID code in Core or generic Rust capability semantics;
- no code-only asset identity;
- no removal of exact/manual asset entry;
- no broad Activity DTO promotion as a side effect of asset discovery;
- no recommendation/ranking engine disguised as the catalog;
- no TUI async framework solely to mimic RefPython background refresh.

## Current completion gate

Asset Discovery P0 is complete only when:

1. the final cache-first TUI picker passes focused tests, workspace clippy/tests and both release builds;
2. temporary probe/verifier files are absent from the Terminal product branch;
3. the exact branch diff is reviewed against `main@8136ab0cc6090cc3bb611e77f0cf3f434c77c3db`;
4. formal Terminal PR CI and Release validation pass on the exact final head;
5. the final squash commit on `main` is GitHub Verified and post-merge CI passes.
