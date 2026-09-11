# Linux Device Unlock physical acceptance

Status: **OPEN**

This guide validates the current Linux Device Unlock product on a real desktop session.

## Product boundary

Linux Device Unlock has two independent roles:

- **DeviceAuthenticator:** Polkit `auth_self` asks the current desktop user to authenticate through the system authentication agent.
- **DeviceSecretStore:** the current user's existing default Secret Service collection stores the exact-envelope 32-byte `WalletUnlockKey`.

Fresnica does not create, lock, or unlock a keyring. It does not install a helper, daemon, service, setuid binary, PAM configuration, or root key store.

The only system file is a UID-scoped Polkit action embedded in the `fresnica` binary and managed by `device-unlock enable|disable`:

```text
/usr/share/polkit-1/actions/com.fresnica.device-unlock.<UID>.policy
```

The action id is:

```text
com.fresnica.device-unlock.authenticate.<UID>
```

## Prerequisites

Use a non-root Linux desktop user with:

- a running Polkit authority and a registered desktop authentication agent;
- an existing default Secret Service collection, already unlocked by the desktop session;
- `pkexec` or `sudo` available for the one-time UID policy install/refresh;
- a protected Fresnica software signer and its Fresnica Passphrase.

The system authentication method is chosen by the desktop/PAM stack. Fresnica does not require or detect a specific fingerprint reader.

## 1. First enable

Run:

```text
fresnica wallet device-unlock enable NAME
```

Expected order on the first enable for this Linux UID:

1. Fresnica confirms the default Secret Service collection is available and unlocked.
2. Fresnica reports that one-time Linux Device Unlock system setup is required.
3. The OS may request administrator authentication while Fresnica installs the UID policy.
4. Polkit requests fresh authentication for the current Linux user.
5. Only after successful system authentication does Fresnica ask for the Fresnica Passphrase.
6. The verified 32-byte unlock key is stored in the current user's default Secret Service collection.

No separate Fresnica/system password, new keyring, or helper installation is allowed.

## 2. Policy lifecycle

After enable, verify the installed action is UID-scoped:

```text
pkaction --action-id com.fresnica.device-unlock.authenticate.$(id -u) --verbose
```

Expected implicit authorization:

```text
any:      no
inactive: no
active:   auth_self
```

There must be no `auth_self_keep` policy.

Re-running enable with an outdated embedded policy must replace that UID policy atomically before enrollment/authentication continues. A current policy must not trigger administrator setup again.

Different Linux users must have different policy filenames and action ids. One user enabling or disabling Device Unlock must not change another user's policy.

## 3. Status

Run:

```text
fresnica wallet device-unlock status NAME
```

After successful enrollment the expected state is `ready` and the provider is `Linux system authentication`. Status must not authenticate or release the stored unlock key.

## 4. Automatic signing authentication

Prepare a Testnet write such as a payment with the enrolled software signer.

The command must show the normal transaction review and normal submit confirmation only. There must be no Fresnica prompt asking whether to use Device Unlock or the Passphrase.

After the user approves submission, Device Unlock is selected automatically because the exact signer envelope is enrolled. Polkit must request current-user authentication before Secret Service releases the unlock key.

Within one `fresnica` process, successful system authentication may be reused for later signing stages. A Soroban invocation that needs both detached authorization signing and final envelope signing must therefore authenticate **at most once** in that CLI process.

A new CLI process must not inherit Fresnica authentication state.

If the signer has no Device Unlock enrollment, signing goes directly to the Fresnica Passphrase without probing or announcing Device Unlock.

If Polkit, its authentication agent, the UID policy, or the unlocked Secret Service path becomes unavailable after enrollment, Fresnica may explicitly require the Fresnica Passphrase. If the user cancels an actual system-authentication request, the operation must cancel/fail closed rather than silently downgrade.

`-y` may skip Fresnica's transaction confirmation, but it must never bypass system authentication.

## 5. Disable and cleanup

Run:

```text
fresnica wallet device-unlock disable NAME
```

Disable requires the fresh Fresnica Passphrase. It removes only that exact signer enrollment. If other Device Unlock enrollments remain for the same Linux UID, the UID policy remains installed. When the current user's last enrollment is removed, Fresnica automatically removes that UID policy; administrator authentication may be requested for the removal.

## 6. Negative cases

Verify these fail safely:

- locked default Secret Service collection: enable stops before administrator/system authentication and asks the user to unlock the desktop keyring first;
- missing `pkexec` and `sudo` when policy setup is required: enable explains that one of those installers is needed;
- inactive/non-local session or no suitable Polkit authentication path: routine signing requires the Fresnica Passphrase;
- Polkit action already grants authorization without a fresh challenge: Fresnica rejects it as Device Authentication and requires the Fresnica Passphrase;
- stale exact-envelope enrollment after re-protection: the old unlock key must not sign the changed signer envelope;
- cancelled Polkit authentication: do not sign, submit, or silently request the Passphrase as if authentication were merely unavailable.

## Acceptance report

Report distro/version, desktop environment, Polkit version/agent, Secret Service implementation, enable result, whether one-time policy setup appeared, system-auth method offered, status after enable, Testnet write result, multi-stage one-process authentication count if tested, disable result, policy cleanup result, and exact output for any failure.

Never send a mnemonic, S-key, Fresnica Passphrase, OS credential, biometric data, keyring password, or unlock-key material.
