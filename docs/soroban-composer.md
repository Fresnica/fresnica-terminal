# Soroban ABI Composer

Fresnica v0.6 exposes deployed Soroban Contract Spec as a versioned machine interface and uses that same spec to validate contract arguments. ScVal encoding and normalization stay with the official Soroban spec tooling; Fresnica only adds narrow validation where the upstream parser is permissive or incomplete.

## Inspect a contract

```sh
fresnica --network testnet contract C... --json
```

The response keeps the legacy flat `functions` field and adds:

```json
{
  "abi": {
    "schema": "fresnica-soroban-abi-v1",
    "functions": [],
    "types": []
  }
}
```

`abi.functions[].inputs[].type` is recursive. It covers primitives, Option, Result, Vec, Map, Tuple, BytesN, and UDT references. `abi.types` contains struct, union, enum, and error-enum definitions.

## Input composition

Each input reports how it can be composed:

| mode | guided | use |
| --- | --- | --- |
| `typed_json` | `true` | Pass ABI-shaped values through `--args-json`. |
| `dynamic_scval_json` | `false` | Use explicit tagged ScVal JSON; preserve exact XDR identity. |
| `scval_xdr_success_only` | `false` | Supply an exact successful ScVal through `--scval-xdr`. |
| `unsupported` | `false` | No safe complete input path is available. |

Unknown future modes should be treated as unsupported. Existing v1 mode names and meanings are stable; incompatible changes require a new schema version.

Example:

```json
{
  "name": "target",
  "type": {
    "kind": "option",
    "value": { "kind": "primitive", "name": "address" }
  },
  "composition": {
    "mode": "typed_json",
    "guided": true
  }
}
```

## Typed JSON

For `typed_json`, pass one top-level object before the function name:

```sh
fresnica --network testnet contract C... \
  --simulate --json \
  --args-json '{"from":"G...","amount":"10000000"}' \
  transfer
```

Terminal splits the top-level object by argument name. Type validation and ScVal construction happen against the deployed Contract Spec.

Structured types stay structured; do not flatten `Vec<Request>`, maps, tuples, or UDTs into private string formats.

## Simulation

Use `--simulate --json` before a write:

```sh
fresnica --network testnet contract C... \
  --simulate --json \
  --args-json '{...}' \
  FUNCTION
```

Simulation does not sign or submit. The response includes the decoded result and effects such as read/write footprint, events, authorization entries, restore requirement, and `requires_send`.

For an actual write, use the normal Fresnica invocation so review, signer selection, authorization, and submission remain wallet-owned.

## Exact ScVal identity

Normalized JSON is intended for semantic review, not byte-exact identity. Dynamic values can normalize to the same JSON while using different ScVal variants.

Fresnica therefore keeps exact XDR beside normalized values:

```json
{
  "value": 7,
  "scval_xdr": "..."
}
```

Read and simulation results similarly expose `result_xdr` where applicable. Preserve these fields if a later step needs exact identity.

For `dynamic_scval_json`, use tagged ScVal JSON such as:

```json
{"u32": 7}
```

For `scval_xdr_success_only`, pass an already encoded successful ScVal with:

```sh
--scval-xdr NAME BASE64
```

Fresnica does not invent a private JSON representation for Contract Spec `Result`/`Error` inputs.

## Saved contracts

Contracts may be saved under a local name:

```sh
fresnica --network testnet contract add pool C... --json
fresnica --network testnet contract pool --json
```

Fresnica records executable/Wasm observations. If the executable changes later, invocation fails until an explicit inspection refreshes the observation.

## Automation

For scripts or agents:

1. read `fresnica capabilities --json`;
2. inspect `contract C... --json`;
3. select the function and inspect each input's `composition`;
4. compose only supported inputs;
5. simulate;
6. inspect values, XDR identity, and effects;
7. invoke normally when a write is intended.

Ordinary supported calls should not require source-code inspection or a private ABI implementation.
