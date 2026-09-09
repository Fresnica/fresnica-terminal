# Windows Device Authentication acceptance

Test build only. Use Testnet and a disposable or low-value software wallet.

Product checkpoint: `a224a02ea6a1e42f76c734ae26728f473f0d159b` on `feat/terminal-device-auth-cross-platform`.

## Scope

Fresnica remains one user-space `fresnica.exe`. Windows now separates:

- **DeviceAuthenticator:** Windows Hello current-user verification for every interactive transaction;
- **DeviceSecretStore:** Credential Manager stores only the exact-envelope 32-byte unlock key.

There is no Fresnica service, LocalSystem broker, helper installation, UAC setup, or Administrator requirement.

## Requirements

- x86_64 Windows 11, build 22000 or newer, for this Windows Hello desktop interop slice.
- Windows Hello configured for the current user (PIN, fingerprint, face, or another system-supported verifier).
- Testnet only; ordinary user PowerShell or Command Prompt.

On an unsupported Windows version, or when Windows Hello is unavailable/not configured, Fresnica must require the fresh Fresnica Passphrase instead of silently reading Credential Manager.

## Prepare

```powershell
.\fresnica.exe --version
$BIN = "$PWD\fresnica.exe"
$HOME_DIR = "$HOME\.fresnica-device-unlock-test"
$WALLET = "du-a"
```

An enrollment created by the previous Credential Manager test build uses the same exact-slot target and can be reused.

## Enable and inspect

```powershell
& $BIN --home $HOME_DIR --network testnet wallet device-unlock enable $WALLET
& $BIN --home $HOME_DIR --network testnet wallet device-unlock status $WALLET
```

Enable requires the fresh Fresnica Passphrase. It must not show UAC, request Administrator privileges, install a service, or copy a helper.

An enrolled credential is normally reported as:

```text
Device unlock: ready
Provider: Windows Hello
```

Credential Manager remains only the storage layer. Optional inspection:

```powershell
cmdkey.exe /list | Select-String "Fresnica:DeviceUnlock:"
```

## Transaction authentication

Do not use `-y` for the first test.

```powershell
& $BIN --home $HOME_DIR --network testnet \
  send 0.0000001 XLM to GDESTINATION --wallet $WALLET
```

After review, expect:

```text
Device unlock: ready (Windows Hello)
[Enter] Authenticate, sign and submit / [p] Fresnica Passphrase / [c] Cancel:
```

Press Enter. Acceptance requires **exactly one Windows Hello-owned verification for this transaction**. Complete it with the Windows-supported method offered on the machine, such as PIN, fingerprint, or face. Only after verification succeeds may Fresnica read Credential Manager, sign and submit.

Repeat the payment. The second transaction must request Windows Hello again; the CLI does not retain a session.

## Confirmation and fallback paths

- `p` uses a fresh Fresnica Passphrase instead of Windows Hello.
- `c` cancels before Windows Hello and does not sign or submit.
- Cancel the Windows Hello dialog: Fresnica must abort/fail closed and must not silently ask for the Passphrase.
- If Hello is unavailable or unsupported, Fresnica explicitly prints `Windows Hello unavailable; Fresnica Passphrase required.` and then uses the fresh Passphrase path. It must never silently read Credential Manager.
- No UAC, service, helper, or Administrator flow may appear.

Now repeat with `-y`:

```powershell
& $BIN --home $HOME_DIR --network testnet \
  send 0.0000001 XLM to GDESTINATION --wallet $WALLET -y
```

`-y` may skip Fresnica's text confirmation but **must not skip Windows Hello verification**.

If Windows Hello never appears when Enter is selected, report whether the command was run in Windows Terminal, classic console host, PowerShell, or Command Prompt. Window ownership remains part of physical acceptance.

## Lifecycle

```powershell
& $BIN --home $HOME_DIR --network testnet wallet device-unlock disable $WALLET
& $BIN --home $HOME_DIR --network testnet wallet device-unlock status $WALLET
```

Disable requires the fresh Fresnica Passphrase. Status must become `disabled`.

## Report

Please report Windows version/build, terminal host, Hello method used, enable/status result, Windows Hello prompt count per transaction, Enter/`p`/`c` results, Hello-dialog cancellation result, `-y` result, whether any UAC/service/helper appeared, disable/re-enable result, and exact output for failures.

Never send a mnemonic, S-key, Fresnica Passphrase, PIN, or unlock-key material.
