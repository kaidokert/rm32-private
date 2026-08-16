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

As of public projection commit `4554100` on top of public `origin/main`
`2fc0e34`:

```text
9 files changed, 808 insertions(+), 141 deletions(-)
```

Current `rm32/` file list:

```text
rm32/Cargo.toml
rm32/src/constants.rs
rm32/src/control/isr_logic.rs
rm32/src/control/shared_impl.rs
rm32/src/control/state.rs
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

## Next Candidates

Immediate next action:

- Do a projection cleanup pass before the next public behavior PR. The current
  eight-file `rm32/` delta is no longer full of easy isolated fixes; much of it
  is desync/orbit/reclimb policy that is bench-qualified only on L431.
- Remove or quarantine bench-only observability from the public squash:
  `shared_state.rs` debug counters and `transfer.rs high_pin_count` are useful
  while debugging, but should not leak into public code unless promoted into a
  deliberate, supported observability API.
- Trim narrative comments in `constants.rs`, `main_state.rs`,
  `control/isr_logic.rs`, `shared_comm.rs`, and `shared_state.rs`. Keep short
  comments that explain non-obvious AM32 intent or hardware constraints; drop
  bench forensics, line-number archaeology, "kept divergence" essays, and
  long story blocks.

Best next public PR candidate:

- Prefer a small cleanup around 1 kHz ADC/PID/LVC dispatch if a minimal
  still-private delta remains after the rebase. It is cross-board and
  AM32-intent shaped.
- Otherwise, pause the behavioral PR stream and do projection cleanup first:
  strip narrative comments, delete bench-only diagnostics, and keep the public
  squash close to reviewable code before touching desync/orbit/reclimb.

Avoid for the next PR:

- `main_state.rs` desync/orbit/reclimb constants and policy: still
  board-qualified on L431 only.
- `DutyKickHalf` / `DutyKickDown`: tied to that desync policy.
- `shared_state.rs` bench debug counters and `transfer.rs high_pin_count`:
  useful during bench work, but not a clean public behavior slice unless they
  are explicitly converted into a supported observability API.
- stm32 bench/runtime instrumentation from the bench branch: private/bench
  cruft, not public PR material.
- Comment-only expansions that cite bench forensics or line-number archaeology:
  keep the short reason, drop the story.

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
- 2026-08-15: bench reintegration `287aa53` was folded into the public
  projection selectively: test/projection cleanup from `control/tests.rs` and
  `transfer.rs` was kept, private stm32 bench instrumentation was not, and
  remaining harness comment-only residue was dropped from the squash.
- 2026-08-15: bench reintegration `fac84c6` was folded into the public
  projection selectively. `rm32/src/control/input.rs` now matches public
  `origin/main`, and `rm32/src/transfer.rs` was rebuilt from public main with
  only the remaining `TransferActions.high_pin_count` bench diagnostic delta.
  That diagnostic is still consumed by the public projection's shared/stm32
  debug path, so removing it is a separate cleanup pass rather than a blind
  reset.
- 2026-08-15: startup COMP masking is public via PR #91 (`a79830d`). The
  projection was rebased across it, which removed that one-line deletion from
  the remaining squash.
- 2026-08-15: bench reintegration `1495e79` added `minz/` qualification
  instrumentation only. It does not change the public `rm32/` projection or
  the `rm32/` tally.
- 2026-08-15: commutation timer polling demotion is public via PR #92
  (`2fc0e34`). The projection was rebased across it; stale duplicate
  `commutate_kick_inflates_ci_zcfoundroutine` test coverage was removed from
  the squash, dropping `rm32/src/control/tests.rs` from the remaining `rm32/`
  tally.
- 2026-08-15: bench cleanup from `private/am32_sheet` (`510de4b`) was folded
  into the public projection selectively. The useful part was quarantining
  bench diagnostics behind an explicit `rm32/bench-diag` feature; `rm32`
  itself defaults to no bench diagnostics, while the current `rm32_stm32`
  bench firmware dependency opts in. This intentionally adds `rm32/Cargo.toml`
  to the `rm32/` tally, so the file count is 9 even though the public-default
  surface is cleaner.
