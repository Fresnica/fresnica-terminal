# Fresnica Native Rust CLI

This client consumes the platform-neutral `fresnica-sdk` semantic boundary and
links the Rust libraries into one `fresnica` executable. It remains a direct
native reference client without a process/FFI transport.

## Current scope

The native client currently covers local wallet lifecycle and contacts, testnet
Friendbot funding, read-only ledger queries, cache-first asset discovery, reviewed payments, issued-asset
trustline lifecycle, Classic SDEX read/write/history operations, contract-spec-driven Soroban invocation, and Stellar anchor flows:

- `info [--wallet NAME]`
- `account [--wallet NAME] [--json]`
- `balance [--wallet NAME] [--json]` (`assets` is an alias)
- `history [--wallet NAME] [--limit N] [--json]`
- `asset discover [--limit N] [--cached] [--json]`
- `send AMOUNT ASSET to DESTINATION [--wallet NAME] [--memo TEXT] [-y]`
- `contact list`
- `contact add NAME G... [--memo TEXT]`
- `contact remove NAME`
- `trust add CODE:GISSUER [--limit VALUE] [--wallet NAME] [-y]`
- `trust limit CODE:GISSUER LIMIT [--wallet NAME] [-y]`
- `trust remove CODE:GISSUER [--wallet NAME] [-y]`
- `dex orderbook SELLING BUYING [--json]`
- `dex offers [--wallet NAME] [--limit N] [--json]`
- `dex buy BASE COUNTER AMOUNT PRICE [--wallet NAME] [--allow-trustline] [-y]`
- `dex sell BASE COUNTER AMOUNT PRICE [--wallet NAME] [--allow-trustline] [-y]`
- `dex update OFFER_ID BASE COUNTER AMOUNT PRICE [--wallet NAME] [-y]`
- `dex cancel OFFER_ID [--wallet NAME] [-y]`
- `dex trades BASE COUNTER [--limit N] [--json]`
- `dex fills [--wallet NAME] [--limit N] [--json]`
- `dex candles BASE COUNTER [--resolution 1m|5m|15m|1h|1d|1w] [--start MS] [--end MS] [--offset MS] [--limit N] [--json]`
- `contract invoke C... [--wallet NAME] [-y] [--json] -- FUNCTION [--NAME VALUE]...`
- `anchor discover CODE:GISSUER --home-domain DOMAIN [--json]`
- `anchor auth CODE:GISSUER --home-domain DOMAIN [--wallet NAME] [--json]`
- `anchor deposit CODE:GISSUER --home-domain DOMAIN [--wallet NAME] [--field NAME=VALUE]... [--json]`
- `anchor withdraw CODE:GISSUER --home-domain DOMAIN [--wallet NAME] [--field NAME=VALUE]... [--json]`
- `anchor status CODE:GISSUER ID --home-domain DOMAIN [--wallet NAME] [--protocol sep24|sep6] [--pay] [--json]`
- `anchor customer CODE:GISSUER --home-domain DOMAIN [--wallet NAME] [--id CUSTOMER_ID] [--transaction ID] [--type TYPE] [--lang LANG] [--input PATH|-] [--json]`
- `wallet list`
- `wallet use NAME`
- `wallet create NAME`
- `wallet import-secret NAME`
- `wallet import-mnemonic NAME`
- `wallet import-watch NAME G...`
- `wallet import-ledger NAME [--hd-path N]`
- `wallet attach-ledger NAME [--hd-path N]`
- `wallet detach-ledger NAME`
- `wallet attach-secret NAME`
- `wallet attach-mnemonic NAME [--index N] [--language LANGUAGE]`
- `wallet detach-signer NAME`
- `wallet system-auth enable NAME`
- `wallet system-auth disable NAME`
- `wallet system-auth status NAME`
- `wallet testnet-fund [--wallet NAME]` (`wallet fund` is an alias)
- `wallet reveal [NAME]`
- `wallet backup NAME PATH`
- `wallet restore PATH [--name NAME]`
- `wallet delete NAME`
- `plugin ls`

It reads and writes the same wallet record files, `.default` pointer,
`contacts.json`, and `fresnica-wallet-backup` version-1 format as the Python
reference client. The default application home is `FRESNICA_HOME` when set,
otherwise `~/.fresnica`.

The selected Stellar network and provider endpoints are separate runtime concerns. By default the shared `fresnica-client` profile uses Fresnica's mainnet/testnet Horizon endpoint; Testnet also has the shared Stellar RPC default while Mainnet contract invocation requires an explicit RPC endpoint until Fresnica intentionally adopts a stable default. Set `FRESNICA_HORIZON_URL` / `--horizon-url URL` for Horizon and `FRESNICA_RPC_URL` / `--rpc-url URL` for Stellar RPC; command-line values win. Provider overrides never change the selected Stellar network identity or signing passphrase.

