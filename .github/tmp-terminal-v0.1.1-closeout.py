from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    target = Path(path)
    text = target.read_text()
    if text.count(old) != 1:
        raise SystemExit(f"expected exactly one match in {path}: {old!r}")
    target.write_text(text.replace(old, new, 1))


def replace_section(path: str, start_marker: str, end_marker: str | None, replacement: str) -> None:
    target = Path(path)
    text = target.read_text()
    if text.count(start_marker) != 1:
        raise SystemExit(f"expected exactly one start marker in {path}: {start_marker!r}")
    start = text.index(start_marker)
    if end_marker is None:
        end = len(text)
        suffix = "\n"
    else:
        if text.count(end_marker) != 1:
            raise SystemExit(f"expected exactly one end marker in {path}: {end_marker!r}")
        end = text.index(end_marker, start)
        suffix = "\n\n" + text[end:].lstrip("\n")
    target.write_text(text[:start] + replacement.rstrip() + suffix)


refactor = "docs/terminal-shared-foundation-refactor.md"
replace_once(
    refactor,
    "Status: active — foundation, CLI and TUI hardening complete; final integration pending",
    "Status: complete — merged and released as Fresnica Terminal v0.1.1",
)
replace_once(
    refactor,
    "Branch: `refactor/terminal-shared-foundation`",
    "Milestone branch: `refactor/terminal-shared-foundation` (merged via PR #3)",
)
replace_section(
    refactor,
    "### Phase 6 - Final integration and release — active",
    "## Non-goals",
    """### Phase 6 - Final integration and release — complete

The completed implementation was finalized through the repository's normal merge and release gates:

1. PR #3 (`refactor: establish Terminal shared capability foundation`) passed formal `CI #33` and `Release Terminal #20` on exact head `92f64e396b410d074d218dfec6ac5d58e8d410bb`, including RefPython compatibility and Linux/macOS/Windows package builds.
2. PR #3 was squash-merged to `main` as `fcf0e5b177283e2dfd0a07d1180f0fc7189ae2ec`; post-merge `CI #34` passed.
3. Compatibility impact was classified as patch-level: implementation hardening and bug fixes without public command-semantic breakage.
4. Release PR #4 changed only the CLI/TUI package versions, their two `Cargo.lock` entries, and the new immutable `releases/terminal-v0.1.1.json` marker. Formal `CI #35` and `Release Terminal #21` passed, including all three platform packages.
5. PR #4 was squash-merged to `main` as `88fe7067062521789044a643b6816cc70f77aeaa`; post-merge `CI #36` passed independently.
6. `Release Terminal #22` passed validate, Linux/macOS/Windows packaging and publish, creating the `v0.1.1` prerelease targeted exactly at `88fe7067062521789044a643b6816cc70f77aeaa`.

Published v0.1.1 asset digests recorded by GitHub Release metadata:

```text
fresnica-terminal-0.1.1-linux-x64.zip    064cb3469e4e5c4a7f008bcecf3aeeed18b17f88256e3d676ce10e89c4230743
fresnica-terminal-0.1.1-macos-arm64.zip   e0d8d609d584030fb93fc38416b7674d2015e0b0506c089e3fbb38f3bcfdb82a
fresnica-terminal-0.1.1-windows-x64.zip   1d7382592648be6950b016142b09e1927ba272f69a210ee9d18f6b379bd3417d
fresnica-terminal-release-manifest.json   23f14b34a4a3ac7183c7f3a1d1e12564a797113488b47bc6a955e72203409c94
SHA256SUMS                                 c4575d34812d7fbacdbaf048ee32adecc74546effee0985fa2e458d31cfcfac8
```

The release manifest was generated from the downloaded platform artifacts by the publish job and records version `0.1.1`, release commit `88fe7067062521789044a643b6816cc70f77aeaa`, and shared Fresnica source `9ba6f23cefe34e8d5940b311ec78f27eed982fe7`.
""",
)
replace_once(
    refactor,
    "## Definition of done\n\nThis milestone is complete when:\n",
    "## Definition of done — satisfied\n\nAll milestone conditions are satisfied:\n",
)

audit = "docs/terminal-flow-audit.md"
replace_once(
    audit,
    "Status: active evidence record — implementation hardening complete; final integration pending",
    "Status: complete evidence record — implementation merged and released as v0.1.1",
)
replace_once(
    audit,
    "Branch: `refactor/terminal-shared-foundation`",
    "Milestone branch: `refactor/terminal-shared-foundation` (merged via PR #3)",
)
replace_section(
    audit,
    "## Final integration gates",
    None,
    """## Final integration evidence

All final gates were satisfied:

1. The final refactor diff was reviewed against `main@a742ef3130e455c9cbdbf42378d07f3e1f30153f`; temporary verifier/workflow files were absent and the historical v0.1.0 marker remained byte-identical.
2. PR #3 exact head `92f64e396b410d074d218dfec6ac5d58e8d410bb` passed `CI #33` and `Release Terminal #20`, then squash-merged as `main@fcf0e5b177283e2dfd0a07d1180f0fc7189ae2ec`; post-merge `CI #34` passed.
3. The compatibility impact was patch-level. PR #4 prepared v0.1.1 with an exact four-file release diff and passed `CI #35` plus `Release Terminal #21` including Linux/macOS/Windows packaging.
4. PR #4 squash-merged as `main@88fe7067062521789044a643b6816cc70f77aeaa`; independent post-merge `CI #36` passed.
5. `Release Terminal #22` published prerelease `v0.1.1` targeted exactly at `88fe7067062521789044a643b6816cc70f77aeaa` with Linux x64, macOS arm64 and Windows x64 archives, release manifest and `SHA256SUMS`.
6. GitHub Release metadata records SHA-256 digests `064cb346...` (Linux), `e0d8d609...` (macOS), `1d738259...` (Windows), `23f14b34...` (manifest) and `c4575d34...` (`SHA256SUMS`).

This audit is now a closed record for the v0.1.1 hardening milestone. Future Terminal work should start from current `main`, current contracts and new repository evidence rather than extending this refactor plan mechanically.
""",
)

for path in (refactor, audit):
    text = Path(path).read_text()
    if "final integration pending" in text:
        raise SystemExit(f"stale pending marker remains in {path}")
