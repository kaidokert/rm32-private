# minz / rm32 structure comparison evidence

This directory contains review artifacts for a cascaded structural comparison of
`minz/` against the combined `rm32/` + `rm32_stm32/` implementation.

## Evidence schema

Each YAML inventory records:

- `scope`: the trees and concerns inspected.
- `method`: searches and source-reading approach.
- `traits`: structural properties, each with `claim`, `evidence` (repository-relative
  paths plus line numbers or symbols), and `confidence`.
- `abstractions`: named interfaces, state containers, orchestration layers, hardware
  adapters, and build/configuration mechanisms.
- `flows`: important runtime paths expressed as ordered symbol/path steps.
- `tests_and_verification`: how the implementation is exercised.
- `limitations`: explicit uncertainty or omitted/generated/vendor material.
- `comparison_candidates`: likely differences or overlaps for the consolidation pass.

Claims without direct source evidence are not eligible for the final report. Markdown
design notes can be used as context, but code, manifests, build scripts, and tests are
preferred as authoritative evidence.

## Passes

1. Independent inventories: `minz-inventory.yaml`, `rm32-inventory.yaml`, and
   `crosscut-inventory.yaml`.
2. Evidence audit and reconciliation: `audit.yaml`.
3. Consolidated report: `report.md`.
