#!/usr/bin/env python3
"""Read a `chain-capture` dump and report the commutation timing chain.

The firmware writes one line per event (`src/chain.rs`):

    CHAINSNAP len=<n> total=<n>      (the accept ring: COMP's rows)
    CHAIN 1 <crossing_us> <crossing_fine> <arm_fine> <wait_us> <spent_fine> <step> <flag>
    ...
    CHAINSNAP len=<n> total=<n>      (the service ring: COM's rows)
    CHAIN 2 <sched_us> <fire_fine> <bridge_fine> <late_us> <0> <step> <flag>
    ...
    CHAINEND

**The fine columns are 64 MHz ticks (15.6 ns).** Until E180 every stamp was
1 µs, which quantised the crossing-to-bridge delay (17-20 µs) and the chain
term inside it (4 µs) in steps the size of the effect -- two A/B comparisons
turned on differences of exactly one tick. The coarse µs stamp is kept for
pairing, where the control path's own µs quantisation is the right unit.

Two rings, one per root, because no `Seam` may be touched by two roots -- which
is what lets the recording image run with COM above COMP (step 6a). They are
merged here by their stamps, which are on the same 1 MHz clock.

with, for kind 1 (an acceptance's arm): a = the accepted crossing's entry
stamp, b = the instant the one-shot was armed, c = the wait asked for, d = what
the handler had spent at the arm, flag bit 7 = the wait was already exhausted,
flag bits 0-2 = `roots::stage_code` (bit 0 driven observer, bit 1 closed-loop
detector, bit 2 guard tracking);

and for kind 2 (a COM service): a = when the handler ran, b = when the bridge
was actually updated (0 for phases 2 and 3, which change no gates), c = the
instant it was scheduled for, d = the lateness the firmware would report, flag
= the timer's **purpose** in the low seven bits (1 commutation, 2 reverse-blank
end, 3 blanking floor) **and bit 7 set when this service preempted a
zero-crossing decision** (E170; only a `com-top` recording image ever sets
it).

Every stamp is the 16-bit TIM17 count at 1 MHz, so differences are µs and wrap
every 65.536 ms; this joins them with `wrapping_sub` semantics.

What it computes, and nothing else:

* **effective angle = (commutation instant − crossing instant) / interval**,
  where the commutation instant is the phase-1 bridge update and the interval
  is the gap between consecutive accepted crossings. This is the one quantity
  the campaign compares across images: nominal advance *level* is not
  comparable between images whose latency chains differ (E145's trap).
* the same lateness the firmware reports, **split by purpose**, which is what
  `com_late_max_us` cannot do (E153).
* late arms, with the stage and the wait/elapsed that produced them.

Ramp and hold are not distinguished by the firmware -- no root sees the
commanded duty -- so any such split here is a host classification by position
in the run, and is labelled as one.

Usage:  python scripts/chain.py <capture.log> [--csv rows.csv]
"""
import argparse
import collections
import statistics
import sys

Row = collections.namedtuple('Row', 'kind at_us at_fine x_fine y_us z_fine step flag')

# The fine clock is TIM2 free-running at the 64 MHz core clock (E180), so one
# tick is 15.625 ns and a µs is exactly 64 ticks. Every *measured* delta in
# this script is fine; `y_us` (the requested wait, and the lateness the
# firmware reports) stays µs, because those are control quantities and are
# genuinely µs-quantised.
TICKS_PER_US = 64.0


def advance_of(ci, level):
    """`src/commutation.rs::advance_of`, mirrored exactly (shift-only split)."""
    level = min(level, 64)
    return (ci >> 6) * level + (((ci & 63) * level) >> 6)


def wait_time(ci, level):
    """`src/commutation.rs::wait_time`: (ci >> 1) - advance, saturating."""
    return max((ci >> 1) - advance_of(ci, level), 0)


def u16(a, b):
    """b - a as a forward 16-bit interval, in whatever unit a and b are."""
    return (b - a) & 0xFFFF


def us(ticks):
    """Fine ticks to µs."""
    return ticks / TICKS_PER_US


