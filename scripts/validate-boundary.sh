#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

rev="$(tr -d '[:space:]' < FRESNICA_REV)"
if [[ ! "$rev" =~ ^[0-9a-f]{40}$ ]]; then
  echo "FRESNICA_REV must contain one full Git commit SHA." >&2
  exit 1
fi

if [[ "$(grep -F -c "rev = \"$rev\"" Cargo.toml)" -ne 2 ]]; then
  echo "Both shared Fresnica workspace dependencies must be pinned to FRESNICA_REV." >&2
  exit 1
fi

if grep -RInE 'fresnica-core|fresnica_core' Cargo.toml crates/*/Cargo.toml crates/*/src; then
  echo "Terminal products must not depend on or import fresnica-core directly." >&2
  exit 1
fi

if grep -RInE 'fresnica-(client|sdk)[[:space:]]*=.*path[[:space:]]*=' Cargo.toml crates/*/Cargo.toml; then
  echo "Terminal products must consume shared Fresnica crates through the pinned Git revision, not repository-relative paths." >&2
  exit 1
fi

if grep -RInE 'clients/rust-(cli|tui)|reference/rust-client|\.\./\.\./(core|sdk|reference)' Cargo.toml crates/*/Cargo.toml crates/*/src; then
  echo "Terminal repository contains stale monorepo-relative paths." >&2
  exit 1
fi

echo "Fresnica terminal repository boundary: OK ($rev)"
