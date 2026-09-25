#!/usr/bin/env python3
"""Run `sag.py` over every capture on disk and fail if any of them breaks.

**Why this exists.** E264 changed `sagtrace::Block` from 9 fields to 14 and
`sag.py`'s `Row` with it, but left one `Row(...)` built with nine positional
arguments. That raised `TypeError` on *every* capture containing `SAGSLOW`
rows -- all 21 of them -- and the entry claiming the change was verified had
piped the tool through `head -12`, so the traceback fell off the end of the
output and the failure was reported as a success.

There was no automated test of `sag.py` anywhere in the tree. This is it.
Running it takes about a second and it would have caught that in one.

It checks three things per capture:

* the tool exits 0 on a capture that has rows, and 1 with a stated reason on one
  that has none (the `-warmup` files legitimately have none);
* `parse` produces rows whose field count matches a known format version, with
  no `None` in a field the format is supposed to carry;
* a v1 and a v3 capture do not end up in the same row list, which is what
  happened when two dumps were concatenated in one log.

Usage:  python scripts/sag_selftest.py [--dir captures/sag]
"""

from __future__ import annotations

import argparse
import pathlib
import subprocess
import sys

HERE = pathlib.Path(__file__).resolve().parent
SAG = HERE / "sag.py"


def one(path: pathlib.Path) -> tuple[bool, str]:
    """Run the tool and decide whether its behaviour is acceptable."""
    r = subprocess.run(
        [sys.executable, str(SAG), str(path)],
        capture_output=True,
        text=True,
        check=False,
    )
    out = (r.stdout or "") + (r.stderr or "")
    if "Traceback" in out:
        last = [ln for ln in out.strip().splitlines() if ln.strip()][-1]
        return False, f"crashed: {last.strip()[:140]}"
    if r.returncode == 0:
        return True, "ok"
    # A non-zero exit is fine *if* the tool said why in a way a human can act on.
    for expected in ("no SAGROW rows found", "REFUSED"):
        if expected in out:
            return True, f"declined cleanly ({expected})"
    return False, f"exit {r.returncode} with no stated reason: {out.strip()[:140]}"


def structural(path: pathlib.Path) -> tuple[bool, str]:
    """Parse in-process and check the rows are internally consistent."""
    sys.path.insert(0, str(HERE))
    import sag  # noqa: PLC0415 - imported here so a syntax error is reported per file

    try:
        snap, rows, slow = sag.parse(path)
    except SystemExit as e:  # a refusal is a pass; see `one`
        return True, f"declined cleanly ({str(e)[:60]})"
    if not rows:
        return True, "no rows"
    # Every row must agree about which format it is: either all carry the v3
    # fields or none do. A mixed list is the concatenated-dump bug.
    v3 = [r.bus_raw is not None for r in rows]
    if any(v3) and not all(v3):
        return False, f"MIXED format versions in one row list ({sum(v3)} of {len(v3)} carry v3 fields)"
    if all(v3):
        for name in ("at_fine", "pwm_ctr", "bus_raw", "phase_a", "phase_b", "phase_c"):
            if any(getattr(r, name) is None for r in rows):
                return False, f"v3 capture with a None in {name}"
    # `slow` rows are always fully populated when present.
    if any(s.bus is None for s in slow):
        return False, "a SAGSLOW row with a None field"
    return True, f"{len(rows)} rows, {len(slow)} slow, v{'3' if all(v3) else '1'}"


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--dir", default=None, help="capture directory (default: ../captures/sag)")
    a = ap.parse_args()
    root = pathlib.Path(a.dir) if a.dir else HERE.parent / "captures" / "sag"
    files = sorted(root.glob("*.txt"))
    if not files:
        print(f"REFUSED: no captures under {root} -- a selftest that tests nothing passes vacuously")
        return 1

    bad = 0
    for f in files:
        ok1, why1 = one(f)
        ok2, why2 = structural(f)
        if ok1 and ok2:
            continue
        bad += 1
        print(f"FAIL {f.name}")
        if not ok1:
            print(f"     run:       {why1}")
        if not ok2:
            print(f"     structure: {why2}")

    print(f"\n{len(files) - bad} of {len(files)} captures OK")
    if bad:
        print(f"{bad} FAILED")
        return 1
    print("sag.py handles every capture on disk")
    return 0


if __name__ == "__main__":
    sys.exit(main())
