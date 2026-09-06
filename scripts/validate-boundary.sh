#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

rev="$(tr -d '[:space:]' < FRESNICA_REV)"
if [[ ! "$rev" =~ ^[0-9a-f]{40}$ ]]; then
  echo "FRESNICA_REV must contain one full Git commit SHA." >&2
  exit 1
fi

client_rev="$(awk '/^\[workspace.dependencies.fresnica-client\]$/{in_section=1; next} /^\[/{in_section=0} in_section && /^rev = /{print; exit}' Cargo.toml)"
if [[ "$client_rev" != "rev = \"$rev\"" ]]; then
  echo "fresnica-client must be pinned to FRESNICA_REV." >&2
  exit 1
fi

if grep -Fqx '[workspace.dependencies.fresnica-sdk]' Cargo.toml; then
  sdk_rev="$(awk '/^\[workspace.dependencies.fresnica-sdk\]$/{in_section=1; next} /^\[/{in_section=0} in_section && /^rev = /{print; exit}' Cargo.toml)"
  if [[ "$sdk_rev" != "rev = \"$rev\"" ]]; then
    echo "Direct fresnica-sdk workspace dependency must be pinned to FRESNICA_REV." >&2
    exit 1
  fi
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

if grep -RInE 'HorizonGateway|RpcGateway|MAINNET_HORIZON_URL|TESTNET_HORIZON_URL|TESTNET_RPC_URL|horizon_gateway|rpc_gateway' crates/*/src; then
  echo "Terminal products must consume provider-neutral Fresnica service APIs, not Horizon/RPC gateway adapters or endpoint constants." >&2
  exit 1
fi

if grep -RInE 'https://horizon(-testnet)?\.stellar\.org' crates/*/src; then
  echo "Terminal products must not hardcode shared Horizon defaults; provide overrides through NetworkProfile." >&2
  exit 1
fi

echo "Fresnica terminal repository boundary: OK ($rev)"
