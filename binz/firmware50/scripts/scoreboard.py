"""Per-rung scoreboard over EVERY emitted counter, ranked by how hard it moves.

The reason this exists (E298/E299). Across a ten-rung walk and a six-rung climb
I built, defended and then dismantled a bus-excursion observer while
`thin_count` — the leading indicator of the campaign's actual blocker — went
from 0 to 4491 per run in the same captures. It is emitted on `BEMFRCOMP`,
beside `spent_max_us` and `late_arms`, fields I read repeatedly for other
reasons.

The lesson was not "look harder". It was that **nothing scored the quantities
the machine was already reporting**, so only a quantity someone had chosen to
theorise about could ever be noticed. This tool scores all of them and sorts by
movement, so a monotone degradation surfaces whether or not anyone suspected it.

It is deliberately dumb: no model, no threshold, no interpretation. It reports
which counters move with duty and how monotonically, and leaves the reading to
a human. A counter that ranks high here is a hypothesis to test, not a finding.

Usage:
    python scripts/scoreboard.py                       # all of today's captures
    python scripts/scoreboard.py --glob 'e296-*.txt' --top 25
    python scripts/scoreboard.py --field thin_count    # one counter, per rung
"""
from __future__ import annotations

import argparse
import collections
import math
import pathlib
import re
import statistics

# Lines whose fields are per-run counters worth scoring. `COASTTIMING` is
# excluded because `iv_us` is a list, and `PREFLIGHT` because it is a fixed
# pre-run assertion rather than a measurement.
SCORED_LINES = (
    "BEMFDONE", "BEMFGATE", "BEMFRATE", "BEMFRCOMP", "BEMFSECTOR", "BEMFPHASE",
    "BEMFREVISIT", "BEMFDRIVEN", "BEMFGUARD", "BEMFCURRENT", "BEMFWITNESS",
    "BEMFCOAST", "BEMFSAG", "BEMFTAIL",
)

# Fields that are the independent variable, a label, or a restatement of one --
# scoring them against duty is circular and would top every ranking.
IGNORE = frozenset({
    "target_duty_tenths", "duty_tenths", "ceiling_tenths", "applied_cap",
    "applied_ccr", "handoff_ehz", "total_ms", "inject", "advance_level",
})


def collect(
    root: pathlib.Path, pattern: str, keep_injected: bool = False
) -> tuple[dict[int, list[dict[str, int]]], list[str]]:
    """Every scored field of every complete, uninjected capture, keyed by rung.

    Returns the rows **and** the injected captures excluded, so the caller
    reports the exclusion instead of silently dropping data.
    """
    out: dict[int, list[dict[str, int]]] = collections.defaultdict(list)
    skipped_injected: list[str] = []
    for p in sorted(root.glob(pattern)):
        t = p.read_text(encoding="utf-8", errors="replace")
        m = re.search(r"\btarget_duty_tenths=(\d+)", t)
        if not m:
            continue
        # A run that never reached its report is not a data point. Requiring
        # BEMFCURRENT is the same completeness test `cohort.parse` applies.
        if "BEMFCURRENT " not in t:
            continue
        # **Injected runs are excluded by default**, and this is not cosmetic.
        # `Inject::Sag` mutates `applied_duty` and `Inject::AverageCurrent`
        # rebuilds the whole accumulator, so an injected capture's duty label,
        # its mA fields and its stop reason all describe the provocation rather
        # than the rung. Without this filter the ranking surfaced
        # `e278-prot500-i_01` -- an `inject=25` run -- as "the rung-600 cohort",
        # and its `Reason::LateArm` stop would have been read as a natural
        # duty-600 failure. One capture, and it would have been the headline.
        inj = re.search(r"\binject=(\d+)", t)
        if inj and inj.group(1) != "0" and not keep_injected:
            skipped_injected.append(p.name)
            continue
        row: dict[str, int] = {}
        for line in t.splitlines():
            line = line.strip()
            head = line.split(" ", 1)[0] if " " in line else ""
            if head not in SCORED_LINES:
                continue
            for k, v in re.findall(r"(\w+)=(-?\d+)\b", line):
                if k in IGNORE:
                    continue
                # Same-named fields on different lines are a known hazard
                # (`cohort.AMBIGUOUS_KEYS`): qualify every key by its line so
                # `BEMFGATE zc_per_s` and `BEMFRATE zc_per_s` stay distinct.
                row[f"{head}.{k}"] = int(v)
        if row:
            out[int(m.group(1))].append(row)
    return out, skipped_injected


