# Plugin Work for Agents

Use this when a task actually requires a Fresnica plugin. For project-wide rules, read [`AGENTS.md`](../AGENTS.md).

## 1. Check whether code is needed

```sh
fresnica capabilities --json
```

If one existing operation already satisfies the request, use it directly. A plugin should add orchestration or protocol logic, not another name for an existing command.

## 2. Use the executable convention

Ship an executable named:

```text
fresnica-<name>
```

The minimal reference is [`examples/plugins/fresnica-xlm-balance`](../examples/plugins/fresnica-xlm-balance/README.md). Do not add an SDK merely to spawn Fresnica.

## 3. Reuse host context

Use the supplied `FRESNICA_PLUGIN_HOST`, `FRESNICA_PLUGIN_NETWORK`, and `FRESNICA_HOME`. Provider overrides may also be present.

Wallet operations should go back through the host:

```sh
"$FRESNICA_PLUGIN_HOST" --network "$FRESNICA_PLUGIN_NETWORK" ... --json
```

Do not read Fresnica wallet files directly.

## 4. Soroban

Inspect first:

```sh
"$FRESNICA_PLUGIN_HOST" --network "$FRESNICA_PLUGIN_NETWORK" contract C... --json
```

Follow each input's `composition` value:

- `typed_json`: build `--args-json`.
- `dynamic_scval_json`: use tagged ScVal JSON and keep `scval_xdr`.
- `scval_xdr_success_only`: use `--scval-xdr NAME BASE64` for an exact successful value.
- `unsupported` or unknown: stop; do not invent an encoder.

Simulate before a write:

```sh
"$FRESNICA_PLUGIN_HOST" --network "$FRESNICA_PLUGIN_NETWORK" \
  contract C... --simulate --json --args-json '{...}' FUNCTION
```

## 5. Keep authority in Fresnica

Do not implement private-key, mnemonic, passphrase, wallet-storage, generic `sign-xdr`, signer-selection, or confirmation-bypass paths in a plugin.

If the public CLI cannot express a required authority transition, the missing piece belongs in Fresnica as a bounded capability.

## 6. Verify

Run at least:

```sh
fresnica plugin ls --json
fresnica <name> <valid-input>
fresnica <name> <invalid-input>   # non-zero
```

Use Testnet for network behavior. For Soroban, include a simulate-only proof. Preserve host failures and avoid secrets in argv, environment values, logs, or fixtures.
