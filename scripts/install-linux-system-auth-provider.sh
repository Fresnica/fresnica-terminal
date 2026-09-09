#!/usr/bin/env bash
set -euo pipefail

provider="${1:?usage: install-linux-system-auth-provider.sh PROVIDER_BINARY}"
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/.." && pwd)"
policy="$repo_root/packaging/linux/com.fresnica.system-auth.policy"
provider_destination="/usr/libexec/fresnica-system-auth-provider"
policy_destination="/usr/share/polkit-1/actions/com.fresnica.system-auth.policy"

if [[ "$(uname -s)" != "Linux" ]]; then
  echo "This installer is Linux-only." >&2
  exit 2
fi
if [[ "${EUID:-$(id -u)}" -ne 0 ]]; then
  echo "Run this installer as root after building the provider as your normal user." >&2
  exit 2
fi
if [[ ! -x "$provider" ]]; then
  echo "provider binary is not executable: $provider" >&2
  exit 2
fi
if [[ ! -f "$policy" ]]; then
  echo "polkit policy is missing: $policy" >&2
  exit 2
fi

install -D -o root -g root -m 0755 "$provider" "$provider_destination"
chmod 4755 "$provider_destination"
install -D -o root -g root -m 0644 "$policy" "$policy_destination"

if ! pkaction --action-id com.fresnica.system-auth.release >/dev/null 2>&1; then
  echo "polkit did not load com.fresnica.system-auth.release" >&2
  exit 1
fi

owner_mode="$(stat -c '%U:%G %a' "$provider_destination")"
if [[ "$owner_mode" != "root:root 4755" ]]; then
  echo "unexpected provider ownership/mode: $owner_mode" >&2
  exit 1
fi

echo "Fresnica Linux System Auth provider installed."
echo "provider: $provider_destination"
echo "policy: $policy_destination"
echo "authorization: polkit auth_self for active local sessions; no keep/caching policy"
