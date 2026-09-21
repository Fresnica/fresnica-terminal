# Creating Fresnica Plugins

Fresnica plugins are ordinary external executables. There is no required language, framework, or Plugin SDK.

Use a plugin when you need to compose Fresnica capabilities with new business/protocol logic. If one existing `fresnica ... --json` operation already solves the task, call it directly instead of creating a plugin.

## Five-minute quick start

Create an executable named `fresnica-hello` somewhere on `PATH`:

```sh
#!/bin/sh
printf '%s\n' "hello from Fresnica"
```

Then:

```sh
chmod +x fresnica-hello
fresnica plugin ls
fresnica hello
```

Fresnica resolves unknown command chains to `fresnica-<command-chain>` executables. Built-in commands always win.

For multiword commands it tries the longest executable first. For example:

```text
fresnica aqua contract quote ...
  -> fresnica-aqua-contract ...
  -> otherwise fresnica-aqua contract quote ...
```

Remaining argv is forwarded unchanged; stdin/stdout/stderr are inherited; Fresnica returns the child exit status.

On Windows the discoverable executable uses the normal `.exe` form. `fresnica-tui` is reserved and is not treated as a plugin.

## Runtime context

A plugin launched by Fresnica receives public process context:

```text
FRESNICA_PLUGIN_API=1
FRESNICA_PLUGIN_HOST=<absolute Fresnica executable>
FRESNICA_PLUGIN_NETWORK=<selected network>
FRESNICA_HOME=<selected Fresnica home>
FRESNICA_HORIZON_URL=<explicit override, when present>
FRESNICA_RPC_URL=<explicit override, when present>
FRESNICA_TX_TIMEOUT_SECONDS=<explicit override, when present>
```

These are coordination values, not credentials. `FRESNICA_PLUGIN_API=1` is only a protocol marker.

Global Fresnica options are resolved by the host before plugin dispatch. A plugin should preserve the host-provided network/provider context rather than silently constructing a second configuration model.

## The default Plugin API is the public machine CLI

Before asking Fresnica for any private plugin host capability, inspect the public operation inventory:

```sh
fresnica capabilities --json
```

A plugin should normally call the host executable it was given:

```sh
"$FRESNICA_PLUGIN_HOST" \
  --network "$FRESNICA_PLUGIN_NETWORK" \
  token XLM balance G... \
  --json
```

This gives plugins, scripts, bots, and agents one shared operation surface and avoids semantic drift between a public CLI and a hidden plugin API.

The canonical minimal example is [`examples/plugins/fresnica-xlm-balance`](../examples/plugins/fresnica-xlm-balance/README.md).

## Soroban plugins

Do not ship a hand-written ABI table when the deployed Contract Spec can describe the operation.

Inspect the contract:

```sh
"$FRESNICA_PLUGIN_HOST" \
  --network "$FRESNICA_PLUGIN_NETWORK" \
  contract C... --json
```

Read `abi.schema == "fresnica-soroban-abi-v1"`, then use each input's `composition` contract. For `typed_json`/`guided=true`, build the argument object and simulate:

```sh
"$FRESNICA_PLUGIN_HOST" \
  --network "$FRESNICA_PLUGIN_NETWORK" \
  contract C... --simulate --json \
  --args-json '{"name":"alice","target":null}' \
  FUNCTION
```

See [`soroban-composer.md`](soroban-composer.md) for the composition modes and exact-value rules.

## Writes and authority

Plugins orchestrate intent; Fresnica owns wallet authority.

A plugin must not receive or control:

- private keys or mnemonics;
- Fresnica passphrases or raw unlock material;
- decrypted wallet state;
- opened Ledger/HSM/signer-provider handles;
- unrestricted signing capability;
- generic `sign-xdr` authority.

For a write, prefer an existing public Fresnica operation. If a protocol needs a special handoff that the public machine CLI cannot safely express, the correct pattern is a narrowly bounded host capability that reconstructs and reviews the semantic operation inside Fresnica.

The bundled `fresnica-anchor` is the advanced reference implementation. Its SEP-10 and payment handoffs demonstrate bounded host re-entry without giving the plugin generic signer access. See [`anchor-plugin-spike.md`](anchor-plugin-spike.md).

## `__plugin-host` is exceptional

`__plugin-host` is not a general plugin transport. Current capabilities exist for specific authority/state boundaries such as public wallet context, Anchor authentication/receive checks, and host-owned semantic proposals.

Do not add a new `__plugin-host` operation when the public machine CLI can already perform the same job. If a new bounded capability is truly required, its semantics belong to Fresnica and must remain narrower than generic signing or transaction submission authority.

## Output and errors

Prefer machine-readable JSON for calls back into Fresnica. Your own plugin may expose human and JSON modes, but do not parse Fresnica's human rendering when a JSON schema exists.

Return a non-zero exit status for invalid input or failed operations. Do not swallow the host's failure status.

## Installation and discovery

Fresnica does not currently provide a plugin registry or installer. Installation means making a trusted executable available on `PATH` using the `fresnica-<name>` convention.

Verify installation with:

```sh
fresnica plugin ls --json
```

Plugins are ordinary local processes and are not OS-sandboxed. Users should install only plugins they trust.

## Completion checklist

Before calling a plugin complete:

```text
[ ] `fresnica plugin ls --json` discovers it
[ ] `fresnica <name> ...` dispatches to it
[ ] host-provided network/provider context is preserved
[ ] existing Fresnica operations are reused through `--json`
[ ] Soroban interfaces come from deployed Contract Spec when applicable
[ ] invalid input exits non-zero
[ ] no wallet secret crosses the plugin boundary
[ ] no generic signing or signer selection is implemented in the plugin
[ ] Testnet smoke test passes for network-dependent behavior
```

## Further reading

- [`plugin-architecture.md`](plugin-architecture.md): architecture rationale and trust model.
- [`anchor-plugin-spike.md`](anchor-plugin-spike.md): complex real consumer.
- [`operation-foundation.md`](operation-foundation.md): reusable capability ownership.
- [`../AGENTS.md`](../AGENTS.md): complete AI-agent operating guide.
- [`creating-plugins-for-agents.md`](creating-plugins-for-agents.md): compressed agent workflow.