Classic transaction TimeBounds are a separate client policy. Payment, Trustline and SDEX writes default to a 300-second validity window so interactive software/Ledger signing has time to complete. Use global `--tx-timeout SECONDS` or `FRESNICA_TX_TIMEOUT_SECONDS` to override that window; the CLI value wins, zero is rejected, and the exact selected lifetime is shown in write review before signing. The same policy applies when a native plugin returns a host-owned Classic payment proposal, including Anchor withdrawal payment. It does not change Soroban transaction/auth TTLs or the independent 210-second uncertain-submission recovery window.

Asset discovery preserves exact Stellar identity: `XLM` or `CODE:GISSUER` remains
authoritative, while domain/name/organization/source are optional metadata only.
`asset discover` refreshes the bounded mainnet catalog through the shared
`fresnica-client` service and falls back to a valid network-scoped local cache;
`--cached` suppresses remote refresh. Non-mainnet use never imports mainnet
recommendations. Discovery does not replace any existing manual exact-identity
input in Send, Trustline, SDEX, or Anchor commands.

Create/import/reveal cryptography and account identity parsing go through
`fresnica-sdk`; Core remains the cryptographic authority underneath the SDK.
Secret, mnemonic, BIP39-passphrase, and Fresnica-passphrase prompts are read from
the controlling terminal with input hidden; they are not accepted as command-line
arguments.

A watch-only Classic account can later attach a secret, mnemonic, or Ledger signer without
changing wallet identity. Software signer attachment passes the existing G address as the SDK
`expected_signer_public_key`; mismatched material is rejected before the wallet record changes.
`wallet import-ledger` / `attach-ledger` read the public key from the connected Stellar Ledger
app at `m/44'/148'/N'` (default `N=0`) and persist only public provider metadata in the existing
wallet record. `wallet detach-ledger` removes only that metadata. `wallet detach-signer` removes
only local protected software signing material after passphrase verification.

System Auth is an optional device-local convenience for an exact protected software signer; the strong Fresnica Passphrase remains the protection/recovery root. Enable/disable require a fresh Passphrase. Interactive CLI signing may then use one-shot OS authentication for Payment, Trustline, SDEX, Anchor SEP-10/host payments, detached Classic Soroban authorization, and the final Soroban envelope. Non-TTY CLI invocation never activates System Auth. OS biometric retries and device-credential fallback remain provider policy; explicit authentication exhaustion may fall back to a fresh Fresnica Passphrase, user cancellation aborts the operation, and stale/invalid signer-envelope or unlock-key state fails closed. Reveal/Export remain fresh-Passphrase-only.

macOS uses a reserved first-party companion provider located beside `fresnica`, not a PATH plugin. The provider owns Data Protection Keychain / LocalAuthentication and receives only an exact slot id plus the verified 32-byte `WalletUnlockKey` over private pipes. Production packaging requires an Apple-signed app-like wrapper with the required provisioning profile; the source/compile gate is present but an unsigned helper is intentionally not shipped as if it were functional. Linux external System Auth providers remain future explicitly trusted providers, never ordinary `fresnica-*` plugins.

Ledger signing is intentionally bounded to Classic transaction writes currently exposed by Send,
Trustline and SDEX offer commands. Transaction preparation, authorization weight selection and
signature application remain in `fresnica-client` / SDK / Core; Terminal reuses SDF's
`stellar-ledger` HID/APDU implementation only for device public-key lookup and clear-signing. A
connected device is re-checked against the recorded public key before every signature. Mixed
software + Ledger multisig preflights the Fresnica passphrase before any device signing request.
Anchor SEP-10 now reuses the same provider-aware Classic signing coordination and can select a matching Ledger provider without exposing it to the plugin; physical Ledger SEP-10 remains an unverified acceptance item. Soroban authorization, SEP-53 and generic Dapp sessions are not part of this Ledger slice.

Contacts are client-local public metadata. Contact names are resolved before
payment construction, an explicit `--memo` takes precedence over a contact's
default memo, and transaction review always shows the resolved G address even
when the user entered an alias.

Friendbot is a testnet-only client utility. It funds the selected testnet address
directly through `friendbot.stellar.org` with a 15-second request timeout and does
not require signing material, so watch-only testnet wallets are valid targets.

Account state, balances, recent operations, SDEX reads, transaction preparation,
and provider submission are client responsibilities. Reusable Rust application
semantics live in `fresnica-client`, which consumes the resolved network profile and
Horizon/RPC endpoints; none of that HTTP/RPC or product policy is moved into `fresnica-core`.

