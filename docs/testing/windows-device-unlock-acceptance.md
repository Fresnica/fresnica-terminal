# Windows Device Authentication acceptance

Test build only. Use Testnet and a disposable or low-value software wallet.

Product checkpoint: `67eae19b1600e8e1ad33f522fe4c2c855be4109d` on `feat/terminal-device-auth-cross-platform`.

## Scope

Fresnica remains one user-space `fresnica.exe`. Windows now separates:

- **DeviceAuthenticator:** Windows Hello current-user verification when an enrolled signer is first needed in a CLI process;
- **DeviceSecretStore:** Credential Manager stores only the exact-envelope 32-byte unlock key.

There is no Fresnica service, LocalSystem broker, helper installation, UAC setup, or Administrator requirement.

## Requirements

- x86_64 Windows 11, build 22000 or newer, for this Windows Hello desktop interop slice.
- Windows Hello configured for the current user (PIN, fingerprint, face, or another system-supported verifier).
- Testnet only; ordinary user PowerShell or Command Prompt.

On an unsupported Windows version, or when Windows Hello is unavailable/not configured, `device-unlock enable` must fail before asking for the Fresnica Passphrase or writing Credential Manager. Transaction-time loss of Hello after a successful enrollment may explicitly fall back to the fresh Fresnica Passphrase.

## Prepare

```powershell
.\fresnica.exe --version
$BIN = "$PWD\fresnica.exe"
$HOME_DIR = "$HOME\.fresnica-device-unlock-test"
$WALLET = "du-a"
```

For this acceptance test, use a wallet with Device Unlock currently disabled. An older Credential Manager enrollment may still be readable, but it does not prove the new enable-time hard gate; disable it first or use a fresh test wallet/home.

## Enable and inspect

```powershell
& $BIN --home $HOME_DIR --network testnet wallet device-unlock enable $WALLET
& $BIN --home $HOME_DIR --network testnet wallet device-unlock status $WALLET
```

Enable must first show a Windows Hello-owned verification with the reason `Authenticate to enable Fresnica Device Unlock`. Only after Hello returns `Verified` may Fresnica prompt for the fresh Fresnica Passphrase and write Credential Manager. If Hello is cancelled, unavailable, not configured, disabled by policy, busy, or otherwise cannot verify the user, enable must fail immediately: no Fresnica Passphrase prompt and no new `Fresnica:DeviceUnlock:` credential may be created. It must not show UAC, request Administrator privileges, install a service, or copy a helper.

On a machine without usable Hello, expect an error such as:

```text
Windows Hello unavailable (not configured for the current user); Device Unlock was not enabled
```

Then `status` must remain `disabled`, and `cmdkey.exe /list | Select-String "Fresnica:DeviceUnlock:"` must not show a newly created target for this wallet.

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

After review, Fresnica shows only its normal transaction confirmation:

```text
Submit this transaction? [y/N]
```

Enter `y`. If the selected software signer has Device Unlock enrolled, Windows Hello must start automatically; there is no separate Device Unlock selection prompt. Acceptance requires **at most one Windows Hello-owned verification in this CLI process**. Complete it with the Windows-supported method offered on the machine, such as PIN, fingerprint, or face. Only after verification succeeds may Fresnica read Credential Manager, sign and submit.

Run the payment again as a new CLI invocation. It must request Windows Hello again. A single multi-stage operation may reuse one successful verification only until that `fresnica` process exits.

## Confirmation and fallback paths

- A wallet without Device Unlock enrollment goes directly to the Fresnica Passphrase after normal transaction confirmation, with no Device Unlock prompt.
- Cancel the Windows Hello dialog: Fresnica must abort/fail closed and must not silently ask for the Passphrase.
- If Hello is unavailable or unsupported, Fresnica explicitly prints `Windows Hello unavailable (<reason>); Fresnica Passphrase required.` and then uses the fresh Passphrase path. It must never silently read Credential Manager. Report the exact `<reason>`.
- No UAC, service, helper, or Administrator flow may appear.

Now repeat with `-y`:

```powershell
& $BIN --home $HOME_DIR --network testnet \
  send 0.0000001 XLM to GDESTINATION --wallet $WALLET -y
```

`-y` may skip Fresnica's normal transaction confirmation but **must not skip Windows Hello verification** for an enrolled signer.

If Windows Hello never appears after transaction confirmation, copy the full `Windows Hello unavailable (...)` reason and report whether the command was run in Windows Terminal, classic console host, PowerShell, or Command Prompt. This build calls the desktop `IUserConsentVerifierInterop` request directly and does not preflight it through `CheckAvailabilityAsync`.

## Lifecycle

```powershell
& $BIN --home $HOME_DIR --network testnet wallet device-unlock disable $WALLET
& $BIN --home $HOME_DIR --network testnet wallet device-unlock status $WALLET
```

Disable requires the fresh Fresnica Passphrase. Status must become `disabled`.

## Report

Please report Windows version/build, terminal host, Hello method used, enable/status result, Windows Hello prompt count per CLI invocation, automatic-use result, Hello-dialog cancellation result, `-y` result, whether any UAC/service/helper appeared, disable/re-enable result, and exact output for failures.

Never send a mnemonic, S-key, Fresnica Passphrase, PIN, or unlock-key material.
