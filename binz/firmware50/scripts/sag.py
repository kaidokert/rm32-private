#!/usr/bin/env python3
"""Read a `sag-capture` dump and report the sharp-sag guard's own margin.

The firmware records one row per judgement -- the guard judges **every scan**,
at the measured 9.8 kHz, with a sliding 8-scan mean, not a decimated block --
in two rings (`src/sagtrace.rs`):

    SAGSNAP judged=<n> fast_len=<n> slow_len=<n> slow_every=<n> frozen=<0|1>
            num=95 den=100 streak_to_latch=3 adc_rail=4095
    SAGROW  <at> <bus_mean> <vref_mean> <filt_bus> <filt_vref> <streak> <step> <duty> <since_zc_us>
    ...
    SAGSLOW <bus_mean> <vref_mean> <filt_bus> <filt_vref>      (every slow_every-th)
    ...
    SAGEND

Raw quantities only -- the firmware does no division -- so the guard's own
comparison is reproduced here exactly as it runs:

    low  <=>  bus_mean * filt_vref * den  <  filt_bus * vref_mean * num

and the margin is reported as the ratio of the two sides in per mille, where
1000 is exactly on the line and larger is safer:

    margin = 1000 * (bus_mean * filt_vref * den) / (filt_bus * vref_mean * num)

**What this is not.** `bus_min` in the run report is the minimum of a *single
raw scan* over the whole run, and the report's `filt_bus` is the filter's value
at the *end* of the run. Neither is what the guard compares, and both have been
quoted at it before (campaign 8). This script only uses the quartet the guard
itself used, as recorded at the instant it judged.

Coverage, which bounds every conclusion drawn here:

* the fast ring is 512 judgements ~= **52 ms**, and for a run that does not
  trip it is the **tail** -- it says nothing about the ramp, the first 207 ms
  (when the reference is still the unloaded rail), or any excursion earlier in
  the hold;
* the decimated ring is 1024 rows every 32nd judgement ~= **1.7 s**, eight of
  the reference's time constants, which is what showing *how the reference got
  there* requires;
* both **freeze on the fault**, so a frozen dump is the pre-trip window;
* `frozen=0` does **not** mean the run was healthy -- a stop for any other
  reason returns before the guard is judged. Read `run_reason` (this script
  lifts it from `BEMFDONE`).

The guard is also a **band-pass**: an 8-scan mean (~808 us) in the numerator
and a 207 ms average in the denominator, so it cannot see a dip faster than
about a millisecond or slower than a fifth of a second. That is a property of
the guard, not of this script.

Usage:
    python scripts/sag.py <capture.txt> [--csv rows.csv] [--worst 20]
"""
from __future__ import annotations

import argparse
import collections
import pathlib
import statistics
import sys

# v2 (campaign 11) added `at_fine`, `bus_raw` and the three phase codes.
# `bus_raw` is the field that matters most: `bus` below is the guard's 8-tap
# sliding mean, so it widens every event by 7 scans and cannot state a dip's
# true width. v1 captures are read with those fields as None.
Row = collections.namedtuple(
    "Row",
    "at at_fine bus_raw phase_a phase_b phase_c bus vref filt_bus filt_vref streak step duty since_zc",
)
Slow = collections.namedtuple("Slow", "bus vref filt_bus filt_vref")

# Field counts including the leading "SAGROW" token.
V1_FIELDS = 10
V2_FIELDS = 15


def _row_from(f: list[str]) -> Row:
    """One row, whichever format version it is. Refuses anything else."""
    v = [int(x) for x in f[1:]]
    if len(f) == V2_FIELDS:
        return Row(*v)
    if len(f) == V1_FIELDS:
        at, bus, vref, fb, fv, streak, step, duty, zc = v
        return Row(at, None, None, None, None, None, bus, vref, fb, fv, streak, step, duty, zc)
    raise SystemExit(
        f"REFUSED: a SAGROW with {len(f) - 1} fields is neither v1 ({V1_FIELDS - 1}) "
        f"nor v2 ({V2_FIELDS - 1}). Silently skipping it would report an empty ring "
        f"as a clean one.\n  {' '.join(f)}"
    )


