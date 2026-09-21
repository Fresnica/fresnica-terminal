# Creating Fresnica Plugins — Agent Fast Path

This is the shortest reliable workflow for an AI agent asked to create a Fresnica plugin. Read [`../AGENTS.md`](../AGENTS.md) for the full project operating model and [`creating-plugins.md`](creating-plugins.md) for the human-oriented guide.

## 1. Prove a plugin is necessary

Run:

```sh
fresnica capabilities --json
```

If one public machine operation already fulfills the request, use it directly. Do not create a wrapper plugin with no business/orchestration responsibility.

Create a plugin when the task combines existing Fresnica capabilities with new protocol/business logic or a new external service.

## 2. Start from the canonical executable model

Your deliverable must be discoverable as:

```text
fresnica-<name>
```

Copy the shape of [`../examples/plugins/fresnica-xlm-balance`](../examples/plugins/fresnica-xlm-balance/README.md) if useful. Language is unrestricted; do not introduce an SDK only to launch a subprocess.

## 3. Use host context, do not reconstruct Fresnica

Expect:

```text
FRESNICA_PLUGIN_API=1
FRESNICA_PLUGIN_HOST
FRESNICA_PLUGIN_NETWORK
FRESNICA_HOME
```

and optional provider/policy overrides. Use `FRESNICA_PLUGIN_HOST` for wallet operations. Do not read wallet files directly.

## 4. Reuse machine operations

Call:

```sh
"$FRESNICA_PLUGIN_HOST" --network "$FRESNICA_PLUGIN_NETWORK" ... --json
```

Parse JSON schemas, not human text. Preserve non-zero host failures.

Do not create a duplicate `__plugin-host` API for an operation already visible through `fresnica capabilities --json`.

## 5. For Soroban, inspect before composing

```sh
"$FRESNICA_PLUGIN_HOST" --network "$FRESNICA_PLUGIN_NETWORK" contract C... --json
```

Require the expected versioned ABI schema, then inspect every input's `composition`:

```text
typed_json + guided=true
    -> construct --args-json from recursive ABI

dynamic_scval_json
    -> explicit tagged ScVal JSON; preserve exact scval_xdr

scval_xdr_success_only
    -> expert exact-XDR success path only via `--scval-xdr NAME BASE64`

unsupported or unknown
    -> STOP; do not invent an encoder
```

Simulate first:

```sh
"$FRESNICA_PLUGIN_HOST" --network "$FRESNICA_PLUGIN_NETWORK" \
  contract C... --simulate --json --args-json '{...}' FUNCTION
```

## 6. Authority boundary

STOP rather than implementing any of the following:

- private-key/mnemonic/passphrase access;
- direct wallet-storage reads;
- generic `sign-xdr`;
- plugin-controlled signer selection;
- review/confirmation bypass;
- a second Soroban ABI codec.

If the public CLI cannot express a required authority transition, identify the missing bounded semantic capability and change Fresnica at the owning layer instead of bypassing it.

## 7. Acceptance

At minimum run:

```sh
fresnica plugin ls --json
fresnica <name> <valid-test-input>
fresnica <name> <invalid-test-input>   # must fail non-zero
```

For network behavior use Testnet. For Soroban, include a simulate-only proof. Confirm no secret material appears in argv, environment values created by the plugin, stdout/stderr, logs, or fixtures.

## Definition of done

An agent may report completion only when:

```text
[ ] plugin discovery works
[ ] dispatch works
[ ] public machine capabilities are reused
[ ] host network/provider context is preserved
[ ] Soroban calls use inspected ABI/composition metadata
[ ] failure exit codes are preserved
[ ] security invariants remain inside Fresnica
[ ] relevant Testnet/local validation actually ran
```
