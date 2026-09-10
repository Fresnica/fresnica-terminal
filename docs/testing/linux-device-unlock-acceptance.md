# Linux Device Authentication acceptance

Test build only. Use Testnet and a disposable or low-value software wallet.

Product checkpoint: `acc979c7e51747f28644f0a8213bbe416097a3b0` on `feat/terminal-device-auth-cross-platform`.

## Scope

Fresnica remains one user-space binary. Linux now keeps authentication and storage separate:

- **DeviceAuthenticator:** fprintd verifies a fingerprint enrolled for the current Linux user;
- **DeviceSecretStore:** the exact-envelope 32-byte unlock key lives in the user's existing default Secret Service collection.

Fresnica does not create, lock or unlock a keyring. It relies on the system's existing fprintd service but installs no Fresnica helper, daemon, service, polkit policy, PAM configuration or privileged setup.

The previous dedicated `Fresnica Device Unlock` keyring design is rejected: physical GNOME testing showed that it requires the user to create another keyring password. A leftover keyring from that test is not used by this build and can be removed manually later if desired.

## Requirements

- x86_64 desktop Linux with an active graphical user session;
- fprintd available with a usable fingerprint reader;
- at least one fingerprint already enrolled for the current Linux user;
- a Secret Service implementation such as GNOME Keyring or KWallet;
- the user's normal/default Secret Service collection already unlocked by the desktop session;
- Testnet only; run Fresnica as the normal desktop user.

If the machine has no fingerprint reader or enrolled fingerprint, Device Authentication is intentionally unsupported and Fresnica Passphrase remains the safe path.

## Prepare

```bash
chmod +x fresnica
./fresnica --version

BIN="$PWD/fresnica"
HOME_DIR="$HOME/.fresnica-device-unlock-test"
WALLET="du-a"
```

For a clean acceptance test, start with Device Unlock disabled or use a fresh Testnet wallet/home. Do not create a new `Fresnica Device Unlock` keyring if an older test binary asks for one; that binary is obsolete.

## Enable

```bash
"$BIN" --home "$HOME_DIR" --network testnet \
  wallet device-unlock status "$WALLET"

"$BIN" --home "$HOME_DIR" --network testnet \
  wallet device-unlock enable "$WALLET"
```

Successful enable must follow this order:

```text
Touch the fingerprint sensor to enable Fresnica Device Unlock.
<successful fingerprint scan>
Fresnica passphrase:
Device unlock enabled for wallet "du-a" on this device.
```

There must be **no new-keyring/password-creation prompt**. Only a successful fprintd verification may proceed to the Fresnica Passphrase and Secret Service write.

If fprintd, the reader or an enrolled fingerprint is unavailable, enable must fail before asking for the Fresnica Passphrase and must store nothing. If the default keyring is locked, enable must ask the user to unlock it in the desktop session and retry; Fresnica must not unlock it itself.

After successful enrollment:

```text
Device unlock: ready
Provider: Linux fingerprint
```

## Transaction authentication

Do not use `-y` for the first test.

```bash
"$BIN" --home "$HOME_DIR" --network testnet \
  send 0.0000001 XLM to GDESTINATION --wallet "$WALLET"
```

After review, expect:

```text
Device unlock: ready (Linux fingerprint)
[Enter] Authenticate, sign and submit / [p] Fresnica Passphrase / [c] Cancel:
```

Press Enter. Fresnica should print:

```text
Touch the fingerprint sensor to authenticate this Fresnica transaction.
```

A successful fingerprint verification authorizes key read, signing and submission for this one transaction. Repeat the payment: the next transaction must require a new fingerprint verification; there is no CLI session.

A completed fingerprint mismatch may be retried up to three times. fprintd quality-retry states such as an incomplete/too-short/not-centered scan stay inside the current attempt. Three real mismatches fail the transaction and must not silently fall back to the Fresnica Passphrase.

## Fallback and control paths

- `p` uses a fresh Fresnica Passphrase and must not invoke fprintd.
- `c` cancels before fingerprint verification and must not sign or submit.
- If fprintd/reader availability disappears after enrollment, Fresnica may explicitly require the fresh Fresnica Passphrase.
- If the default keyring is locked, Fresnica must not unlock it or start fingerprint verification; it explicitly requires the fresh Fresnica Passphrase.
- A fingerprint mismatch is authentication failure, not an availability condition; do not silently downgrade it.

Repeat with `-y`:

```bash
"$BIN" --home "$HOME_DIR" --network testnet \
  send 0.0000001 XLM to GDESTINATION --wallet "$WALLET" -y
```

`-y` may skip Fresnica's text confirmation but **must not skip fingerprint verification**.

## Lifecycle

```bash
"$BIN" --home "$HOME_DIR" --network testnet \
  wallet device-unlock disable "$WALLET"
"$BIN" --home "$HOME_DIR" --network testnet \
  wallet device-unlock status "$WALLET"
```

Disable requires the fresh Fresnica Passphrase but not a fingerprint. The default keyring must already be unlocked. Status must become `disabled`.

## Report

Please report distro/version, desktop environment, Secret Service implementation, fprintd version, fingerprint reader model if convenient, enable result, fingerprint attempt behavior, status after enable, payment result, second-payment fresh-auth result, `p`/`c`/`-y` results, disable/re-enable result, and exact output for any failure.

Never send a mnemonic, S-key, Fresnica Passphrase, fingerprint data, keyring password, or unlock-key material.