def parse(path: pathlib.Path) -> tuple[dict, list[Row], list[Slow]]:
    snap, rows, slow = {}, [], []
    for line in path.read_text(errors="replace").splitlines():
        line = line.strip()
        if line.startswith("SAGSNAP"):
            for kv in line.split()[1:]:
                if "=" in kv:
                    k, v = kv.split("=", 1)
                    snap[k] = int(v)
        elif line.startswith("SAGROW "):
            rows.append(_row_from(line.split()))
        elif line.startswith("SAGSLOW "):
            f = line.split()
            if len(f) == 5:
                slow.append(Slow(*(int(x) for x in f[1:])))
        elif line.startswith("BEMFDONE "):
            for kv in line.split()[1:]:
                if kv.startswith("reason="):
                    snap["run_reason"] = int(kv.split("=", 1)[1])
    return snap, rows, slow


def check_units(snap: dict, rows: list[Row]) -> None:
    """Refuse a capture whose tick rate is not declared.

    **The fine stamp's rate is part of the format, not commentary.** It changed
    from 15.625 ns to 125 ns between campaigns, so reading a v2 delta with the
    v1 rate under-reports every interval by 8x. `scripts/chain.py` already
    refuses legacy captures for exactly this reason; assuming a rate here would
    reintroduce the trap one file over.
    """
    has_fine = any(r.at_fine is not None for r in rows)
    if not has_fine:
        return  # a v1 capture, which carries no fine stamps to misread
    if "fine_hz" not in snap:
        raise SystemExit(
            "REFUSED: rows carry a fine stamp but SAGSNAP does not declare "
            "`fine_hz`. The tick changed 15.625 ns -> 125 ns between campaigns, "
            "so a delta read at the wrong rate is wrong by 8x. Re-dump with an "
            "image that emits the units."
        )
    if "span16_us" not in snap:
        raise SystemExit("REFUSED: SAGSNAP declares `fine_hz` but not `span16_us`.")


def fine_delta_us(a: Row, b: Row, snap: dict) -> float | None:
    """Wrap-safe µs between two rows on the fine clock, or None if it aliases.

    The stamp keeps only 16 bits, so a gap at or beyond `span16_us` is
    indistinguishable from a short one. Returning None rather than a number is
    the whole point: an aliased pairing must not be reported as a measurement.
    """
    if a.at_fine is None or b.at_fine is None:
        return None
    hz = snap.get("fine_hz")
    if not hz:
        return None
    ticks = (b.at_fine - a.at_fine) & 0xFFFF
    us = ticks * 1_000_000.0 / hz
    return None if us >= snap.get("span16_us", 0) else us


def raw_run_width(rows: list[Row], threshold: int) -> tuple[int, int]:
    """Longest run of consecutive rows whose **raw** bus is below `threshold`,
    and the count of such rows.

    This is the quantity a width claim needs and v1 could not supply. The
    recorded *mean* is an 8-tap boxcar, so it reports `m + 7` scans for any raw
    notch of `m` -- i.e. >= 814 µs for even a single-scan spike, which is why a
    width prediction against the mean cannot fail.
    """
    best = run = n = 0
    for r in rows:
        if r.bus_raw is not None and r.bus_raw < threshold:
            run += 1
            n += 1
            best = max(best, run)
        else:
            run = 0
    return best, n


def fail_closed(r: Row, rail: int) -> bool:
    """The guard latches at once on an implausible VREF, whatever the ratio
    says (`protection.rs`). Reproduced here because a script that only models
    the ratio can report "no low blocks" on the very row that tripped."""
    return r.vref == 0 or r.vref >= rail


