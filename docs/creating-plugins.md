# Creating Fresnica Plugins

Fresnica plugins are executables on `PATH`. There is no required language or Plugin SDK.

Use a plugin when you need protocol or business logic around existing Fresnica operations. If one public `fresnica ... --json` command already does the job, call it directly instead.

## Quick start

Create `fresnica-hello`:

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

Unknown command chains are resolved to `fresnica-<command-chain>`. Built-ins take precedence. For multiword commands Fresnica tries the longest executable name first:

```text
fresnica aqua contract quote ...
  -> fresnica-aqua-contract ...
  -> otherwise fresnica-aqua contract quote ...
```

Remaining arguments are passed unchanged. stdin/stdout/stderr are inherited and Fresnica returns the child exit status. On Windows the executable uses the normal `.exe` form.

## Host context

A plugin launched by Fresnica receives:

```text
FRESNICA_PLUGIN_API=1
FRESNICA_PLUGIN_HOST=<absolute Fresnica executable>
FRESNICA_PLUGIN_NETWORK=<selected network>
FRESNICA_HOME=<selected Fresnica home>
FRESNICA_HORIZON_URL=<when explicitly set>
FRESNICA_RPC_URL=<when explicitly set>
FRESNICA_TX_TIMEOUT_SECONDS=<when explicitly set>
```

These values are process context, not credentials. Use them instead of inventing another Fresnica configuration model.

## Call Fresnica through the public CLI

The public machine CLI is the normal plugin API. Discover it with:

```sh
fresnica capabilities --json
```

Then call the host executable:

```sh
"$FRESNICA_PLUGIN_HOST" \
  --network "$FRESNICA_PLUGIN_NETWORK" \
  token XLM balance G... \
  --json
```

Use JSON output when available and preserve non-zero exit codes.

The minimal working example is [`examples/plugins/fresnica-xlm-balance`](../examples/plugins/fresnica-xlm-balance/README.md).

## Soroban

Inspect deployed Contract Spec rather than maintaining a private ABI table:

```sh
"$FRESNICA_PLUGIN_HOST" \
  --network "$FRESNICA_PLUGIN_NETWORK" \
  contract C... --json
```

For inputs reported as `typed_json` with `guided=true`, compose a JSON object and simulate:

```sh
"$FRESNICA_PLUGIN_HOST" \
  --network "$FRESNICA_PLUGIN_NETWORK" \
  contract C... --simulate --json \
  --args-json '{"name":"alice","target":null}' \
  FUNCTION
```

See [`soroban-composer.md`](soroban-composer.md) for the other composition modes.

## Writes and signing

Plugins do not own wallet authority. Keep private keys, mnemonics, passphrases, unlock material, signer handles, signer selection, and generic `sign-xdr` inside Fresnica.

Use an existing public write operation when possible. If a protocol needs an authority or state handoff that the public CLI cannot express safely, add a narrow host capability to Fresnica rather than giving the plugin generic signing access.

The bundled `fresnica-anchor` is the reference for this advanced case. See [`anchor-plugin-spike.md`](anchor-plugin-spike.md).

`__plugin-host` is deliberately not a general plugin API. Do not add a private host operation for something already available through the public machine CLI.

## Install and verify

Installation means putting a trusted `fresnica-<name>` executable on `PATH`. Fresnica does not currently provide a registry or installer.

Check discovery:

```sh
fresnica plugin ls --json
```

Before shipping a network-dependent plugin, verify:

- discovery through `plugin ls --json`;
- dispatch through `fresnica <name> ...`;
- invalid input returns non-zero;
- host network/provider context is preserved;
- existing Fresnica operations are reused through JSON interfaces;
- Testnet behavior works;
- no wallet secret or generic signing authority crosses the process boundary.

Plugins are ordinary local processes and are not OS-sandboxed. Install only code you trust.

## References

- [`plugin-architecture.md`](plugin-architecture.md) — architecture and trust boundary.
- [`anchor-plugin-spike.md`](anchor-plugin-spike.md) — complex real plugin.
- [`operation-foundation.md`](operation-foundation.md) — capability ownership.
- [`../AGENTS.md`](../AGENTS.md) — project guidance for agents.
- [`creating-plugins-for-agents.md`](creating-plugins-for-agents.md) — short agent workflow.
