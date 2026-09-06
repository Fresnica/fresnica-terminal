# Terminal Capability Gap Audit

Status: active product-planning evidence

Baseline: `main@8136ab0cc6090cc3bb611e77f0cf3f434c77c3db`

Shared-source pin: `Fresnica/fresnica@9ba6f23cefe34e8d5940b311ec78f27eed982fe7`

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

The Rust TUI already covers wallet selection, balances/history, Send, Trustline and SDEX market/offer flows. Its README lagged behind this implementation and is corrected in this audit branch.

## Gap matrix

| Area | Shared evidence today | Terminal state | Classification | Priority |
| --- | --- | --- | --- | --- |
| Asset Discovery / Catalog | Defined Capability; mature RefPython cache-first catalog + reusable asset picker | Exact `CODE:GISSUER` must normally be typed manually | Shared Rust capability gap + product surface gap | **P0** |
| Classic ledger authorization visibility | Rust client plans exact per-transaction authorization and coordinates local Ed25519 multisig | Submission can use local signer records, but review does not explain required/satisfied signer conditions | Product/API review gap | **P1** |
| Soroban invoke | RefPython simulation/review/submit semantics proven; Rust client has RPC/Soroban lifecycle | No CLI/TUI product surface | Candidate product-flow gap | **P1, after input/review contract is proven** |
| History / Activity | RefPython has richer grouped Activity semantics; catalog keeps maturity Defined | Terminal exposes provider-shaped recent history | Candidate semantics | P2 |
| SEP-53 / Dapp | Core/SDK SEP-53 is normative; generic Dapp request/session model remains Defined | No Terminal Dapp session surface | Candidate semantics | P2; do not invent generic transport |
| External Ledger signer | Provider-neutral boundary + real macOS/Testnet RefPython Ledger evidence | No Terminal hardware adapter | Provider/platform work | P2, demand-driven |
| Passkey / smart account | Concrete provider evidence is Testnet/provider-specific; product model is separate C-account path | No Terminal path | Provider/product work | Not current Terminal priority |
| Anchor TUI parity | CLI Anchor path exists; no second Terminal consumer | TUI has no Anchor flow | Product choice, not shared abstraction gap | Demand-driven |

## P0 — Asset Discovery / Catalog

This is the clearest next bounded product milestone.

Current friction is structural: Send, Trustline and DEX operate on exact asset identity but Terminal mostly expects the user to already know and type `CODE:GISSUER`. RefPython has already demonstrated a safer product pattern:

```text
cached exact identities
        +
optional remote catalog metadata
        +
search / selection
        +
always-available manual exact identity
```

The shared Capability already fixes the important invariants:

1. exact identity remains `XLM` or `CODE:GISSUER`;
2. code/name/domain never replaces issuer identity;
3. discovery is network-scoped;
4. remote provider failure must not block cached/manual selection;
5. discovery/catalog data is distinct from product recommendation/ranking.

What is missing is the reusable Rust implementation. Because both Rust CLI and TUI need the same cache/fallback/identity semantics, this belongs in `reference/rust-client`, not a Terminal-local service.

### Proposed first slice

Upstream `fresnica-client`:

- add an `AssetCatalogEntry` model preserving exact `AssetId` identity plus optional metadata/provenance;
- add a cache-first catalog service rooted under the existing Fresnica home;
- mainnet may refresh from one bounded public provider initially; provider choice remains implementation detail;
- testnet/non-mainnet remains manual/cache/native without pretending mainnet recommendations apply;
- corrupt cache fails explicitly, remote refresh failure falls back to valid cache;
- do not encode recommendation policy into the shared service.

Terminal:

- CLI: add an explicit discovery/list surface useful for scripting and inspection; keep all existing exact-identity command forms working;
- TUI: use one reusable asset-selection presentation for Trustline and SDEX selection first, then Send where appropriate;
- always retain manual exact identity entry;
- show enough issuer/domain/source information to avoid code-only ambiguity.

The first implementation should not copy every RefPython provider or cache-schema detail merely for parity. Independent Rust evidence is more useful if it implements the agreed invariants with a small surface.

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

The technical substrate is substantial: RefPython proved simulation/assembly/review/authorization/submission and `fresnica-client` now exposes the Rust RPC/Soroban lifecycle. Terminal should nevertheless not freeze an arbitrary CLI syntax for raw `ScVal` arguments merely because the lower layer exists.

Before implementation, prove a stable product input/review shape for common contract argument types or an explicit expert XDR mode. Reintroducing direct `stellar-xdr` parsing into the CLI would reverse the v0.1.1 boundary cleanup unless the responsibility is deliberately placed in the shared Rust client.

## Explicit non-goals for the next milestone

- no second Payment/Trustline/DEX Flow layer;
- no generic Dapp transport/session framework;
- no universal smart-account/provider abstraction;
- no hardware HID code in Core or generic Rust capability semantics;
- no code-only asset identity;
- no removal of exact/manual asset entry;
- no broad Activity DTO promotion as a side effect of asset discovery.

## Next bounded milestone

**Asset Discovery foundation:** implement the smallest reusable cache-first Rust catalog in `Fresnica/fresnica`, validate it independently, then update the Terminal exact source pin and expose the capability without changing existing manual command compatibility.

Use a dedicated upstream branch/PR first. Only after that upstream commit is merged should Terminal pin it and implement CLI/TUI presentation.
