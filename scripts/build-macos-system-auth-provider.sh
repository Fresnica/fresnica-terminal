#!/usr/bin/env bash
set -euo pipefail

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "macOS System Auth provider must be compiled on macOS." >&2
  exit 2
fi

upstream="${1:?usage: build-macos-system-auth-provider.sh UPSTREAM_DIR OUTPUT_DIR}"
output="${2:?usage: build-macos-system-auth-provider.sh UPSTREAM_DIR OUTPUT_DIR}"
source_dir="$upstream/bindings/native/platform/apple"
app="$output/FresnicaSystemAuth.app"
binary="$app/Contents/MacOS/fresnica-system-auth-provider"

rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$output/swift"
cp "$source_dir/FresnicaSystemAuthProviderMain.swift" "$output/swift/main.swift"

swiftc -O \
  "$source_dir/FresnicaWalletUnlockKeyStore.swift" \
  "$output/swift/main.swift" \
  -o "$binary"
cat > "$app/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleIdentifier</key>
  <string>com.fresnica.terminal.system-auth</string>
  <key>CFBundleName</key>
  <string>Fresnica System Auth</string>
  <key>CFBundleExecutable</key>
  <string>fresnica-system-auth-provider</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleVersion</key>
  <string>1</string>
  <key>CFBundleShortVersionString</key>
  <string>1</string>
</dict>
</plist>
PLIST

echo "$app"
