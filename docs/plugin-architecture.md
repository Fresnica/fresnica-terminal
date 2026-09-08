# Fresnica Terminal Plugin Architecture

Status: **Accepted architecture; implementation staged**
Last reviewed: 2026-09-08

## Decision

Fresnica Terminal has two complementary external-command paths. They share the
same process-oriented executable convention, but they serve different purposes.

1. **Fresnica-native plugins** use the `fresnica-*` namespace. Fresnica owns
   these integrations and may later provide a bounded wallet execution context.
   Examples include ecosystem integrations such as `fresnica aqua ...` and
   future SAINT integration such as `fresnica saint ...`.
2. **Stellar ecosystem plugins** use the existing `stellar-*` namespace, with
   legacy `soroban-*` compatibility. Fresnica consumes this ecosystem rather
   than cloning Stellar CLI's plugin registry, search, or install product.

This dual namespace is an architectural decision. A milestone that implements
only one path must not be documented as removing the other path.

## Command resolution

Built-in Fresnica commands always win. For an unknown command, plugin resolution
uses the longest matching command chain first. For a given chain the intended
precedence is:

```text
fresnica-<command-chain>
stellar-<command-chain>
soroban-<command-chain>
```

For example, `fresnica aqua contract ...` should first resolve a matching
`fresnica-aqua[-contract]` executable. If no Fresnica-native executable exists,
a compatible `stellar-aqua[-contract]` or legacy `soroban-aqua[-contract]` may
satisfy the same unknown command chain.

The exact executable matching rules remain intentionally compatible with the
Stellar CLI external-command style: PATH discovery, argument forwarding,
inherited stdin/stdout/stderr, and child exit-status propagation.

## Trust and wallet-context boundary

Plugins and signer providers are separate extension mechanisms.

A plugin is a lower-trust external process for query, ecosystem, build, and
proposal capabilities. Fresnica must not inject or pass:

- private keys or mnemonic material;
- Fresnica passphrases or raw unlock material;
- decrypted wallet state;
- opened Ledger/HSM handles;
- an unrestricted signer capability.

A Fresnica-native plugin may eventually receive the minimum public/session
context required by a real integration, such as selected network or account
identity. That context is a versioned product contract, not ambient access to
the wallet. Its concrete fields must be driven by a real consumer rather than a
speculative generic host ABI.

A plugin that needs an on-chain write should return or otherwise propose the
transaction/invocation material to Fresnica. Fresnica remains responsible for
semantic review, authorization, signer selection, signature verification, and
submission safety through Core/SDK/Client-owned boundaries. Plugins do not get
a shortcut around that path.

Signer Providers are high-trust signing capabilities coordinated by Fresnica.
Ledger, HSM, secure-enclave, and future passkey-style signers belong to that
separate model and are not CLI plugins.

## Ecosystem intent

The near-term reason for `stellar-*` compatibility is pragmatic: Fresnica can
consume useful Stellar CLI ecosystem tools before a Fresnica plugin community
exists. The reason for `fresnica-*` is different: Fresnica itself can ship
wallet-aware ecosystem integrations without moving DeFi/protocol-specific logic
into Core or the Terminal binary.

Therefore Aqua-style DeFi logic belongs outside Core and may be delivered as a
Fresnica-native plugin while Fresnica remains the wallet/security authority.
SAINT follows the same consumer model once its standalone bounded query contract
is stable; SAINT itself must not depend on Fresnica plugin internals.

## Deliberate non-goals

The plugin architecture does not currently require a plugin registry/search
service, installer/updater, in-process ABI, generic `FRESNICA_BIN` callback,
unrestricted host RPC, or signer-provider bridge. These require separate,
consumer-driven decisions.

## Implementation status

Draft PR #32 (`feat/terminal-stellar-plugin-dispatch`) proves the
Stellar-compatibility half: `stellar-*`, legacy `soroban-*`, longest-chain
matching, PATH discovery/listing, platform executable rules, inherited stdio,
and exit-status propagation.

The follow-up `feat/terminal-fresnica-plugin-namespace` slice restores
`fresnica-*` on top of that dispatcher with the accepted precedence: longest
command chain first, then `fresnica-*`, `stellar-*`, `soroban-*` for the same
chain. It deliberately adds no wallet-context fields, host callback, signing
bridge, registry/search/install behavior, dependency, or Cargo manifest change.
Earlier Draft PR #31 remains historical implementation evidence rather than a
branch to revive wholesale.

## Documentation/source-of-truth rule

This file records the durable plugin architecture. `docs/CURRENT.md`, handoff
notes, PR descriptions, and milestone plans should record implementation state
and link here rather than redefining namespace or trust policy.

When implementation and this accepted architecture differ, documentation must
say **not implemented yet**, not silently rewrite the architecture to match the
current branch.
