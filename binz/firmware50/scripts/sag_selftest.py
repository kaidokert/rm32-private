#!/usr/bin/env python3
"""Self-test for `sag.py`: real captures plus synthetic fixtures with expected verdicts.

**Why this exists, and why it was rewritten.** E264 changed `sagtrace::Block`
and `sag.py`'s `Row` with it, but left one `Row(...)` built positionally with
too few arguments. That raised `TypeError` on every capture containing
`SAGSLOW` rows -- all of them -- and the entry claiming the change was verified
had piped the tool through `head -12`, so the traceback fell off the end.

The first version of this file caught that, and an independent review then
**defeated it four ways** (E269). The hole that mattered: a synthetic v3 capture
with `at_fine = 0` on every row -- the original defect verbatim -- passed, and
`sag.py` reported `fine deltas: 255 of 255 usable, min=0.000 max=0.000 us`. The
old `one()` accepted any exit 0 *and* accepted the string "REFUSED"
unconditionally, so a refused file counted as a pass; `structural()` checked
`at_fine is not None` and never that it varied.

So: **every case now states the verdict it expects**, and a case that comes out
any other way fails. Real captures expect `ok` or `no-rows`; the synthetic
fixtures expect a specific refusal, and if one of them ever passes, this test
fails. That is the only shape in which a self-test is evidence.

Usage:  python scripts/sag_selftest.py [--dir captures/sag] [--keep]
"""

from __future__ import annotations

import argparse
import pathlib
import subprocess
import sys
import tempfile

HERE = pathlib.Path(__file__).resolve().parent
SAG = HERE / "sag.py"

# A minimal well-formed v3 capture. `mk` edits copies of this.
GOOD_SNAP = (
    "SAGSNAP judged=300 fast_len=4 slow_len=2 slow_every=32 frozen=1 "
    "num=95 den=100 streak_to_latch=3 adc_rail=4095 "
    "fine_hz=8000000 span16_us=8192 row_v=3"
)
# at at_fine pwm_ctr bus_raw pa pb pc bus vref filt_bus filt_vref streak step duty zc
# The fine stamps must agree with the coarse ones: 101 us of coarse spacing is
# 808 ticks at 125 ns. An earlier version of this fixture used 8808-tick steps
# against 101 us coarse steps -- a ten-fold disagreement -- and `sag.py`
# accepted it, which is how the loose cross-check in `fine_delta_us` was found
# (E269). A fixture that is itself inconsistent cannot test consistency.
GOOD_ROWS = [
    "SAGROW 1000  8000  100 1193 2060 2060 2060 1193 1507 1193 1507 0 1 550 40",
    "SAGROW 1101  8808  733 1190 2055 2061 2059 1192 1507 1193 1507 0 2 550 40",
    "SAGROW 1202  9616  300  700 1800 2061 2059 1131 1507 1193 1507 1 3 550 40",
    "SAGROW 1303 10424  900 1188 2058 2060 2061 1186 1507 1193 1507 0 4 550 40",
]
GOOD_SLOW = ["SAGSLOW 1193 1507 1193 1507", "SAGSLOW 1192 1507 1193 1507"]


def write(d: pathlib.Path, name: str, snap: str, rows: list[str], slow: list[str]) -> pathlib.Path:
    p = d / name
    p.write_text(
        "\n".join([snap, *rows, *slow, "SAGEND", "BEMFDONE reason=26"]) + "\n",
        encoding="utf-8",
    )
    return p


def fixtures(d: pathlib.Path) -> list[tuple[pathlib.Path, str, str]]:
    """(path, expected verdict, why this case exists)."""
    cases: list[tuple[pathlib.Path, str, str]] = []

    cases.append((
        write(d, "v3_good.txt", GOOD_SNAP, GOOD_ROWS, GOOD_SLOW),
        "ok",
        "a well-formed v3 capture must be read, not refused -- no on-disk "
        "capture exercises the v3 path at all, so without this the whole v3 "
        "branch is untested",
    ))

    # The E264 defect verbatim: the clock never ran. Built by **field index**,
    # not by substituting literal tick values -- an earlier version listed the
    # old literals and silently stopped zeroing rows when GOOD_ROWS changed,
    # so only the first row was flat, at_fine was not constant, and the fixture
    # passed when it was supposed to be refused. A fixture builder that can
    # drift out of step with its own data is the same class of defect as the
    # tool it tests.
    def zero_fine(row: str) -> str:
        f = row.split()
        f[2] = "0"  # SAGROW at at_fine ... -> index 2 is at_fine
        return " ".join(f)

    flat = [zero_fine(r) for r in GOOD_ROWS]
    cases.append((
        write(d, "v3_flat_fine.txt", GOOD_SNAP, flat, GOOD_SLOW),
        "refused",
        "every at_fine identical = Hal::fine() unoverridden or TIM2 unclocked. "
        "This is the exact defect the tool exists to expose and it used to be "
        "reported as '255 of 255 usable, 0.000 us'",
    ))

    cases.append((
        write(d, "v3_no_rate.txt", GOOD_SNAP.replace(" fine_hz=8000000", ""), GOOD_ROWS, GOOD_SLOW),
        "refused",
        "fine stamps with no declared rate: reading them at the old rate is "
        "wrong by 8x",
    ))

    cases.append((
        write(d, "v3_future.txt", GOOD_SNAP.replace("row_v=3", "row_v=9"), GOOD_ROWS, GOOD_SLOW),
        "refused",
        "a future format must not be parsed by field count alone",
    ))

    cases.append((
        write(d, "bad_width.txt", GOOD_SNAP, ["SAGROW 1 2 3"], GOOD_SLOW),
        "refused",
        "a row of unknown width must refuse, not be skipped -- skipping it "
        "reports an empty ring as a clean one",
    ))

    cases.append((
        write(d, "bad_slow.txt", GOOD_SNAP, GOOD_ROWS, ["SAGSLOW 1 2"]),
        "refused",
        "a malformed SAGSLOW must refuse like a SAGROW; it used to be dropped "
        "silently and the decimated section vanished with no message",
    ))

    cases.append((
        write(d, "no_rows.txt", GOOD_SNAP, [], []),
        "no-rows",
        "a header with no rows must decline with a stated reason",
    ))
    return cases


