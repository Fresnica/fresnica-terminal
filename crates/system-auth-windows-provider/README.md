# Fresnica Windows System Auth Provider

This crate builds the two narrowly scoped Windows components used by Fresnica Terminal System Auth:

- `fresnica-system-auth-provider.exe`: user-session Win32 WebAuthn / Windows Hello bridge;
- `fresnica-system-auth-service.exe`: LocalSystem broker that owns protected enrollment state.

Neither executable is a Fresnica command plugin. The provider is loaded only from the administrator-installed `%ProgramFiles%\Fresnica\SystemAuth` path. The service exposes only the local `\\.\pipe\FresnicaSystemAuth.v1` named pipe, rejects remote pipe clients, and verifies that the connecting process is the installed provider before using its Windows SID as the storage namespace.

Enrollment still begins with a fresh Fresnica Passphrase in the CLI. The service receives only the exact System Auth slot and verified 32-byte `WalletUnlockKey`. On the first enrollment for a Windows user, the provider uses the Win32 WebAuthn platform API to create an RSA-2048/RS256 credential with `userVerification=required`; the service validates the returned RP hash, user-presence/user-verification flags, credential id, and COSE public key. Later signer enrollment reuses that device/user domain and does not trigger another Hello prompt.

Routine release is cryptographically gated rather than implemented as "show Hello, then DPAPI decrypt": the service generates fresh WebAuthn client data, the provider calls `WebAuthNAuthenticatorGetAssertion` for the exact stored platform credential with user verification required, and the LocalSystem service verifies the credential id, RP hash, UV/UP flags, and RS256 assertion signature before releasing the exact enrolled unlock key. The Windows Hello/FIDO2 private key never enters Fresnica.

Enrollment records are additionally protected with DPAPI under the LocalSystem service account before being written beneath `%ProgramData%\Fresnica\SystemAuth`. DPAPI protects storage at rest; it is not treated as proof of user authentication.

Build both binaries with the Windows MSVC target, then run the development installer from an elevated PowerShell:

```powershell
.\scripts\install-windows-system-auth-development.ps1 `
  -ProviderBinary .\target\x86_64-pc-windows-msvc\release\fresnica-system-auth-provider.exe `
  -ServiceBinary .\target\x86_64-pc-windows-msvc\release\fresnica-system-auth-service.exe
```

The installer ACLs the program directory read/execute for ordinary users and writable only by SYSTEM/Administrators; the state directory remains SYSTEM/Administrators only. Removing the last signer enrollment also drops Fresnica's stored domain metadata, so `disable -> re-enable` can recover by creating a fresh Windows Hello credential if the prior platform credential was externally invalidated. Physical Windows Hello success/cancel/fallback and this recovery path still require a real interactive Windows session.
