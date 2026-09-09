# Fresnica Terminal Plugin Architecture

Status: **accepted for v0.3.0; Fresnica-native namespace only; host wire remains experimental**

Last verified: 2026-09-08.

## Durable decision

Fresnica learned the external-executable dispatch model from Stellar CLI, but the product contract is intentionally narrower:

```text
fresnica <command-chain>
        -> built-in command, when present
        -> otherwise PATH executable fresnica-<command-chain>
```

Only `fresnica-*` is a Fresnica plugin namespace. `stellar-*` and legacy `soroban-*` executables are **not** automatically dispatched. They belong to Stellar's developer-tool/config/identity model and would create a misleading expectation that a command launched under `fresnica` automatically uses the selected Fresnica wallet and authorization policy.

The earlier Stellar-compatible dispatcher remains useful as architecture evidence, not as the shipped product contract.

## Executable convention

```text
package / project convention:  fresnica_<name>
PATH executable:                fresnica-<name>
```

Executable naming is language-neutral. `fresnica-tui` is a reserved companion product binary, not a plugin, and is explicitly excluded from listing/dispatch.

## Command resolution

Built-ins win. Unknown commands use longest matching Fresnica command chain first. For example:

```text
fresnica aqua contract quote ...
  -> fresnica-aqua-contract ...
  -> if absent, fresnica-aqua contract quote ...
```

Remaining argv is forwarded unchanged, stdin/stdout/stderr are inherited, and Fresnica returns the child exit status.

`fresnica plugin ls` lists only executable `fresnica-*` plugins visible on PATH.

## Why Stellar CLI plugin auto-compatibility was removed

The compatibility prototype proved useful mechanics: PATH discovery, longest-chain matching, platform executable naming, argv/stdout/stderr inheritance, exit-code propagation, and release-package discovery. Those mechanics are retained.

Automatic `stellar-*` / `soroban-*` fallback was removed before v0.3.0 because it offered little wallet-product value and weakened the user mental model:

- Stellar CLI plugins generally assume Stellar CLI developer configuration and identities.
- Fresnica commands imply the selected Fresnica wallet, network, review, signer selection and submission policy.
- Silently launching a Stellar plugin beneath `fresnica` would look wallet-aware even when it is not.

If a future Stellar ecosystem tool is valuable to Fresnica, integrate it deliberately as a `fresnica-*` adapter or reuse its library/protocol directly instead of granting automatic namespace compatibility.

## Trust boundary

A plugin is a lower-trust external process for ecosystem orchestration, queries and semantic proposals. Fresnica must not inject or pass:

- private keys or mnemonic material;
- Fresnica passphrases or raw unlock material;
- decrypted wallet state;
- opened Ledger/HSM handles;
- signer-provider objects;
- unrestricted transaction-signing capability.

The host remains responsible for validation, authorization, user review, signer selection, signature verification and submission safety.

Plugins are ordinary local executables and are not OS-sandboxed by Fresnica. Users must install only plugins they trust. The wallet guarantee is narrower: Fresnica does not hand the child secret wallet/signing material or generic signing authority.

## Bounded native host re-entry

The first real native consumer, `fresnica-anchor`, proves four bounded interaction classes:

1. public selected wallet/network context;
2. read-only asset receive readiness;
3. Anchor-specific SEP-10 authentication returning only a short-lived bearer token;
4. payment proposal -> Fresnica reconstructs and interactively reviews/signs/submits.

Current concrete host operations are documented in [`docs/anchor-plugin-spike.md`](anchor-plugin-spike.md). They validate the direction, not a generic host RPC or stable Plugin SDK.

A plugin that needs an on-chain write proposes intent/material back to Fresnica. It does not choose the signer and cannot bypass confirmation. Anchor withdrawal payment handoff therefore returns to the normal Fresnica Payment capability.

Host-owned transaction policy may cross the plugin process boundary without becoming plugin authority. The validated example is Classic transaction lifetime: an explicit `--tx-timeout` is supplied to `fresnica-*` so a later Fresnica-owned payment proposal uses the same reviewed TimeBounds.

Signer Providers remain a separate, higher-trust extension model coordinated by Fresnica. Ledger/HSM/passkey-style signers are not plugins.

System Auth Providers are a third extension class: they authorize local use of an existing protected software signer and may release only its exact-envelope `WalletUnlockKey` to Fresnica after platform authentication. They do not sign arbitrary payloads and are not ordinary plugins or Signer Providers. The macOS implementation is a reserved first-party companion at a fixed sibling app-bundle path; `fresnica-system-auth-provider` is excluded from plugin discovery. Linux likewise uses a separately installed root-owned provider at the fixed `/usr/libexec/fresnica-system-auth-provider` path, with per-release polkit `auth_self` authorization for the exact caller process. Windows uses an administrator-installed provider at a fixed `%ProgramFiles%\Fresnica\SystemAuth` path plus a narrow LocalSystem broker; release is gated by a fresh Win32 WebAuthn/Windows Hello platform assertion that the broker verifies before returning an unlock key. None of these high-trust providers is selected merely from `PATH`.

## Anchor protocol placement

SEP-24 and SEP-6 are complementary product paths, not simply new versus old:

- SEP-24 is hosted/interactive.
- SEP-6 is programmatic and can stay inside the wallet.

Fresnica follows what the Anchor advertises and preserves `/info`-driven legacy SEP-6 compatibility without domain-name special cases. Both async transaction/status withdrawals and immediate `account_id + memo` withdrawals return through the same Fresnica-owned review/signing boundary.

Draft SEP-59 is complementary and inbound-only. It models reusable external receiving instruments as account resources; it does not replace SEP-6 withdrawals.

## Deliberate non-goals for v0.3.0

Do not add merely to make the plugin framework look complete:

- plugin registry/search/install/update;
- Stellar CLI plugin auto-compatibility;
- in-process extension ABI;
- unrestricted host RPC;
- generic `sign-xdr`;
- signer-provider access from plugins;
- a stable SDK wrapping the current one-consumer env/JSON wire.

A second real native consumer should determine which current wire details are genuinely general before a Native Plugin SDK is declared stable.

## Historical evidence

- `feat/terminal-stellar-plugin-dispatch@bd55984777de5e9058063dd6fb91580b69f471fa` proved Stellar-style executable dispatch mechanics.
- `feat/terminal-fresnica-plugin-namespace@26f5e63af4fd7b9d297b8a654ad0ad6c47f487ce` proved the native namespace.
- Anchor parity moved the real command surface to bundled `fresnica-anchor` and removed the old CLI Anchor built-in.
- v0.3.0 convergence keeps only the Fresnica-native product namespace while preserving the proven dispatch mechanics.

## Anti-drift rule

Do not reintroduce `stellar-*` / `soroban-*` fallback merely because the resolver can support it. Compatibility must have a concrete Fresnica wallet product use case and an explicit identity/authorization model first.
