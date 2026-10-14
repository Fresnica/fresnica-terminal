# Minimal Fresnica plugin: XLM balance

This POSIX shell plugin reads an XLM balance by calling Fresnica's public machine interface:

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

It does not read Fresnica storage or handle signing material.

On Windows, expose the same behavior as `fresnica-xlm-balance.exe`.
