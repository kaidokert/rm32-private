"""Falsification tests for `cohort.never_powered` (E337).

This predicate excuses a run from a rung cohort, so it is the most
safety-relevant thing in the fixture: too loose and it launders real protection
trips into "unmeasured", which is indistinguishable from retry-until-pass.

It exists because the operator cutting the bench supply mid-session produced a
capture that `run_gates` judged a firmware latch, and `rung_report` holds a rung
failed on an ELF *permanently* — so a power switch permanently disqualified a
good image at its hardest rung. That happened twice. The first time it was
edited out of `ladder_state.json` by hand, which both qualification reviews
called procedurally wrong while accepting the physics. This states the physics
in the fixture instead, and these tests bound it.

* TEST A — it fires on the one capture it is for.
* TEST B — it excuses **nothing else in the entire corpus**.
* TEST C — it cannot excuse a powered `Reason::Bus` or `FastBusSag` trip, which
  always has a normal reference rail and accepted crossings.
* TEST D — each clause is load-bearing: relaxing any one of them alone must
  still refuse a powered trip.

Run it whenever the predicate or `cohort.parse` changes:

    python scripts/cohort_selftest.py
"""

from __future__ import annotations

import glob
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).parent))

import cohort  # noqa: E402

TARGET = "q600-advref-h2_01.txt"


def main() -> int:
    failed: list[str] = []
    records: list[tuple[str, dict]] = []
    for f in sorted(glob.glob("captures/*/*.txt")) + sorted(glob.glob("captures/*.txt")):
        r = cohort.parse(pathlib.Path(f))
        if r:
            records.append((f, r))
    if not records:
        print("REFUSED: no captures parsed; this test would pass vacuously.")
        return 2
    print(f"parsed {len(records)} captures")

    # TEST A
    hits = [f for f, r in records if cohort.never_powered(r)]
    target_hit = [f for f in hits if TARGET in f]
    ok = len(target_hit) == 1
    if not ok:
        failed.append("A")
    print(f"TEST A  fires on {TARGET}: {'yes' if ok else 'NO'}")

    # TEST B
    others = [f for f in hits if TARGET not in f]
    if others:
        failed.append("B")
    print(f"TEST B  excuses nothing else: {'yes' if not others else 'NO -> ' + str(others[:3])}")

    # TEST C
    powered_trips = [
        (f, r) for f, r in records if r.get("reason") in (6, 26) and r.get("accepted", 0) > 0
    ]
    leaks = [f for f, r in powered_trips if cohort.never_powered(r)]
    if leaks:
        failed.append("C")
    print(
        f"TEST C  refuses all {len(powered_trips)} powered bus/sag trips: "
        f"{'yes' if not leaks else 'NO -> ' + str(leaks[:3])}"
    )

    # TEST D -- each clause alone must not open the gate on a powered trip.
    clauses = ("hold_ms", "accepted", "unstable", "coast_crossings", "bus_ref")
    for clause in clauses:
        opened = 0
        for _f, r in powered_trips:
            probe = dict(r)
            # Neutralise this one clause the way a bug would, leaving the rest.
            probe[clause] = 0 if clause != "bus_ref" else 1
            if cohort.never_powered(probe):
                opened += 1
        if opened:
            failed.append(f"D:{clause}")
        print(f"TEST D  '{clause}' alone does not open the gate: "
              f"{'yes' if not opened else f'NO ({opened} trips excused)'}")

    if failed:
        print()
        print(f"SELF-TEST FAILED ({len(failed)}): {', '.join(failed)}")
        print("A predicate that excuses a real protection trip is worse than no")
        print("predicate, because it looks like a pass.")
        return 1
    print()
    print("self-test OK: the predicate fires once, excuses nothing else, and every")
    print("clause is load-bearing")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
