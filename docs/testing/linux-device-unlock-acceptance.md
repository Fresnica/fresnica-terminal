# Linux Device Authentication acceptance

Test build only. Use Testnet and a disposable or low-value software wallet.

Product checkpoint: `1b8cc2f592935ffb98bca6da39660bafa82fe41c` on `feat/terminal-device-auth-cross-platform`.

## Scope

Fresnica remains one user-space binary. Linux separates two responsibilities:

- **DeviceAuthenticator:** a desktop Secret Service `Prompt` must complete for this transaction;
- **DeviceSecretStore:** the exact-envelope 32-byte unlock key lives in a dedicated `Fresnica Device Unlock` Secret Service collection.

No root, helper, daemon, service, polkit policy, or administrator setup is part of this design.

## Requirements

- x86_64 desktop Linux with an active graphical user session.
- A Secret Service implementation such as GNOME Keyring or KWallet.
- Testnet only; run Fresnica as the normal desktop user.

Headless/SSH-only systems may correctly report Device Unlock as unavailable.

## Prepare

```bash
chmod +x fresnica
./fresnica --version

BIN="$PWD/fresnica"
HOME_DIR="$HOME/.fresnica-device-unlock-test"
WALLET="du-a"
```

For this acceptance test, start with Device Unlock disabled. If this wallet was enrolled with an earlier test build, disable it first or use a fresh test wallet/home. The new enrollment creates/uses the dedicated Fresnica collection and removes any legacy matching item from the default collection.

## Enable

```bash
"$BIN" --home "$HOME_DIR" --network testnet \
  wallet device-unlock status "$WALLET"

"$BIN" --home "$HOME_DIR" --network testnet \
  wallet device-unlock enable "$WALLET"
```

Enable is a hard authenticator gate. Fresnica first ensures the dedicated collection exists. The collection is created **without a custom alias** (empty alias per the Secret Service specification) and rediscovered by exact label `Fresnica Device Unlock`, so implementations that support only the `default` alias are compatible. Fresnica then silently locks it if needed and performs the same raw Secret Service `Unlock` used for transactions. A real Secret Service-owned prompt must complete before Fresnica asks for the fresh Fresnica Passphrase or stores the unlock key. If the service unlocks silently, no prompt is available, or the prompt is cancelled, enable must fail and no unlock key may be stored. On a first failed attempt an empty `Fresnica Device Unlock` collection may remain; this is not an enabled wallet and avoids triggering an extra cleanup prompt. Fresnica must not request privileged installation.

After successful enrollment, status should be `locked` or `ready` with provider `Fresnica Secret Service collection`.

## Transaction authentication

Do not use `-y` for the first test.

```bash
"$BIN" --home "$HOME_DIR" --network testnet \
  send 0.0000001 XLM to GDESTINATION --wallet "$WALLET"
```

After review, expect the final choice:

```text
Device unlock: locked (Fresnica Secret Service collection)
[Enter] Authenticate, sign and submit / [p] Fresnica Passphrase / [c] Cancel:
```

`ready` is also possible before Fresnica deliberately relocks the dedicated collection.

Press Enter. Acceptance requires **exactly one Secret Service-owned authentication prompt for this transaction**. Only after that prompt completes may Fresnica read the unlock key, sign and submit.

The implementation deliberately distinguishes a real Secret Service `Prompt` from a silent `Unlock`. If the desktop service unlocks the collection without presenting a prompt, Fresnica must relock it and print that desktop authentication did not provide a user prompt, then require the fresh Fresnica Passphrase. Silent signing is a failure.

Repeat the payment. A second transaction must require a new desktop authentication prompt; there is no CLI session.

## Confirmation and fallback paths

- `p` uses a fresh Fresnica Passphrase and does not use Device Authentication.
- `c` cancels without signing/submitting and must not surprise-prompt for the Passphrase.
- Dismissing the Secret Service prompt cancels/fails closed; it must not silently fall back.
- Unsupported/promptless Secret Service behavior may explicitly fall back to the fresh Fresnica Passphrase.

Now repeat with `-y`:

```bash
"$BIN" --home "$HOME_DIR" --network testnet \
  send 0.0000001 XLM to GDESTINATION --wallet "$WALLET" -y
```

`-y` may skip Fresnica's text confirmation but **must not skip the Secret Service authentication prompt**.

## Lifecycle

```bash
"$BIN" --home "$HOME_DIR" --network testnet \
  wallet device-unlock disable "$WALLET"
"$BIN" --home "$HOME_DIR" --network testnet \
  wallet device-unlock status "$WALLET"
```

Disable requires the fresh Fresnica Passphrase. Status must become `disabled`.

## Report

Please report distro/version, desktop environment, Secret Service implementation, whether collection creation succeeds without the previous `Only the 'default' alias is supported` error, enable-time prompt count/result, whether a dedicated `Fresnica Device Unlock` collection appeared, status after enable, authentication prompt count per transaction, Enter/`p`/`c` results, `-y` result, disable/re-enable result, and exact output for failures.

Never send a mnemonic, S-key, Fresnica Passphrase, keyring password, or unlock-key material.
