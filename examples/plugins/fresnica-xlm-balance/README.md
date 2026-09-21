# Minimal Fresnica plugin: XLM balance

This POSIX-shell example demonstrates the default plugin architecture without an SDK or private host RPC.

It is intentionally small:

```text
fresnica xlm-balance G...
  -> PATH executable `fresnica-xlm-balance`
  -> call the provided `FRESNICA_PLUGIN_HOST`
  -> reuse public `token XLM balance ... --json`
  -> return the host's JSON and exit status
```

To try it from this repository:

```sh
mkdir -p /tmp/fresnica-plugin-example
ln -sf "$PWD/examples/plugins/fresnica-xlm-balance/fresnica-xlm-balance" \
  /tmp/fresnica-plugin-example/fresnica-xlm-balance

PATH="/tmp/fresnica-plugin-example:$PWD/target/debug:$PATH" \
  target/debug/fresnica plugin ls --json

PATH="/tmp/fresnica-plugin-example:$PWD/target/debug:$PATH" \
  target/debug/fresnica --network testnet xlm-balance \
  GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF
```

The example does not read Fresnica storage or handle signing material. It only composes an existing machine operation.

On Windows, build or wrap the same behavior as a discoverable `fresnica-xlm-balance.exe`; the process contract is language-neutral.
