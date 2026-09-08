# Anchor native-plugin spike

Status: **Experimental consumer evidence; not a frozen plugin SDK**
Last verified: 2026-09-08

## Purpose

Anchor is the first real `fresnica-*` consumer used to discover what a wallet-aware
plugin actually needs. The spike deliberately starts from existing Anchor behavior
instead of designing a generic host ABI first.

The target command remains:

```text
fresnica anchor ...
        |
        v
fresnica-anchor
```

The plugin owns Anchor-specific command grammar and SEP-24 orchestration. Fresnica
remains the wallet, authorization, signer-selection and secret-handling authority.

## Exact source checkpoint

- Terminal spike base: `feat/terminal-fresnica-plugin-namespace@26f5e63af4fd7b9d297b8a654ad0ad6c47f487ce`.
- Terminal product spike: `feat/terminal-anchor-plugin-spike@7d47c2ff006af85d437a8311d03e13d6d12081e7`.
- Pinned Fresnica source: `51ac66909484bc1c78d544c8d68f8546197aacb0`.
- Upstream experimental branch: `feat/rust-client-anchor-explicit-domain`.

The upstream branch adds only two capabilities required by this consumer:

1. explicit-home-domain Anchor discovery while preserving the old issuer-derived API;
2. bounded Ed25519 provider signing with caller-supplied satisfied conditions,
   excluded keys and minimum-signature proof.

## Stellar CLI baseline

Current Stellar CLI executable plugins are intentionally thin: unknown commands
resolve to PATH executables, remaining argv is forwarded, stdio/environment are
normal child-process state, and the child exit code is propagated. Stellar CLI does
not inject a wallet context or provide a signing host API. More capable ecosystem
commands may invoke `stellar` themselves.

Fresnica keeps that behavior for compatible `stellar-*` / `soroban-*` plugins. They
do not receive Fresnica-native host context.

## First native-plugin host contract

The Anchor experiment proves that a Fresnica-native plugin needs **bounded host
re-entry**, not ambient wallet authority.

Only a `fresnica-*` child currently receives these experimental coordination values:

```text
FRESNICA_PLUGIN_API=1
FRESNICA_PLUGIN_HOST=<absolute Fresnica executable>
FRESNICA_PLUGIN_NETWORK=<selected network>
FRESNICA_HOME=<selected Fresnica home>
FRESNICA_HORIZON_URL=<override, when explicitly set>
FRESNICA_RPC_URL=<override, when explicitly set>
```

`FRESNICA_PLUGIN_API=1` is a protocol/version marker, not a security credential.
The executable and home values let the child re-enter the same Fresnica instance;
they do not grant a signer. Plugins already run with the invoking OS user's normal
permissions, so the home path itself is not treated as secret capability material.
A later persistent host channel may replace these process-coordination details.

The experimental hidden host currently exposes two semantic operations:

```text
__plugin-host context [--wallet NAME]
__plugin-host anchor-auth ASSET --home-domain DOMAIN [--wallet NAME]
```

`context` returns only selected public identity/context. `anchor-auth` performs the
SEP-10 challenge path inside Fresnica and returns the resulting short-lived Anchor
auth token to the requesting native plugin.

## Signing boundary

`anchor-auth` does not expose `sign-xdr`, a signer handle, a private key, a mnemonic,
an unlock key or a Fresnica passphrase.

The host performs:

```text
resolve wallet / ledger authorization
        |
fetch + validate SEP-10 challenge
        |
remove Anchor server signature from client-satisfaction proof
        |
exclude Anchor server signing key
        |
require at least one client Ed25519 signature
        |
Fresnica Signing Coordination
  -> protected software signer and/or external provider (Ledger)
  -> SDK signature verification
        |
exchange signed challenge
        |
short-lived SEP-10 bearer token
```

The token is the minimum capability the Anchor plugin needs to call the authenticated
SEP-24 endpoint. Host output is streamed without constructing an ordinary JSON token
copy; the plugin captures both the host response buffer and token in zeroizing memory.

## Official Testnet evidence

The Stellar reference flow uses `testanchor.stellar.org` with SRT:

```text
SRT:GCDNJUBQSX7AJWLJACMJ7I4BC3Z47BQUTMHEICZLE6MU4KQBRYG5JY6B
```

This exposed a real limitation in the old built-in command: the SRT issuer account
currently has no usable `home_domain`, while the reference wallet flow supplies the
Anchor home domain explicitly. The upstream `discover_anchor_at(asset, home_domain)`
API was added for this reason; the old automatic issuer-derived API remains intact.

A live command through the actual dispatcher succeeded:

```text
fresnica --network testnet anchor discover SRT:<issuer> \
  --home-domain testanchor.stellar.org --json
```

It resolved `fresnica-anchor` from PATH and reported SEP-10 plus SEP-24 deposit and
withdraw support from the live reference Anchor.

A live SEP-10 attempt proved the full process chain:

```text
fresnica
  -> fresnica-anchor
       -> fresnica __plugin-host anchor-auth
```

The inner host reached the expected interactive Fresnica signing prompt. The run was
terminated there without entering or automating wallet secret material. Therefore the
live test proves dispatch, host re-entry, live Anchor challenge acquisition/validation
and arrival at Fresnica authorization; it does **not** claim a completed token exchange
or SEP-24 interactive transaction.

No physical Ledger was attached to this VPS test. Provider-level tests prove that the
same bounded signer selection supports external Ed25519 providers, enforces excluded
keys/minimum signatures, and verifies returned signatures, but physical Ledger SEP-10
remains an explicit acceptance test.

## Isolation evidence

A subprocess smoke verified:

```text
fresnica-* child: FRESNICA_PLUGIN_API/HOST/NETWORK/HOME present
stellar-* child:  none of those Fresnica-native host values injected
```

Direct invocation of `__plugin-host` without the native-plugin protocol marker fails.
This marker is still not authorization; each semantic host operation must validate its
own inputs and preserve normal Fresnica user-consent/signing rules.

## Transitional routing

The durable global rule remains: Fresnica built-ins win over plugin fallback.

Because `anchor` still exists as a built-in behavior oracle while it is being extracted,
this spike alone probes a matching `fresnica-anchor` before the old built-in. That is a
migration exception, not a general shadowing rule. If Anchor reaches plugin parity, the
correct final state is to remove the built-in Anchor command and return to normal unknown-
command dispatch rather than retain a permanent shadowable built-in.

## Current scope and non-claims

The external `fresnica-anchor` spike currently covers:

- explicit-domain discovery;
- SEP-10 authentication through bounded host re-entry;
- SEP-24 deposit initiation;
- SEP-24 withdrawal initiation.

It does not yet replace built-in:

- transfer status handling;
- SEP-12 customer flow;
- SEP-6 path/fallback;
- withdrawal payment completion;
- the old built-in fallback itself.

Do not generalize the experimental environment/schema into a plugin SDK until this
consumer reaches end-to-end acceptance. Do not add generic signing, unrestricted host
RPC, registry/search/install, or signer-provider access merely to make the framework
look complete.
