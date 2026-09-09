# Windows Device Unlock acceptance

Test build only. Use Testnet and a disposable or low-value software wallet.

Product checkpoint: `9e479ea3263d972dc46c981b1667317dabd4c031` on `feat/terminal-device-unlock`.

## Scope

Fresnica is a single user-space `fresnica.exe`. Device Unlock uses the current Windows user's Credential Manager. This build has no Fresnica Windows service, LocalSystem broker, helper installation, UAC setup, or Administrator requirement.

Windows Hello is not claimed by this baseline; an enrolled Credential Manager entry is reported as `ready`.

## Requirements

- x86_64 Windows 10 or Windows 11.
- Testnet only.
- Run from an ordinary user PowerShell or Command Prompt.

## Verify the package

In PowerShell from the extracted package directory:

```powershell
Get-FileHash .\fresnica.exe -Algorithm SHA256
Get-Content .\SHA256SUMS.txt
.\fresnica.exe --version
```
Confirm the binary hash matches `SHA256SUMS.txt`. The version line must identify Fresnica source `ecdc6bf77741`.

## Prepare an isolated test wallet

Use a protected Testnet software wallet. You may use an existing disposable wallet or create/import one with normal Fresnica wallet commands.

```powershell
$BIN = "$PWD\fresnica.exe"
$HOME_DIR = "$HOME\.fresnica-device-unlock-test"
$WALLET = "du-a"

& $BIN --home $HOME_DIR --network testnet wallet list
```

Do not use a watch-only or Ledger-only wallet for Device Unlock enrollment.

## Enable and inspect Device Unlock

```powershell
& $BIN --home $HOME_DIR --network testnet wallet device-unlock enable $WALLET
& $BIN --home $HOME_DIR --network testnet wallet device-unlock status $WALLET
```

Enable must request the fresh Fresnica Passphrase. It must not show UAC, request Administrator privileges, install a service, or copy a helper into Program Files.

Expected enrolled status is `Device unlock: ready` with provider `Windows Credential Manager`.
Optional inspection:

```powershell
cmdkey.exe /list | Select-String "Fresnica:DeviceUnlock:"
```

## Sign a Testnet payment

Do not use `-y` for acceptance testing.

```powershell
& $BIN --home $HOME_DIR --network testnet send 0.0000001 XLM to GDESTINATION --wallet $WALLET
```

After the transaction review, expect:

```text
Device unlock: ready (Windows Credential Manager)
[Enter] Sign and submit / [p] Fresnica Passphrase / [c] Cancel:
```

Acceptance:

- Enter uses the current-user Device Unlock credential and submits only after signing succeeds.
- `p` uses a fresh Fresnica Passphrase instead.
- `c` cancels without signing or submitting and must not surprise-prompt for the Fresnica Passphrase.
- No UAC, Administrator prompt, service installation, or Fresnica helper process appears.
- Repeating the command remains a normal user-space flow.

## Lifecycle

```powershell
& $BIN --home $HOME_DIR --network testnet wallet device-unlock disable $WALLET
& $BIN --home $HOME_DIR --network testnet wallet device-unlock status $WALLET
```

Disable requires the fresh Fresnica Passphrase. Status must become `disabled`. Re-enable and repeat one payment.

## Report

Please report:

- Windows version;
- enable PASS/FAIL;
- `ready` status PASS/FAIL;
- Enter sign+submit PASS/FAIL;
- `p` Passphrase path PASS/FAIL;
- `c` fail-close PASS/FAIL;
- whether any UAC/service/helper behavior appeared (expected: no);
- disable/re-enable PASS/FAIL;
- exact error/output for any failure.

Never send a mnemonic, S-key, Fresnica Passphrase, or unlock-key material.
