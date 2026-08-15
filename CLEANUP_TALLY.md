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

As of public projection commit `c28e0c7` on top of public `origin/main`
`8da5e48`:

```text
11 files changed, 1258 insertions(+), 478 deletions(-)
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
