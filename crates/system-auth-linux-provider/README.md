# Fresnica Linux System Auth Provider

This crate is the separately installed high-trust Linux companion for Fresnica Terminal System Auth. It is not a `fresnica-*` command plugin and is not searched on `PATH`.

The installed provider runs from `/usr/libexec/fresnica-system-auth-provider` as a root-owned setuid executable. Enrollment records are scoped to the real caller UID and stored under root-only `/var/lib/fresnica-system-auth`. Only the exact 32-byte `WalletUnlockKey` is stored; the provider never receives a Fresnica Passphrase, mnemonic, Stellar secret key, transaction XDR, or generic signing authority.

`release` asks polkit to authorize the exact parent process using `pid,start-time,uid`. The action `com.fresnica.system-auth.release` is `auth_self` for active local sessions and has no temporary authorization cache. Exit code 10 means Fresnica should ask for a fresh Passphrase, 11 means the user cancelled, and other failures are fail-closed provider errors.

Build as an ordinary user:

```sh
cargo build --release -p fresnica-system-auth-linux-provider --bin fresnica-system-auth-provider
```

Then install the already-built binary and polkit policy as root:

```sh
sudo scripts/install-linux-system-auth-provider.sh target/release/fresnica-system-auth-provider
```

A headless/SSH machine without a suitable polkit authentication agent is intentionally not treated as System Auth-capable for release; the provider returns the explicit Passphrase fallback outcome instead.
