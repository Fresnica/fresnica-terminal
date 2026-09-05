# Fresnica Native Rust CLI

This client consumes the platform-neutral `fresnica-sdk` semantic boundary and
links the Rust libraries into one `fresnica` executable. It remains a direct
native reference client without a process/FFI transport.

## Current scope

The native client currently covers local wallet lifecycle and contacts, testnet
Friendbot funding, read-only Horizon queries, reviewed payments, issued-asset
trustline lifecycle, Classic SDEX read/write/history operations, and Stellar anchor flows:

- `info [--wallet NAME]`
- `account [--wallet NAME] [--json]`
- `balance [--wallet NAME] [--json]` (`assets` is an alias)
- `history [--wallet NAME] [--limit N] [--json]`
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
- `anchor discover CODE:GISSUER [--json]`
- `anchor auth CODE:GISSUER [--wallet NAME]`
- `anchor deposit CODE:GISSUER [--wallet NAME] [--field NAME=VALUE]... [--json]`
- `anchor withdraw CODE:GISSUER [--wallet NAME] [--field NAME=VALUE]... [--json]`
- `anchor status CODE:GISSUER ID [--wallet NAME] [--protocol sep24|sep6] [--pay] [-y] [--json]`
- `anchor customer CODE:GISSUER [--wallet NAME] [--id CUSTOMER_ID] [--transaction ID] [--type TYPE] [--lang LANG] [--input PATH|-] [--json]`
- `wallet list`
- `wallet use NAME`
- `wallet create NAME`
- `wallet import-secret NAME`
- `wallet import-mnemonic NAME`
- `wallet import-watch NAME G...`
- `wallet attach-secret NAME`
- `wallet attach-mnemonic NAME [--index N] [--language LANGUAGE]`
- `wallet detach-signer NAME`
- `wallet testnet-fund [--wallet NAME]` (`wallet fund` is an alias)
- `wallet reveal [NAME]`
- `wallet backup NAME PATH`
- `wallet restore PATH [--name NAME]`
- `wallet delete NAME`

It reads and writes the same wallet record files, `.default` pointer,
`contacts.json`, and `fresnica-wallet-backup` version-1 format as the Python
reference client. The default application home is `FRESNICA_HOME` when set,
otherwise `~/.fresnica`.

Create/import/reveal cryptography and account identity parsing go through
`fresnica-sdk`; Core remains the cryptographic authority underneath the SDK.
Secret, mnemonic, BIP39-passphrase, and Fresnica-passphrase prompts are read from
the controlling terminal with input hidden; they are not accepted as command-line
arguments.

A watch-only Classic account can later attach a secret or mnemonic signer without
changing wallet identity. The CLI passes the existing G address as the SDK
`expected_signer_public_key`; mismatched material is rejected before the wallet
record changes. `wallet detach-signer` removes only local protected signing
material after passphrase verification and keeps the same account as watch-only.

Contacts are client-local public metadata. Contact names are resolved before
payment construction, an explicit `--memo` takes precedence over a contact's
default memo, and transaction review always shows the resolved G address even
when the user entered an alias.

Friendbot is a testnet-only client utility. It funds the selected testnet address
directly through `friendbot.stellar.org` with a 15-second request timeout and does
not require signing material, so watch-only testnet wallets are valid targets.

Account state, balances, recent operations, SDEX reads, transaction preparation
and Horizon submission are client responsibilities. Reusable Rust application
semantics live in `fresnica-client`, which talks to the matching public or testnet
Horizon server; none of that HTTP or product policy is moved into `fresnica-core`.

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
second cache implementation in this slice.

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

The implemented CLI anchor surface covers capability discovery, SEP-10 authentication, SEP-24/SEP-6 transfer flows, transfer status/payment handoff, and SEP-12 customer status/update. Reusable protocol/HTTP/application semantics remain in `fresnica-client`; CLI owns terminal argument parsing, hidden input, rendering, confirmation, and local file selection.

## Build

```sh
cargo build --release -p fresnica-cli --bin fresnica
```

The executable is then:

```text
target/release/fresnica
```

For example:

```sh
target/release/fresnica wallet list
target/release/fresnica --network testnet wallet testnet-fund
target/release/fresnica account
target/release/fresnica balance
target/release/fresnica history --limit 20
target/release/fresnica contact add Alice G... --memo 12345
target/release/fresnica send 1 XLM to Alice
target/release/fresnica trust add USDC:G...
target/release/fresnica dex orderbook XLM USDC:G...
target/release/fresnica dex buy XRP:G... XLM 100 0.325 --allow-trustline
target/release/fresnica dex trades XRP:G... XLM --limit 20
target/release/fresnica dex fills
target/release/fresnica dex candles XRP:G... XLM --resolution 1h
```

A wallet record is bound to its configured Stellar network. Network commands
fail before contacting Horizon if the invocation network does not match the
wallet record; use `--network testnet` for a testnet wallet.

## Deliberate non-goals of this slice

Local chain-data caching and TUI presentation remain outside the CLI command surface.

The native client does not expose a raw `sign-xdr` shortcut. Routine transaction
signing stays behind client-side construction and review rather than creating a
path that bypasses product review.

OS authentication remains a client responsibility. A future native platform
adapter may release a standard `WalletUnlockKey` through the SDK/Core boundary;
no Keychain, biometric, PAM, or Windows Hello logic belongs in `fresnica-core`.
