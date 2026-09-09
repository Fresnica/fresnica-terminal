# Linux Device Unlock acceptance

Test build only. Use Testnet and a disposable or low-value software wallet.

Product checkpoint: `9e479ea3263d972dc46c981b1667317dabd4c031` on `feat/terminal-device-unlock`.

## Scope

Fresnica is a single user-space binary. Device Unlock uses the desktop Secret Service (for example GNOME Keyring or KWallet). This test must not require root, a Fresnica helper, a daemon, a service, or a polkit policy.

## Requirements

- x86_64 desktop Linux.
- An active graphical user session with a Secret Service implementation.
- Testnet only.
- Run Fresnica as the normal desktop user.

Headless/SSH-only systems may correctly report Device Unlock as unavailable.

## Verify the package

From the extracted package directory:

```bash
sha256sum -c SHA256SUMS
chmod +x fresnica
./fresnica --version
```
The version line must identify Fresnica source `ecdc6bf77741`.

## Prepare an isolated test wallet

Use a protected Testnet software wallet. You may use an existing disposable wallet or create/import one with normal Fresnica wallet commands.

```bash
BIN="$PWD/fresnica"
HOME_DIR="$HOME/.fresnica-device-unlock-test"
WALLET="du-a"

"$BIN" --home "$HOME_DIR" --network testnet wallet list
```

Do not use a watch-only or Ledger-only wallet for Device Unlock enrollment.

## Enable and inspect Device Unlock

```bash
"$BIN" --home "$HOME_DIR" --network testnet wallet device-unlock enable "$WALLET"
"$BIN" --home "$HOME_DIR" --network testnet wallet device-unlock status "$WALLET"
```

Enable must request the fresh Fresnica Passphrase. It must not request system-wide installation or privileged setup.

Expected status is normally `ready` or `locked`, depending on the desktop keyring. `unavailable` is acceptable only when no compatible Secret Service is active.
## Sign a Testnet payment

Do not use `-y` for acceptance testing.

```bash
"$BIN" --home "$HOME_DIR" --network testnet send 0.0000001 XLM to GDESTINATION --wallet "$WALLET"
```

After the transaction review, an enrolled signer must produce one of these final choices:

```text
Device unlock: ready (...)
[Enter] Sign and submit / [p] Fresnica Passphrase / [c] Cancel:
```

or:

```text
Device unlock: locked (...)
[Enter] Unlock, sign and submit / [p] Fresnica Passphrase / [c] Cancel:
```

Acceptance:

- Enter uses Device Unlock and submits only after signing succeeds.
- `p` uses a fresh Fresnica Passphrase instead of Device Unlock.
- `c` cancels without signing or submitting and must not surprise-prompt for the Fresnica Passphrase.
- If the keyring is locked, any unlock UI must be owned by the desktop Secret Service, not Fresnica.
- Repeating the command while the keyring is already unlocked should not invent a second Fresnica authentication step.

## Lifecycle

```bash
"$BIN" --home "$HOME_DIR" --network testnet wallet device-unlock disable "$WALLET"
"$BIN" --home "$HOME_DIR" --network testnet wallet device-unlock status "$WALLET"
```

Disable requires the fresh Fresnica Passphrase. Status must become `disabled`. Re-enable and repeat one payment.

## Report

Please report:

- distro and version;
- desktop environment;
- Secret Service/keyring implementation if known;
- enable PASS/FAIL;
- `ready` or `locked` status;
- Enter sign+submit PASS/FAIL;
- `p` Passphrase path PASS/FAIL;
- `c` fail-close PASS/FAIL;
- disable/re-enable PASS/FAIL;
- exact error/output for any failure.

Never send a mnemonic, S-key, Fresnica Passphrase, or unlock-key material.