def spearman(xs: list[float], ys: list[float]) -> float:
    """Rank correlation: monotonicity without assuming a shape.

    Pearson would miss a counter that explodes super-linearly, which is exactly
    the shape `thin_count` has (0 -> 4491 over eight rungs).
    """
    def ranks(v: list[float]) -> list[float]:
        order = sorted(range(len(v)), key=lambda i: v[i])
        r = [0.0] * len(v)
        i = 0
        while i < len(order):
            j = i
            while j + 1 < len(order) and v[order[j + 1]] == v[order[i]]:
                j += 1
            avg = (i + j) / 2 + 1
            for k in range(i, j + 1):
                r[order[k]] = avg
            i = j + 1
        return r
    rx, ry = ranks(xs), ranks(ys)
    mx, my = statistics.mean(rx), statistics.mean(ry)
    num = sum((a - mx) * (b - my) for a, b in zip(rx, ry))
    den = math.sqrt(sum((a - mx) ** 2 for a in rx) * sum((b - my) ** 2 for b in ry))
    return num / den if den else 0.0


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--root", default="captures/2026-09-24")
    ap.add_argument("--glob", default="e2*-*.txt")
    ap.add_argument("--top", type=int, default=20)
    ap.add_argument("--field", help="print one counter per rung instead of the ranking")
    ap.add_argument("--min-rungs", type=int, default=4)
    ap.add_argument("--keep-injected", action="store_true",
                    help="include inject!=0 runs; their duty label, mA fields and "
                         "stop reason describe the provocation, not the rung")
    args = ap.parse_args()

    by_rung, injected = collect(pathlib.Path(args.root), args.glob, args.keep_injected)
    if len(by_rung) < args.min_rungs:
        print(f"only {len(by_rung)} rung(s) matched {args.glob}; need {args.min_rungs}")
        return 2
    rungs = sorted(by_rung)
    n = sum(len(v) for v in by_rung.values())
    print(f"{n} complete captures over {len(rungs)} rungs "
          f"({rungs[0]}..{rungs[-1]}) matching {args.glob}")
    if injected:
        print(f"EXCLUDED {len(injected)} injected capture(s): "
              + ", ".join(sorted(injected)[:6])
              + (" ..." if len(injected) > 6 else ""))
    print()

    if args.field:
        key = next((k for k in {k for v in by_rung.values() for r in v for k in r}
                    if k == args.field or k.endswith("." + args.field)), None)
        if not key:
            print(f"no such counter: {args.field}")
            return 2
        print(f"{key}\n")
        print(f'{"rung":>5}{"n":>3}  values')
        for d in rungs:
            vals = [r[key] for r in by_rung[d] if key in r]
            print(f"{d:>5}{len(vals):>3}  {vals}")
        return 0

    # Rank every counter by how monotonically its rung-median moves with duty.
    keys = sorted({k for v in by_rung.values() for r in v for k in r})
    scored = []
    for key in keys:
        med = {}
        for d in rungs:
            vals = [r[key] for r in by_rung[d] if key in r]
            if vals:
                med[d] = statistics.median(vals)
        if len(med) < args.min_rungs:
            continue
        xs = list(med)
        ys = [med[d] for d in xs]
        if len(set(ys)) == 1:
            continue  # constant: nothing to say, and it would rank as 0
        rho = spearman([float(x) for x in xs], ys)
        lo, hi = ys[0], ys[-1]
        # Fold change across the span, guarding a zero start.
        fold = (hi / lo) if lo else float("inf") if hi else 1.0
        scored.append((abs(rho), rho, fold, key, lo, hi, len(med)))

    scored.sort(reverse=True)
    print("counters ranked by |rank correlation| with duty, then fold change.")
    print("A high rank is a HYPOTHESIS TO TEST, not a finding -- many of these")
    print("move with duty for uninteresting reasons (interval scaling, counts")
    print("over a fixed window). The point is that nothing is invisible.\n")
    print(f'{"rho":>7}{"fold":>11}{"first":>12}{"last":>12}{"rungs":>7}  counter')
    for _, rho, fold, key, lo, hi, m in scored[:args.top]:
        f = "inf" if fold == float("inf") else f"{fold:.1f}x"
        print(f"{rho:>+7.3f}{f:>11}{lo:>12.0f}{hi:>12.0f}{m:>7}  {key}")
    print(f"\n({len(scored)} non-constant counters scored; showing {min(args.top, len(scored))})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
