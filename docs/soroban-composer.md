# Soroban ABI Composer

Status: **v0.6 stable machine contract candidate (`fresnica-soroban-abi-v1`)**

Fresnica v0.6 turns a deployed Soroban Contract Spec into a lossless, machine-readable interface and a safe composition path. The goal is not to replace Stellar's ABI tooling. Fresnica uses the official Soroban spec tools for ScVal encoding and normalization, adding only narrow fail-closed guards around known malformed/ambiguous boundaries.

## Inspect a deployed contract

```sh
fresnica --network testnet contract C... --json
```

The response keeps the older flat `functions` representation for compatibility and adds:

```json
{
  "abi": {
    "schema": "fresnica-soroban-abi-v1",
    "functions": [],
    "types": []
  }
}
```

`abi.functions[].inputs[].type` is recursive and can describe primitives, Option, Result, Vec, Map, Tuple, BytesN, and UDT references. `abi.types` contains struct, union, enum, and error-enum definitions.

## Composition support

Every ABI input advertises the safest complete-domain composition contract:

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

The stable v1 modes are:

| mode | guided | meaning |
| --- | --- | --- |
| `typed_json` | `true` | Complete safe input domain is representable as ABI-guided JSON. |
| `dynamic_scval_json` | `false` | Type contains open-ended Soroban `Val`; use explicit tagged ScVal JSON. |
| `scval_xdr_success_only` | `false` | Conservative Result boundary; exact successful ScVal XDR can be validated. |
| `unsupported` | `false` | Fresnica has no safe complete-domain composition path. |

Consumers must treat unknown future modes as unguided/unsupported. Existing v1 mode labels and meanings are frozen; incompatible semantic changes require a new schema version.

## Compose typed arguments

For `typed_json` inputs, pass one top-level object before the function name:

```sh
fresnica --network testnet contract C... \
  --simulate --json \
  --args-json '{"from":"G...","amount":"10000000"}' \
  transfer
```

Terminal only splits the top-level object by argument name. Type validation, ScVal construction, and normalized review are owned by `fresnica-client` and the official Contract Spec tooling.

Structured values remain structured. A contract input such as `Vec<Request>` or `Option<Map<Address, Vec<Route>>>` should be built from the ABI model rather than flattened into ad-hoc strings.

## Simulate before authority

Use:

```sh
fresnica --network testnet contract C... --simulate --json --args-json '{...}' FUNCTION
```

Simulate-only mode never signs and never submits. Its machine response includes the decoded result and execution effects such as read/write footprint, event/auth counts, restore requirement, and the derived `requires_send` classification.

When a real write is intended, invoke through the normal Fresnica path so review, signer selection, authorization, and submission remain wallet-owned.

## Exact ScVal identity

Normalized JSON is semantic review data, but dynamic `Val` values can lose the original ScVal discriminant: for example, different integer ScVal variants can normalize to the same JSON number. Fresnica therefore exposes exact identity alongside normalized values:

```json
{
  "value": 7,
  "scval_xdr": "..."
}
```

Read/simulation results similarly expose `result_xdr` where applicable. Agents should preserve exact XDR when identity matters rather than reconstructing it from normalized JSON.

## Dynamic `Val`

A `dynamic_scval_json` input is intentionally not marked guided. Use explicit tagged ScVal JSON, for example:

```json
{"u32": 7}
```

Do not build a finite form generator that pretends an open-ended `Val` domain is fully described by the surrounding ABI.

## Result and unsupported boundaries

Typed JSON does not invent a Fresnica-specific representation for Contract Spec `Result` or `Error` inputs. The conservative Result success path is exposed as `scval_xdr_success_only`; callers with an already encoded successful ScVal may use the expert host option `--scval-xdr NAME BASE64`. Direct error values and unsupported shapes fail closed.

If `composition.mode` is `unsupported`, stop. Do not bypass the contract with a private encoder or raw wallet signing path.

## Saved contract names

Contracts can be saved and then inspected/invoked by local name:

```sh
fresnica --network testnet contract add pool C... --json
fresnica --network testnet contract pool --json
```

Fresnica records executable/Wasm observations. A later executable change fails closed until an explicit re-inspection refreshes the observation.

## Agent workflow

For an agent, the intended loop is:

```text
1. `fresnica capabilities --json`
2. inspect `contract C... --json`
3. locate the function and recursive types
4. check every input's `composition`
5. build `--args-json` only for supported modes
6. simulate with `--simulate --json`
7. inspect normalized values, exact XDR identity, and effects
8. use normal Fresnica invocation for a real authorized write
```

No source-code inspection should be necessary for an ordinary supported contract call.