def align(accepts, services, span=6):
    """Pair each phase-1 service with the acceptance that scheduled it.

    The two rings are per-root (step 6a) and each keeps its own last
    `CHAIN_LEN` events, so they do **not** cover the same span: the service
    ring holds commutations *and* blanking-floor services, so 1024 of its rows
    reach back only about half as far as 1024 acceptances. They cannot be
    merged by raw 16-bit stamps either -- the rings' wrap phases differ, and a
    naive stamp merge paired rows half a sector apart (it produced a 45 µs
    "chain" and a 0.61 effective angle, which is how this was caught).

    What is true is that both rings stop at the same instant, the disarm. So
    the tails align: the last n acceptances go with the last n commutations.
    A small search around that refines it, scored on the key that ties the two
    rows together -- a service carries the instant it was **scheduled for** and
    an acceptance carries its stamp and the wait it asked for, so
    `sched ≈ crossing + wait`.
    """
    svc = [r for r in services if r.flag & 0x7F == 1 and r.x_fine != 0]
    if not accepts or not svc:
        return []
    n = min(len(accepts), len(svc))
    base_a, base_s = accepts[-n:], svc[-n:]

    def score(shift):
        errs = []
        for k in range(n):
            m = k + shift
            if 0 <= m < len(base_a):
                d = u16(base_a[m].at_us + base_a[m].y_us, base_s[k].at_us)
                errs.append(min(d, 0x10000 - d))
        return statistics.median(errs) if len(errs) >= n // 2 else None

    best, best_err = 0, score(0)
    for shift in range(-span, span + 1):
        err = score(shift)
        if err is not None and (best_err is None or err < best_err):
            best, best_err = shift, err
    out = []
    for k in range(n):
        m = k + best
        if 0 <= m < len(base_a):
            out.append((base_a[m], base_s[k]))
    return out, best, best_err if best_err is not None else -1.0


