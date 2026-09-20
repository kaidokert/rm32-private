# Frozen minz control reference — 2026-09-12

Archive: `minz_20260912.zip`

SHA256: `2799b284bca75ff97e7d5a8cb57eae4ba0ce32f581e80203f6bc7168e9606c44`

Git HEAD: `2efda4911c4be8e238e16d192d821151bd775887`.
HEAD alone is insufficient: this reference includes modified core/Cargo.toml
and core/src/{am32,am32_hal,am32_loop,zct_trace}.rs. The archive contains
128 payload files with individual byte lengths and SHA256 hashes in
`manifest.json`, scoped Git status, and a binary-capable tracked diff.

Includes core, MCU adapters, examples (including am32_clone), scripts, notes,
top-level Markdown and build metadata, plus parent workspace Cargo metadata.
Excludes captures, external dependencies and private optional dependency
trees. This freezes the control source; it is NOT a complete offline firmware
build closure or proof that historical captures used these exact bytes.

From binz:

```text
python scripts/freeze_minz_reference.py captures/reference/minz_20260912.zip --verify
python scripts/freeze_minz_reference.py captures/reference/minz_20260912.zip --check-source
cargo test --manifest-path ../minz/core/Cargo.toml --target x86_64-pc-windows-msvc
```

Check source before reference-dependent replay/build: current Cargo dependencies
still point to the live sibling. A mismatch requires investigation and an explicit
new reference, not silent baseline replacement. Archive creation refuses overwrite.

## Benchmark provenance: still open

E193 adds read-only replay of the separately retained historical FALCON data:

```text
python scripts/minz_historical_baseline.py --verify-archive captures/reference/minz_falcon_historical_20260913.zip
```

Checks membership,payload hashes,parser identity against the pinned source
archive,and recomputes all three capture summaries. Does not read live minz or
modify either archive. Reproducible metrics do NOT establish the flashed
firmware identity,calibration or matched-AM32 parity. Do not compare FALCON's
single-window sigma directly with binz's full-cycle sigma.

- CARRIER_REQUAL.md reports 1112–1661 eHz and the older SWIFT controller,
  not a matched 200 eHz AM32-clone benchmark. Do not use it as our pass bar.
- captures/map_cmp_am32.csv begins at 528 eHz, 8.15 V, 100 mA; its trace
  identity and motor/load need establishing before comparison.
- captures/am32check2_112917.csv reports 1642–2325 eHz at 7.25–7.82 V.
- captures/zctsweep_am32full.csv stores ticks, while cmp_am32_low_trace.csv
  stores microseconds. Neither filename proves the exact flashed reference.
- No matched-speed lock/dropout/recovery/current benchmark established yet.
  These are search findings, not an exhaustive claim that none exists.

Reference unit baseline: 83/83 host tests passed, including actual polling,
COMP/COM routines, recovery and desync tests. These are component tests, not
a timed integrated replay or motor-lock qualification. Cargo emitted an
incremental-cache access warning; compilation/test execution succeeded.