def margin_permille(r: Row, num: int, den: int) -> float:
    """The guard's own cross-product, as a ratio; 1000.0 is exactly the line.

    `None` for a fail-closed row: there is no meaningful margin when the
    normalisation itself is rejected.
    """
    lhs = r.bus * r.filt_vref * den
    rhs = r.filt_bus * r.vref * num
    return 1000.0 * lhs / rhs if rhs else float("inf")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("capture")
    ap.add_argument("--csv")
    ap.add_argument("--worst", type=int, default=10, help="how many tightest blocks to list")
    a = ap.parse_args()
    snap, rows, slow = parse(pathlib.Path(a.capture))
    # Before any arithmetic: refuse a capture whose tick rate is undeclared.
    check_units(snap, rows)
    if not rows:
        print("no SAGROW rows found", file=sys.stderr)
        return 1
    num = snap.get("num", 95)
    den = snap.get("den", 100)
    latch = snap.get("streak_to_latch", 3)
    rail = snap.get("adc_rail", 4095)
    judged = snap.get("judged", len(rows))
    print(
        f'judgements={judged} fast_kept={snap.get("fast_len")} slow_kept={snap.get("slow_len")} '
        f'frozen={snap.get("frozen")} fraction={num}/{den} streak_to_latch={latch} '
        f'run_reason={snap.get("run_reason", "?")}'
    )
    # What the ring threw away, which no earlier version printed
    # (E181 SS7.2): a fast ring of 512 over a run of hundreds of thousands
    # of judgements has overwritten nearly all of them, and that number
    # belongs on the page rather than being inferred from a percentage.
    fast_len = snap.get("fast_len") or len(rows)
    if judged:
        print(f"  overwritten (fast): {max(judged - fast_len, 0)} judgements never kept")
    if not snap.get("frozen"):
        print(
            "  NOT frozen: either the run never tripped the sag guard, or it stopped for "
            "another reason before the guard was judged -- read run_reason, not this flag"
        )
    # The judgement period, **measured from the fast ring's own 16-bit
    # stamps** rather than assumed. The earlier code carried a literal
    # 101 us -- the same class of stale constant this script exists to
    # catch (E181 SS1.3). `None` when the ring is too short to measure one.
    # Per-row forward deltas, not end-minus-start: 512 judgements at ~101 us
    # span 51.7 ms and the stamp is 16-bit, so the total *wraps* and a naive
    # difference is meaningless (caught by running this on the 47.5% capture).
    # Each step is ~101 us, far inside the modulus; the median of them is the
    # period and the sum is the window.
    steps = [(b.at - a_.at) & 0xFFFF for a_, b in zip(rows, rows[1:])]
    steps = [d for d in steps if 0 < d < 0x8000]
    period_us = statistics.median(steps) if steps else None
    window_ms = sum(steps) / 1000.0 if steps else None
    if judged and rows:
        pct = 100.0 * len(rows) / judged
        extra = f", one every {period_us:.2f} us measured" if period_us else ""
        print(f"  fast window is {len(rows)} of {judged} judgements = {pct:.3f}% of the run, and it is the tail{extra}")
        if period_us and window_ms:
            print(
                f"  fast window spans {window_ms:.1f} ms (summed from its own stamps)"
                f" of a run of {judged * period_us / 1000.0:.0f} ms"
            )

    m = [margin_permille(r, num, den) for r in rows]
    low = [r for r, x in zip(rows, m) if x < 1000.0]
    print(
        f"margin per mille (1000 = exactly on the line): min={min(m):.1f} "
        f"p01={statistics.quantiles(m, n=100)[0]:.1f} p50={statistics.median(m):.1f} "
        f"max={max(m):.1f}"
    )
    print(f"low blocks (the guard's own test): {len(low)} of {len(rows)}")
    streaks = [r.streak for r in rows]
    print(f"streak held: max={max(streaks)} (latches at {latch}); rows with streak>0: {sum(1 for s in streaks if s)}")

    # Fail-closed rows, and the cross-check against the guard's own streak:
    # if the recomputed lows and the recorded streak disagree, the host and the
    # firmware have diverged and nothing else here is trustworthy.
    # **Printed even when zero** (E186 SS4). The fail-closed latch is the
    # only mechanism genuinely distinguishable from the streak latch, and it
    # *forges* the streak on its way out (`protection.rs` sets
    # `lows = SAG_STREAK` before returning), so a fail-closed trip dumps a
    # row with streak=3 and a perfectly normal margin. Reporting its absence
    # by silence made the one real discriminator invisible.
    fc = [r for r in rows if fail_closed(r, rail)]
    print(f"fail-closed rows (vref 0 or >= {rail}): {len(fc)} -- these latch whatever the ratio says")
    # On a frozen ring the last row is the deciding judgement, so name its
    # numbers outright instead of leaving them to the --worst table.
    if rows:
        last = rows[-1]
        ratio = (last.filt_bus / last.bus) if last.bus else 0.0
        need = den / num - 1.0
        print(
            f"  last row: bus_mean={last.bus} vref_mean={last.vref} "
            f"filt_bus={last.filt_bus} filt_vref={last.filt_vref} "
            f"streak={last.streak} margin={margin_permille(last, num, den):.1f}"
        )
        print(
            f"  reference above the bus by {100.0 * (ratio - 1.0):+.2f}%"
            f" (a streak latch needs >= +{100.0 * need:.2f}%);"
            f" fail_closed={fail_closed(last, rail)}"
        )
    disagree = 0
    for i, r in enumerate(rows[1:], start=1):
        # A row whose streak rose must have been low; one whose streak is 0
        # must not have been (a healthy judgement zeroes it).
        rose = r.streak > rows[i - 1].streak
        was_low = m[i] < 1000.0 or fail_closed(r, rail)
        if rose != (was_low and r.streak != 0):
            disagree += 1
    print(f"host/firmware disagreement on lows: {disagree} of {max(len(rows) - 1, 1)} rows")

    order = sorted(range(len(rows)), key=lambda i: m[i])
    print()
    print(f"tightest {min(a.worst, len(rows))} judgements:")
    print(
        f'{"margin":>9} {"bus":>6} {"vref":>6} {"filt_bus":>9} {"filt_vref":>9} '
        f'{"streak":>7} {"step":>5} {"duty":>6}'
    )
    for i in order[: a.worst]:
        r = rows[i]
        print(
            f"{m[i]:9.1f} {r.bus:6d} {r.vref:6d} {r.filt_bus:9d} {r.filt_vref:9d} "
            f"{r.streak:7d} {r.step:5d} {r.duty:6d}"
        )
    # `since_zc_us` is deliberately NOT summarised: judgements come every
    # ~101 us and a sector at 47.5% is ~84 us, so the field is aliased and any
    # distribution of it measures the aliasing, not the phase (review of
    # E175). It stays in the CSV for anyone who wants to show that.

    # The decimated history: how the reference got where it was. This is the
    # half of the record the fast ring cannot show, because 52 ms is a quarter
    # of one 207 ms filter time constant.
    if slow:
        # Derived from the measured judgement period above. `SAGSLOW` rows
        # carry no stamp of their own, so this is the fast ring's rate
        # projected over the decimation -- labelled as such, not offered as
        # a measurement of the slow ring (E181 SS1.3).
        per = period_us if period_us else 101.0
        how = "fast-ring rate" if period_us else "ASSUMED 101 us/judgement"
        span_ms = len(slow) * snap.get("slow_every", 32) * per / 1000.0
        fb = [r.filt_bus for r in slow]
        bus = [r.bus for r in slow]
        print()
        print(
            f"decimated history: {len(slow)} rows every {snap.get('slow_every')} judgements "
            f"~= {span_ms:.0f} ms ({how})"
        )
        print(
            f"  filt_bus {min(fb)}..{max(fb)} (last {fb[-1]}), "
            f"bus_mean {min(bus)}..{max(bus)} (last {bus[-1]})"
        )
        drift = [margin_permille(Row(0, r.bus, r.vref, r.filt_bus, r.filt_vref, 0, 0, 0, 0), num, den) for r in slow]
        print(f"  margin over that history: min={min(drift):.1f} p50={statistics.median(drift):.1f} max={max(drift):.1f}")

    if a.csv:
        with open(a.csv, "w", newline="") as fh:
            fh.write("at,bus_mean,vref_mean,filt_bus,filt_vref,streak,step,duty_tenths,since_zc_us,margin_permille\n")
            for r, x in zip(rows, m):
                fh.write(f"{r.at},{r.bus},{r.vref},{r.filt_bus},{r.filt_vref},{r.streak},{r.step},{r.duty},{r.since_zc},{x:.2f}\n")
        print(f"\nrows written to {a.csv}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
