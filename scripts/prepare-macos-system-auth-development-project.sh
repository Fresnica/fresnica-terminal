#!/usr/bin/env bash
set -euo pipefail

upstream="${1:?usage: prepare-macos-system-auth-development-project.sh UPSTREAM_DIR OUTPUT_DIR}"
output="${2:?usage: prepare-macos-system-auth-development-project.sh UPSTREAM_DIR OUTPUT_DIR}"
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
template_dir="$script_dir/macos-system-auth-development"
source_dir="$upstream/bindings/native/platform/apple"

for file in FresnicaWalletUnlockKeyStore.swift FresnicaSystemAuthProviderMain.swift; do
  if [[ ! -f "$source_dir/$file" ]]; then
    echo "missing pinned upstream Apple source: $source_dir/$file" >&2
    exit 2
  fi
done

rm -rf "$output"
mkdir -p "$output/Sources"
cp "$source_dir/FresnicaWalletUnlockKeyStore.swift" "$output/Sources/"
cp "$source_dir/FresnicaSystemAuthProviderMain.swift" "$output/Sources/main.swift"
cp "$template_dir/FresnicaSystemAuth.entitlements" "$output/"
cp -R "$template_dir/FresnicaSystemAuth.xcodeproj" "$output/"

echo "$output/FresnicaSystemAuth.xcodeproj"
