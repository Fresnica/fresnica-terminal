#!/usr/bin/env bash
set -euo pipefail

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "This development installer must run on macOS." >&2
  exit 2
fi

upstream="${1:?usage: install-macos-system-auth-development.sh UPSTREAM_DIR DESTINATION_DIR}"
destination="${2:?usage: install-macos-system-auth-development.sh UPSTREAM_DIR DESTINATION_DIR}"
bundle_id="com.fresnica.terminal.system-auth"
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
identity="${FRESNICA_APPLE_SIGNING_IDENTITY:-}"
if [[ -z "$identity" ]]; then
  identity="$(security find-identity -v -p codesigning \
    | awk -F'"' '/"Apple Development:/ {print $2; exit}')"
fi
if [[ -z "$identity" ]]; then
  echo "no valid Apple Development code-signing identity was found" >&2
  exit 2
fi

team_id="${FRESNICA_APPLE_TEAM_ID:-}"
if [[ -z "$team_id" ]]; then
  certificate="$(security find-certificate -c "$identity" -p 2>/dev/null || true)"
  if [[ -n "$certificate" ]]; then
    team_id="$(printf '%s\n' "$certificate" \
      | openssl x509 -noout -subject -nameopt RFC2253 2>/dev/null \
      | sed -E 's/^subject=//' \
      | tr ',' '\n' \
      | sed -n 's/^OU=//p' \
      | head -n 1)"
  fi
fi
if [[ ! "$team_id" =~ ^[A-Z0-9]{10}$ ]]; then
  echo "could not derive the Apple Team ID from signing identity: $identity" >&2
  echo "set FRESNICA_APPLE_TEAM_ID explicitly and retry" >&2
  exit 2
fi

work="$(mktemp -d "${TMPDIR:-/tmp}/fresnica-system-auth-dev.XXXXXX")"
trap 'rm -rf "$work"' EXIT
"$script_dir/prepare-macos-system-auth-development-project.sh" "$upstream" "$work" >/dev/null

build_root="$work/build"
obj_root="$work/obj"

xcodebuild \
  -project "$work/FresnicaSystemAuth.xcodeproj" \
  -target FresnicaSystemAuth \
  -configuration Release \
  -allowProvisioningUpdates \
  -allowProvisioningDeviceRegistration \
  DEVELOPMENT_TEAM="$team_id" \
  CODE_SIGN_IDENTITY="Apple Development" \
  CODE_SIGN_STYLE=Automatic \
  PRODUCT_BUNDLE_IDENTIFIER="$bundle_id" \
  SYMROOT="$build_root" \
  OBJROOT="$obj_root" \
  -quiet \
  build

app="$build_root/Release/FresnicaSystemAuth.app"
provider="$app/Contents/MacOS/fresnica-system-auth-provider"
profile="$app/Contents/embedded.provisionprofile"

if [[ ! -x "$provider" ]]; then
  echo "Xcode did not produce the System Auth provider executable" >&2
  exit 1
fi
if [[ ! -f "$profile" ]]; then
  echo "Xcode did not embed a macOS development provisioning profile" >&2
  exit 1
fi

codesign --verify --deep --strict --verbose=2 "$app"
signed_entitlements="$work/signed-entitlements.plist"
profile_plist="$work/profile.plist"
codesign -d --entitlements :- "$provider" 2>/dev/null > "$signed_entitlements"
security cms -D -i "$profile" > "$profile_plist"

plist_value() {
  /usr/libexec/PlistBuddy -c "Print :$1" "$2" 2>/dev/null || true
}

signed_team="$(plist_value com.apple.developer.team-identifier "$signed_entitlements")"
signed_app_id="$(plist_value com.apple.application-identifier "$signed_entitlements")"
if [[ -z "$signed_app_id" ]]; then
  signed_app_id="$(plist_value application-identifier "$signed_entitlements")"
fi
signed_group="$(plist_value keychain-access-groups:0 "$signed_entitlements")"
profile_team="$(plist_value TeamIdentifier:0 "$profile_plist")"
profile_prefix="$(plist_value ApplicationIdentifierPrefix:0 "$profile_plist")"
if [[ ! "$profile_prefix" =~ ^[A-Z0-9]{10}$ ]]; then
  echo "development profile has no valid App ID prefix" >&2
  exit 1
fi
expected="$profile_prefix.$bundle_id"

if [[ "$signed_team" != "$team_id" || "$profile_team" != "$team_id" ]]; then
  echo "development signature/profile Team ID mismatch" >&2
  exit 1
fi
if [[ "$signed_app_id" != "$expected" || "$signed_group" != "$expected" ]]; then
  echo "development keychain entitlement mismatch" >&2
  echo "expected: $expected" >&2
  exit 1
fi

mkdir -p "$destination"
installed="$destination/FresnicaSystemAuth.app"
rm -rf "$installed"
ditto "$app" "$installed"
codesign --verify --deep --strict --verbose=2 "$installed"

installed_provider="$installed/Contents/MacOS/fresnica-system-auth-provider"
set +e
"$installed_provider" probe >/dev/null 2>&1
probe_status=$?
set -e
if [[ "$probe_status" -ne 0 && "$probe_status" -ne 10 ]]; then
  echo "signed System Auth provider probe failed with exit $probe_status" >&2
  exit 1
fi

cat <<EOF
Fresnica macOS System Auth development companion installed.
app: $installed
bundle: $bundle_id
team: $team_id
app-id-prefix: $profile_prefix
identity: $identity
probe: $probe_status
EOF
