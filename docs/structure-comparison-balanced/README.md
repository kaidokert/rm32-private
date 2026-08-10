# Balanced MinZ / rm32 structural comparison

This directory is a fresh comparison of the current working tree. Earlier
`docs/structure-comparison/` artifacts are historical inputs only and must not
be used as evidence for this pass.

## Scope

The inventory is deliberately symmetric and exhaustive within these surfaces:

- MinZ: every current file under `minz/core/src/`, `minz/src/`, and
  `minz/examples/`, plus manifests, build/link/target configuration.
- rm32: every current file under `rm32/src/` and `rm32_stm32/src/`, every board
  YAML, manifests and build scripts, plus the process black-box harness/vectors.

Deleted files are not implementation traits. They may be mentioned only in the
review metadata as proof that an earlier report is stale.

## Module disposition schema

Every in-scope implementation file receives one `modules` entry:

- `path`: repository-relative path.
- `layer`: `portable_core`, `firmware_orchestration`, `platform_adapter`,
  `target_family`, `configuration`, `protocol`, `diagnostics`, or `test`.
- `role`: concise responsibility derived from executable source.
- `wiring`: `active`, `active_optional`, `library_only`, `debug_only`,
  `test_only`, `declared_unwired`, or `configuration_data`.
- `runtime_context`: main/foreground, periodic ISR, event ISR, build time,
  host only, or not applicable.
- `counterpart`: closest file/concept on the other side, relationship
  (`direct`, `partial`, or `none`), and why.
- `evidence`: source paths with line numbers or symbols.
- `confidence`: `high`, `medium`, or `low`.

## Required aggregate sections

Each inventory also records:

- crate/package topology;
- runtime and interrupt flows;
- state ownership and cross-context communication;
- HAL and hardware-adapter boundaries;
- configuration and target-selection layers;
- protocol/input/output capabilities;
- safety, observability, serialization, and tooling;
- host tests, target builds, and runtime-verification boundaries;
- `side_only_capabilities` for every capability without a counterpart;
- limitations and uncertainty.

## Waves

1. Independent exhaustive inventories:
   `minz-modules.yaml`, `rm32-modules.yaml`, and `crosscut-map.yaml`.
2. Independent audits:
   `audit-minz.yaml`, `audit-rm32.yaml`, and `audit-crosscut.yaml`.
3. Consolidation:
   `audit.yaml` and `report.md`.

The final report must distinguish module presence from active firmware wiring
and must report one-sided capabilities in both directions.