Reviewed write commands present operation-specific review and ask for
confirmation before requesting the Fresnica passphrase. Payment preparation, its
review DTO, submission, and pending-retry protection are shared through
`fresnica-client`; CLI parsing, rendering, confirmation, and hidden passphrase input
remain terminal-owned. The exact prepared XDR is then passed to the SDK composite
passphrase-signing operation, so routine CLI signing does not expose a raw
`WalletUnlockKey` outside the Rust SDK/Core call.
If the HTTP submission result is uncertain,
the native client persists the locally computed transaction hash and blocks a
later same-account write until Horizon confirms it or the 210-second recovery
window expires after a not-found lookup.

Pending recovery state uses the same `pending-transactions.json` path and public
metadata schema as the Python reference. It stores only network, account,
transaction hash, kind, and submission time; signed XDR, secrets, passphrases,
unlock keys, and signer material are never persisted there. Horizon lookup
failures leave the pending record intact, and failure to persist a newly uncertain
submission produces an explicit do-not-retry warning.

The client transaction builder supports multiple operations when product
semantics require an atomic bundle. SDEX creation uses this only when the user
explicitly approves a missing receiving trustline with `--allow-trustline`, in
which case `ChangeTrust + ManageBuyOffer/ManageSellOffer` are reviewed and signed
as one transaction.

Trustline policy matches the Python reference: add reserves one additional base
reserve, the default limit is `708269837873.6765`, limit changes cannot go below
balance plus buying liabilities, and removal requires zero balance and zero
liabilities.

SDEX semantics match the Python reference and Fresnica/Fex presentation:
BASE/COUNTER is stable, price is COUNTER per BASE, BID/BUY is on the left and
ASK/SELL is on the right. Horizon BID amounts are normalized back to BASE units
using exact `price_r`. BUY uses `ManageBuyOffer` with counter as selling asset and
base as buying asset; SELL uses `ManageSellOffer` with base as selling asset.
Updates must preserve the current pair and side. Cancellation uses the ledger's
stored selling/buying orientation and does not depend on remembering the original
operation type.

Pair `trades` and `candles` are direct online Horizon projections. Wallet `fills`
use the same offer-level aggregation rule as the Python reference: only
consecutive trades with the same identified user offer, pair, side, and exact
rational price merge. Trades without a user offer ID, including non-orderbook
activity, remain separate segments. The native client deliberately does not add a
second chain-data cache implementation in this slice; the asset catalog is a
small public-metadata cache owned by the shared Asset Discovery capability.

## Contract invocation

`contract invoke` follows Stellar CLI's fully-typed contract model: the deployed on-chain contract specification is the source of truth for functions, parameter names, types, and documentation. Fresnica options stay before `--`; the function and contract-specific named arguments follow it.

```sh
fresnica --network testnet contract invoke C... -- --help
fresnica --network testnet contract invoke C... -- transfer --help
fresnica --network testnet contract invoke C... --json -- balance --id G...
fresnica --network testnet contract invoke C... --wallet main -- transfer --from G... --to C... --amount 10000000
```

The shared `fresnica-client` resolves Stellar Asset Contract, Wasm, and external-reference specs through Stellar RPC. ABI value parsing and normalized JSON conversion are delegated to the official `soroban-spec-tools` implementation, so Terminal does not maintain a parallel Soroban type parser. Terminal owns only command grammar, human review/confirmation, and its machine JSON schema; it does not parse `ScSpecEntry` or construct `ScVal`. Scalar and complex Contract Spec values, including vectors, maps, tuples, options/results, UDTs, bytesN, and wide integers, use the official Stellar textual/JSON conversion rules. Dynamic help exposes official type examples where available.

Human help sanitizes control characters from untrusted on-chain documentation before terminal rendering. `--json` help remains machine-readable without requiring `-y`. Actual invocation first follows Stellar CLI's current default-send rule: if simulation contains no ledger write, published contract event, or authorization entry, Fresnica returns the Contract-Spec-decoded result without requiring a wallet, passphrase, fee, or submission. A `--json` invocation therefore needs no `-y` when it resolves read-only; if simulation classifies it as a write, `-y` is still required before signing so stdout remains one machine-readable document.


## External CLI plugins

Fresnica uses the executable-dispatch idea proven by Stellar CLI, but the product namespace is intentionally Fresnica-only. Unknown commands resolve by longest command chain to `fresnica-<command-chain>` executables on PATH. `stellar-*` and legacy `soroban-*` executables are not auto-dispatched: they use a different developer-tool identity/config model and would create a misleading wallet-context expectation under the `fresnica` command.

