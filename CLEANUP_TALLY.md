# Cleanup Tally

This file defines the accounting used for the public burn-down tally.

The tally is measured from the public squash projection, not from the raw
private `am32_sheet` branch. The raw private branch contains private docs,
bench tools, experiments, and other material that is not part of the public
projection.

## Source Of Truth

- Public base: `origin/main`
- Public projection branch: `public-ultimate-squash`
- Public projection worktree:
  `/opt/m/rust/esc/rm/rm32-public-squash`

## Commands

Run from `/opt/m/rust/esc/rm/rm32-public-squash`:

```sh
git fetch origin --prune
git diff --shortstat origin/main HEAD -- rm32
git diff --name-only origin/main HEAD -- rm32
```

The first command reports the insert/delete tally for public-bound `rm32/`
changes only. The second command reports the file list.

Do not measure from `private/am32_sheet` directly for this purpose. That branch
can contain private-only directories, bench notes, generated data, and local
tools. Those are intentionally filtered out of the public squash projection.

## Current Tally

As of public projection commit `c0fa0e6` on top of public `origin/main`
`4b0e23a`:

```text
11 files changed, 1158 insertions(+), 448 deletions(-)
```

Current `rm32/` file list:

```text
rm32/src/bin/harness.rs
rm32/src/constants.rs
rm32/src/control/input.rs
rm32/src/control/isr_logic.rs
rm32/src/control/shared_impl.rs
rm32/src/control/state.rs
rm32/src/control/tests.rs
rm32/src/main_state.rs
rm32/src/shared_comm.rs
rm32/src/shared_state.rs
rm32/src/transfer.rs
```

## Interpretation

The file count should only change when an entire public-projection file becomes
identical to `origin/main`, or when a new `rm32/` file enters/leaves the public
projection.

Line counts can go up during cleanup if the branch adds tests, moves behavior
into clearer shared plumbing, or replaces compact bench code with reviewable
public code. Line counts are still useful, but they are not a pure "cruft
removed" metric.

When reporting cleanup progress, include both:

- `git diff --shortstat origin/main HEAD -- rm32`
- `git diff --name-only origin/main HEAD -- rm32`

## Notes

- 2026-08-14: bidirectional DShot auto-detect self-validation is already
  public in `origin/main` via PR #66 (`edde774`). The remaining
  bidir-looking delta in `rm32/src/transfer.rs` is not a clean public PR:
  it is mostly comment/test reshaping plus bench diagnostics such as
  exposing `high_pin_count`, and it replaces unrelated servo calibration
  coverage in the projection. Treat this as projection cleanup or deferred
  diagnostics, not as another laundering chunk.
- 2026-08-14: signal-timeout reset behavior is already public in
  `origin/main` via PR #61 (`89b80dc`). The remaining signal-timeout delta
  in the projection is comment/test reshaping around existing behavior, not
  a useful standalone public PR.
- 2026-08-14: zero-throttle idle BEMF/zero-cross cleanup is public via
  PR #88 (`56744b0`). The projection was rebased across it by keeping the
  short public comment and dropping the longer bench-era explanation.
- 2026-08-15: stopped COMP masking is public via PR #89 (`9ff612b`). The
  projection was rebased across it by keeping the concise public tick-side
  comment and preserving the timer-side stopped guard.
- 2026-08-15: harness import cleanup is public via PR #90 (`4b0e23a`). The
  projection was rebased across it by keeping the public module-level imports
  and the explicit ignored BEMF zero-cross return value.