def parse(path):
    rows, snap = [], {}
    level = None
    with open(path, 'r', errors='replace') as fh:
        for line in fh:
            line = line.strip()
            if line.startswith('CHAINSNAP'):
                for kv in line.split()[1:]:
                    if '=' in kv:
                        k, v = kv.split('=', 1)
                        snap[k] = int(v)
            elif line.startswith('CHAIN '):
                f = line.split()
                if len(f) == 8:
                    rows.append(Row(*(int(x) for x in f[1:])))
            elif line.startswith('BEMFRUN '):
                for kv in line.split()[1:]:
                    if kv.startswith('advance_level='):
                        level = int(kv.split('=', 1)[1])
    return snap, rows, level


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('log')
    ap.add_argument('--csv')
    a = ap.parse_args()
    snap, rows, level_run = parse(a.log)
    if not rows:
        print('no CHAIN rows found', file=sys.stderr)
        return 1
    dropped = snap.get('total', len(rows)) - snap.get('len', len(rows))
    print(f'rows={len(rows)} kept={snap.get("len")} total={snap.get("total")} overwritten={max(dropped, 0)}')

    accepts = [r for r in rows if r.kind == 1]
    services = [r for r in rows if r.kind == 2]

    # Lateness by purpose: what the firmware's single counter mixes.
    print('\nCOM service lateness, µs, by the timer\'s purpose')
    names = {1: 'commutation', 2: 'reverse-blank end', 3: 'blanking floor'}
    for phase in sorted({s.flag & 0x7F for s in services}):
        late = [s.y_us for s in services if s.flag & 0x7F == phase]
        late = [x for x in late if x < 0x8000]
        if not late:
            continue
        print(
            f'  phase {phase} ({names.get(phase, "?"):18s}) n={len(late):6d} '
            f'max={max(late):5d} p50={statistics.median(late):7.1f} mean={statistics.fmean(late):7.2f}'
        )

    # Handler-run to bridge-update, phase 1 only: what the lateness counter
    # leaves out entirely.
    to_bridge = [us(u16(s.at_fine, s.x_fine)) for s in services if s.flag & 0x7F == 1 and s.x_fine != 0]
    if to_bridge:
        print(
            f'\nhandler entry -> bridge update, µs: n={len(to_bridge)} '
            f'max={max(to_bridge):.3f} p50={statistics.median(to_bridge):.3f} mean={statistics.fmean(to_bridge):.3f}'
        )

    # Effective angle, from aligned (acceptance, commutation) pairs and the
    # interval between consecutive acceptances.
    aligned = align(accepts, services)
    pairs = []
    if aligned:
        matched, offset, align_err = aligned
        print(
            f'\npairing: {len(matched)} (acceptance, commutation) pairs, '
            f'index offset {offset:+d}, median sched disagreement {align_err:.1f} us'
        )
        prev = None
        for acc, svc_row in matched:
            if prev is not None:
                # Both in fine ticks: an 80-190 µs sector is 5 100-12 200
                # ticks, far inside the u16 window.
                interval = u16(prev, acc.at_fine)
                if 0 < interval < 0xC000:
                    pairs.append((u16(acc.at_fine, svc_row.x_fine), interval, acc, svc_row))
            prev = acc.at_fine

    # Per-event columns, because an aggregate hides what the rows carry: the
    # requested wait and the elapsed-at-arm are in every kind-1 row (the
    # independent review of E154 asked for these instead of report means).
    if accepts:
        # The wait is a control quantity and stays in whole microseconds;
        # the elapsed-at-arm is a measurement, so it is fine now.
        waits = [r.y_us for r in accepts]
        spent = [us(r.z_fine) for r in accepts]
        print(
            f'\nrequested wait, µs (per event): n={len(waits)} min={min(waits)} p50={statistics.median(waits):.1f} '
            f'max={max(waits)} sd={statistics.stdev(waits) if len(waits) > 1 else 0:.2f}'
        )
        print(
            f'elapsed at the arm, µs:         n={len(spent)} min={min(spent):.3f} p50={statistics.median(spent):.3f} '
            f'max={max(spent):.3f} sd={statistics.stdev(spent) if len(spent) > 1 else 0:.3f}'
        )

    if pairs:
        ang = [d / i for d, i, _, _ in pairs]
        # The angle is a ratio, so say which side its spread comes from: the
        # crossing-to-bridge delay, or the sector interval under it.
        # Both sides of the ratio are fine ticks, printed as microseconds.
        # Three decimals because the stamp is finer than that now (15.6 ns),
        # which is the point of E180: a 17-20 us delay and the 4 us chain
        # term inside it are no longer reported in 1 us steps.
        num = [us(d) for d, _, _, _ in pairs]
        den = [us(i) for _, i, _, _ in pairs]
        print(
            f'\ncrossing -> bridge update, µs: n={len(num)} min={min(num):.3f} p50={statistics.median(num):.3f} '
            f'max={max(num):.3f} sd={statistics.stdev(num) if len(num) > 1 else 0:.3f}'
        )
        print(
            f'sector interval, µs:          n={len(den)} min={min(den):.2f} p50={statistics.median(den):.2f} '
            f'max={max(den):.2f} sd={statistics.stdev(den) if len(den) > 1 else 0:.2f}'
        )
        print(f'\neffective angle = (bridge update - crossing) / interval, n={len(ang)}')
        sd = f'  sd={statistics.stdev(ang):.4f}' if len(ang) > 1 else ''
        print(
            f'  p50={statistics.median(ang):.4f}  mean={statistics.fmean(ang):.4f}  '
            f'min={min(ang):.4f}  max={max(ang):.4f}{sd}'
        )
        print(f'  in electrical degrees of the 60 deg sector: p50={statistics.median(ang) * 60:.2f}')

    # The same angle against the *forward* interval -- the sector the
    # commutation falls into -- rather than the backward one, so the pairing
    # choice is shown instead of assumed (review of E154 asked for it).
    fwd, acc, svc = [], None, None
    for r in rows:
        if r.kind == 1:
            if acc is not None and svc is not None:
                interval = u16(acc.at_fine, r.at_fine)
                if interval:
                    fwd.append(u16(acc.at_fine, svc.x_fine) / interval)
            acc, svc = r, None
        elif r.kind == 2 and r.flag == 1 and r.x_fine != 0 and acc is not None and svc is None:
            svc = r
    if fwd:
        print(
            f'  against the forward interval instead: p50={statistics.median(fwd):.4f} '
            f'sd={statistics.stdev(fwd) if len(fwd) > 1 else 0:.4f} n={len(fwd)}'
        )

    # The angle the firmware actually *scheduled*, from the wait in each row --
    # a measurement, not a model. Then the same quantity for other advance
    # levels, which has to be modelled, with the model's error stated.
    #
    # The model's input is not what the firmware uses: `wait_time` is applied to
    # the estimator's blended, clamped `average_interval` (`src/bemf.rs`), while
    # this has only the raw consecutive-crossing interval. The medians agree but
    # individual events do not, so the per-level columns below are a
    # distribution of a model, not of the firmware -- the match rate against the
    # row's own wait says by how much. (The pre-run review of E159 found this;
    # the earlier version of this table claimed per-event fidelity it lacks.)
    if pairs:
        # The wait is microseconds, so the interval goes in as microseconds.
        sched_meas = [acc.y_us / us(i) for _, i, acc, _ in pairs]
        print(
            f'\nscheduled angle, from each row\'s own requested wait: '
            f'p50={statistics.median(sched_meas):.4f} = {statistics.median(sched_meas) * 60:.2f} deg'
        )
        measured = statistics.median([d / i for d, i, _, _ in pairs])
        print(f'measured effective angle:  p50={measured:.4f} = {measured * 60:.2f} deg')
        # Per-event chain, not a difference of medians.
        chain = [(us(d) - acc.y_us) for d, _, acc, _ in pairs]
        print(
            f'chain per event (bridge - crossing - requested wait), µs: '
            f'p50={statistics.median(chain):.3f} min={min(chain):.3f} max={max(chain):.3f} '
            f'sd={statistics.stdev(chain) if len(chain) > 1 else 0:.3f}'
        )
        if level_run is not None:
            # `wait_time` mirrors the firmware's integer arithmetic, so the
            # interval enters it as whole microseconds.
            i_us = [int(round(us(i))) for _, i, _, _ in pairs]
            hit = sum(1 for k, (_, _, acc, _) in enumerate(pairs)
                      if wait_time(i_us[k], level_run) == acc.y_us)
            err = [wait_time(i_us[k], level_run) - acc.y_us
                   for k, (_, _, acc, _) in enumerate(pairs)]
            pct = 100.0 * hit / len(pairs)
            print(
                f"model check at the run's own level {level_run}: {pct:.1f}% of events match "
                f"the row's own wait, error {min(err)}..{max(err)} us -- usable in the median, "
                f"not per event"
            )
        print('modelled scheduled angle by advance level (raw interval, see caveat):')
        for level in (20, 22, 24, 26):
            sched = [wait_time(int(round(us(i))), level) / us(i) for _, i, _, _ in pairs]
            mark = "  <- the run's level" if level == level_run else ""
            print(f'  level {level:2d}: p50={statistics.median(sched):.4f} = {statistics.median(sched) * 60:5.2f} deg{mark}')

    print()
    print("preempted vs not, from the service rows' own marker:")
    # Preempted commutations against the rest, from the rows' own marker: the
    # step-6 review's point that a tail of unlabelled events cannot show
    # whether a preemption ever landed badly (E170).
    marked = [(d, i, acc, svc) for d, i, acc, svc in pairs if svc.flag & 0x80]
    clean = [(d, i, acc, svc) for d, i, acc, svc in pairs if not svc.flag & 0x80]
    if marked:
        for name, group in (("preempted", marked), ("not preempted", clean)):
            if not group:
                continue
            ch = [us(d) - acc.y_us for d, _, acc, _ in group]
            ang = [d / i for d, i, _, _ in group]
            print(
                f'  {name:14s} n={len(group):4d}  chain p50={statistics.median(ch):.3f} '
                f'min={min(ch):.3f} max={max(ch):.3f}  angle p50={statistics.median(ang):.4f}'
            )
    elif pairs:
        print('  no service row carries the preempt marker (peer image, or no preemption sampled)')

    late_arms = [r for r in accepts if r.flag & 0x80]
    print(f'\nlate arms: {len(late_arms)} of {len(accepts)} acceptances')
    for r in late_arms[:20]:
        print(f'  step={r.step} wait={r.y_us}us spent={us(r.z_fine):.3f}us stage={r.flag & 0x07:#05b}')

    if a.csv:
        with open(a.csv, 'w', newline='') as fh:
            fh.write('kind,at_us,at_fine,x_fine,y_us,z_fine,step,flag\n')
            for r in rows:
                fh.write(','.join(str(x) for x in r) + '\n')
        print(f'\nrows written to {a.csv}')
    return 0


if __name__ == '__main__':
    sys.exit(main())