def verdict(path: pathlib.Path) -> tuple[str, str]:
    """Run the tool and classify the outcome. Never returns 'ok' on a crash."""
    r = subprocess.run(
        [sys.executable, str(SAG), str(path)], capture_output=True, text=True, check=False
    )
    out = (r.stdout or "") + (r.stderr or "")
    if "Traceback" in out:
        last = [ln for ln in out.strip().splitlines() if ln.strip()][-1]
        return "crash", last.strip()[:160]
    if r.returncode == 0:
        return "ok", ""
    if "no SAGROW rows found" in out:
        return "no-rows", ""
    if "REFUSED" in out:
        line = next((ln for ln in out.splitlines() if "REFUSED" in ln), "")
        return "refused", line.strip()[:160]
    return "unexplained", f"exit {r.returncode}: {out.strip()[:160]}"


def structural(path: pathlib.Path) -> str | None:
    """Parse in-process; return a complaint or None. Refusals are not checked
    here -- `verdict` owns those -- so a refusal cannot launder a bad parse."""
    sys.path.insert(0, str(HERE))
    import sag  # noqa: PLC0415

    try:
        _snap, rows, slow = sag.parse(path)
    except SystemExit:
        return None
    if not rows:
        return None
    v3 = [r.bus_raw is not None for r in rows]
    if any(v3) and not all(v3):
        return f"MIXED format versions in one row list ({sum(v3)} of {len(v3)})"
    if all(v3):
        for name in ("at_fine", "pwm_ctr", "bus_raw", "phase_a", "phase_b", "phase_c"):
            if any(getattr(r, name) is None for r in rows):
                return f"v3 capture with a None in {name}"
        if len({r.at_fine for r in rows}) == 1:
            return "v3 rows parsed with a constant at_fine -- a stopped clock"
    if any(s.bus is None for s in slow):
        return "a SAGSLOW row with a None field"
    return None


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--dir", default=None, help="capture directory (default: ../captures/sag)")
    ap.add_argument("--keep", action="store_true", help="keep the synthetic fixtures")
    a = ap.parse_args()

    root = pathlib.Path(a.dir) if a.dir else HERE.parent / "captures" / "sag"
    real = sorted(root.glob("*.txt"))
    if not real:
        print(f"REFUSED: no captures under {root} -- a selftest that tests nothing passes vacuously")
        return 1

    bad = 0
    # Real captures: either readable, or declined because they hold no rows.
    for f in real:
        got, why = verdict(f)
        if got not in ("ok", "no-rows"):
            bad += 1
            print(f"FAIL {f.name}: expected ok/no-rows, got {got} -- {why}")
            continue
        if (complaint := structural(f)) is not None:
            bad += 1
            print(f"FAIL {f.name}: {complaint}")

    # Synthetic fixtures: each states the verdict it must produce.
    tmp = pathlib.Path(tempfile.mkdtemp(prefix="sagfix-"))
    for path, want, why in fixtures(tmp):
        got, detail = verdict(path)
        if got != want:
            bad += 1
            print(f"FAIL {path.name}: expected {want}, got {got}")
            print(f"     this case exists because: {why}")
            if detail:
                print(f"     tool said: {detail}")
        elif (complaint := structural(path)) is not None and want == "ok":
            bad += 1
            print(f"FAIL {path.name}: {complaint}")
    if not a.keep:
        for p in tmp.glob("*"):
            p.unlink()
        tmp.rmdir()

    n = len(real) + 7
    print(f"\n{n - bad} of {n} cases OK ({len(real)} real captures + 7 fixtures)")
    if bad:
        print(f"{bad} FAILED")
        return 1
    print("sag.py handles every capture on disk and refuses every malformed fixture")
    return 0


if __name__ == "__main__":
    sys.exit(main())
