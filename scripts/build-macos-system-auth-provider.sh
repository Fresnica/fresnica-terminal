#!/usr/bin/env bash
set -euo pipefail

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "macOS System Auth provider must be compiled on macOS." >&2
  exit 2
fi

upstream="${1:?usage: build-macos-system-auth-provider.sh UPSTREAM_DIR OUTPUT_DIR}"
output="${2:?usage: build-macos-system-auth-provider.sh UPSTREAM_DIR OUTPUT_DIR}"
mkdir -p "$output"
output="$(cd "$output" && pwd)"
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
project_root="$output/xcode-project"
build_root="$output/build"
obj_root="$output/obj"

"$script_dir/prepare-macos-system-auth-development-project.sh" \
  "$upstream" "$project_root" >/dev/null

xcodebuild \
  -project "$project_root/FresnicaSystemAuth.xcodeproj" \
  -target FresnicaSystemAuth \
  -configuration Release \
  CODE_SIGNING_ALLOWED=NO \
  SYMROOT="$build_root" \
  OBJROOT="$obj_root" \
  -quiet \
  build

app="$output/FresnicaSystemAuth.app"
rm -rf "$app"
ditto "$build_root/Release/FresnicaSystemAuth.app" "$app"
echo "$app"