For example, `fresnica aqua contract ...` first looks for `fresnica-aqua-contract`, then falls back to the shorter `fresnica-aqua` command if present. Remaining arguments are forwarded unchanged, stdio is inherited, and Fresnica exits with the plugin process status. Built-in commands win and cannot be shadowed. `fresnica-tui` and the reserved `fresnica-system-auth-provider` companion are not plugins.

Installed Fresnica plugins can be inspected with:

```sh
fresnica plugin ls
```

Plugins are command extensions, not signer providers. Fresnica does not hand them decrypted wallet state, private keys, mnemonic material, Fresnica passphrases, raw unlock material, or an opened Ledger/HSM signer. The bundled `fresnica-anchor` consumer proves bounded semantic host re-entry: public wallet/network context, issued-asset receive preflight, Anchor-specific SEP-10 authentication, and an interactive-only withdrawal payment proposal. Fresnica owns receive validation, authorization, software/Ledger signer selection, transaction review and signature verification. The plugin receives only bounded semantic results such as the short-lived Anchor token and never gets generic signing authority. See [`docs/anchor-plugin-spike.md`](../../docs/anchor-plugin-spike.md).

Plugins are ordinary local executables and are **not sandboxed** by Fresnica. They run with the operating-system permissions of the current user and inherit the process environment, so users must install only plugins they trust. The guarantee is narrower: Fresnica itself does not inject secret wallet/signing material into the child process.

Fresnica global options parsed before the plugin name remain host policy. Selected public network/provider context and explicitly supported policy such as Classic transaction lifetime may be supplied to `fresnica-*`; plugin-specific arguments remain after the plugin command.

## Diagnostics

`-v` / `--verbose` prints safe execution stages and reports the last stage reached on failure.
`-vv` additionally prints the CLI version, selected network, and exact pinned Fresnica source revision.
Diagnostics intentionally never dump the raw argument vector or hidden input, so command arguments cannot accidentally expose a mnemonic, Stellar secret, Fresnica passphrase, SEP-10 token, or unlock material through verbose logging.

Put verbosity flags before the command:

```sh
fresnica -v balance
fresnica -vv --network testnet anchor discover USDC:G...
```

Default failures stay concise and point to `-v` for additional context.

## Anchor protocol boundary

Anchor is no longer a CLI built-in. The bundled `fresnica-anchor` native plugin covers capability discovery, SEP-10 authentication, SEP-24/SEP-6 transfer flows, transfer status, interactive withdrawal payment handoff, and SEP-12 customer status/update. Reusable protocol/HTTP/application semantics remain in `fresnica-client`; the plugin owns Anchor command grammar/rendering while the Fresnica host owns wallet context, receive preflight, hidden input, review, signer selection and submission.

## Build

```sh
cargo build --release -p fresnica-cli --bin fresnica
cargo build --release -p fresnica-anchor-plugin --bin fresnica-anchor
```

The executable is then:

```text
target/release/fresnica
```

For example:

```sh
target/release/fresnica wallet list
target/release/fresnica --network testnet wallet testnet-fund
target/release/fresnica --horizon-url https://stellar.example/horizon balance
target/release/fresnica account
target/release/fresnica balance
target/release/fresnica history --limit 20
target/release/fresnica asset discover --limit 20
target/release/fresnica contact add Alice G... --memo 12345
target/release/fresnica send 1 XLM to Alice
target/release/fresnica trust add USDC:G...
target/release/fresnica dex orderbook XLM USDC:G...
target/release/fresnica dex buy XRP:G... XLM 100 0.325 --allow-trustline
target/release/fresnica dex trades XRP:G... XLM --limit 20
target/release/fresnica dex fills
target/release/fresnica dex candles XRP:G... XLM --resolution 1h
target/release/fresnica --network testnet contract invoke C... -- --help
```

A wallet record is bound to its configured Stellar network. Wallet-backed network commands
fail before contacting the required provider if the invocation network does not match the
wallet record; use `--network testnet` for a testnet wallet.

## Deliberate non-goals of this slice

General chain-data caching and a product recommendation/ranking engine remain outside the CLI command surface.

Read-only contract calls are simulation-only and expose the decoded return value. File-backed contract-input convenience flags remain outside this slice; values continue to use the official Contract Spec parser rather than a second Fresnica ABI syntax.

The native client does not expose a raw `sign-xdr` shortcut. Routine transaction
signing stays behind client-side construction and review rather than creating a
path that bypasses product review.

OS authentication remains a client responsibility. A future native platform
adapter may release a standard `WalletUnlockKey` through the SDK/Core boundary;
no Keychain, biometric, PAM, or Windows Hello logic belongs in `fresnica-core`.
