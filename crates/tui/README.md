# Fresnica Rust TUI

`fresnica-tui` is the native interactive terminal product for Fresnica.
It consumes the Git-pinned `fresnica-client` shared capability layer; it does not own separate wallet, crypto, Horizon, Payment, Trustline, SDEX, or Asset Discovery semantics.

Current reference slice:

- selected wallet identity and signer capability;
- network-scoped wallet switching for the current session;
- Horizon balances/liabilities;
- recent account activity;
- manual refresh;
- reviewed XLM/issued-asset payment preparation and submission;
- Trustline add/limit/remove flows with reviewed ChangeTrust transactions;
- SDEX BUY/SELL/update/cancel flows with reviewed offers;
- market browsing for order book, trades, and candles;
- cache-first exact asset selection for Send, Trustline, market pairs, and offer pairs;
- masked Fresnica passphrase entry and SDK-backed submission with shared pending-transaction protection.

Asset selection remains identity-safe. While an asset/base/counter field is focused, `/` opens the cached catalog immediately; `r` explicitly refreshes it; Up/Down or `j/k` navigates; Enter applies the full `XLM` or `CODE:GISSUER` identity; Esc closes the picker and preserves the manually typed value. Trustline selection excludes XLM because native assets do not have trustlines. Manual exact identity input remains available at all times.

The TUI owns interaction state, forms, rendering, confirmation, and picker presentation. Payment, Trustline, SDEX, Asset Discovery catalog/cache behavior, exact review data, transaction construction, signing coordination, submission, and pending retry protection remain shared `fresnica-client` behavior rather than copies of CLI command handlers.

Run after building with Rust:

```bash
cargo run -p fresnica-tui -- --network testnet
```

Use `--home PATH` or `FRESNICA_HOME` to point at an isolated wallet directory.
