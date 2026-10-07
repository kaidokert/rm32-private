#!/usr/bin/env python3
"""Map rm32's and firmware50's per-sector interval statistics onto PHYSICAL
phase crossings, from existing captures (no bench time).

Both firmwares tag an interval with the commutation step current at its
accept (the sector it ENDS in), use the same six-step drive table (1 A->B,
2 C->B, 3 C->A, 4 B->A, 5 B->C, 6 A->C; floating C,A,B,C,A,B) on the same
board wiring, so a step number names the same physical crossing in both:

    step:     1    2    3    4    5    6
    crossing: C+   A-   B+   C-   A+   B-     (+ rising, - falling)

Sources:
  rm32       captures/binz/spin_*.txt : `r duty=` hold snapshot, `h1..h6`
             (|iv - mean6| in 1 us bins 0..14, bin 15 = >= 15 us; `dev_x6` =
             signed mean deviation x6 in 0.5 us counts), `he` raw traces.
  firmware50 binz/firmware50/captures/char/map-NNN.txt (NNN = duty tenths):
             `CHARHIST s` (|iv - mean6| 1 us bins 0..63, settled hold window)
             and the `CHARACC at_us wait flag` raw tail (step = flag & 7),
             from which signed per-sector deviations are computed.

For each source and sector it prints: P(|dev| >= 15 us), the share of the
far mode (|dev| >= 20 us), and the signed mean deviation in us. Same-phase
asymmetry in both firmwares points at the board/motor; different phases
point at each firmware's own comparator/mux timing.

    python scripts/sector_phase_compare.py --rm32 captures/binz/spin_X.txt \
        --fw50 ../binz/firmware50/captures/char/map-800.txt
    python scripts/sector_phase_compare.py --fw50-all   # every map rung >= 500
"""

from __future__ import annotations

import argparse
import glob
import pathlib
import re
import sys

PHASE = {1: "C+", 2: "A-", 3: "B+", 4: "C-", 5: "A+", 6: "B-"}
HERE = pathlib.Path(__file__).resolve().parent
FW50_CHAR = HERE.parent.parent / "binz" / "firmware50" / "captures" / "char"


def lines(path: pathlib.Path) -> list[str]:
    return path.read_bytes().decode("latin-1").replace("\r", "").splitlines()


def hist_stats(bins: list[int], overflow_last: bool) -> tuple[int, float, float]:
    n = sum(bins)
    if n == 0:
        return 0, 0.0, 0.0
    ge15 = sum(bins[15:])
    if overflow_last:
        far = None  # rm32's 16-bin histogram cannot resolve >= 20 us
    else:
        far = sum(bins[20:])
    return n, 100.0 * ge15 / n, (100.0 * far / n) if far is not None else float("nan")


def signed_from_tail(recs: list[tuple[int, int]]) -> dict[int, tuple[int, float]]:
    """recs: (at_us u16, step). Signed iv - mean6 per sector, in us."""
    acc: dict[int, list[float]] = {s: [] for s in range(1, 7)}
    ivs: list[int] = []
    for (t0, _), (t1, s1) in zip(recs, recs[1:]):
        iv = (t1 - t0) & 0xFFFF
        ivs.append(iv)
        if len(ivs) >= 6 and 1 <= s1 <= 6:
            m = sum(ivs[-6:]) / 6.0
            acc[s1].append(iv - m)
    return {s: (len(v), sum(v) / len(v) if v else 0.0) for s, v in acc.items()}


def parse_fw50(path: pathlib.Path) -> dict:
    hist: dict[int, list[int]] = {}
    tail: list[tuple[int, int]] = []
    out: dict = {"src": f"fw50 {path.name}"}
    for ln in lines(path):
        f = ln.split()
        if not f:
            continue
        if f[0] == "CHARHIST" and len(f) >= 3:
            hist[int(f[1])] = [int(x) for x in f[2:]]
        elif f[0] == "CHARACC" and len(f) == 4:
            tail.append((int(f[1]), int(f[3]) & 7))
        elif f[0] == "BEMFRATE":
            m = re.search(r"ehz_from_sector=(\d+)", ln)
            out["ehz"] = int(m.group(1)) if m else None
        elif f[0] == "COASTTIMING":
            m = re.search(r"ehz_first=(\d+)", ln)
            out["coast_ehz"] = int(m.group(1)) if m else None
        elif f[0] == "BEMFRUN":
            m = re.search(r"target_duty_tenths=(\d+)", ln)
            out["duty"] = int(m.group(1)) / 10 if m else None
    if len(hist) != 6:
        raise SystemExit(f"{path}: expected 6 CHARHIST rows, found {len(hist)}")
    out["hist"] = {s: hist_stats(hist[s], overflow_last=False) for s in range(1, 7)}
    out["signed"] = signed_from_tail(tail)
    out["tail_n"] = len(tail)
    return out


def parse_rm32(path: pathlib.Path) -> dict:
    out: dict = {"src": f"rm32 {path.name}"}
    hist: dict[int, tuple[list[int], int]] = {}
    traces = []
    for ln in lines(path):
        m = re.match(r"^h([1-6]) n=(\d+) dev_x6=(-?\d+) b=([\d,]+)$", ln)
        if m:
            hist[int(m.group(1))] = ([int(x) for x in m.group(4).split(",")], int(m.group(3)))
            continue
        m = re.match(r"^r duty=(\d+) .*ecom10=(\d+)", ln)
        if m:
            out["duty"] = int(m.group(1)) / 20.0
            ec = int(m.group(2))
            out["ehz"] = round(1e7 / ec) if ec else None
        if ln.startswith("he"):
            traces.append(ln)
    if len(hist) != 6:
        raise SystemExit(f"{path}: expected h1..h6, found {sorted(hist)} (killed or no hold?)")
    out["hist"] = {s: hist_stats(hist[s][0], overflow_last=True) for s in range(1, 7)}
    # dev_x6 is 6x the signed mean deviation in 0.5 us counts: /12 -> us.
    out["signed"] = {s: (hist[s][0] and sum(hist[s][0]), hist[s][1] / 12.0) for s in range(1, 7)}
    out["traces"] = traces[-4:]
    return out


def show(d: dict) -> None:
    hdr = f"{d['src']}  duty={d.get('duty')}%  loop_ehz={d.get('ehz')}"
    if d.get("coast_ehz"):
        hdr += f"  coast_ehz={d['coast_ehz']}"
    print(hdr)
    print("  sector crossing   n       P>=15us  P>=20us  signed_us")
    for s in range(1, 7):
        n, p15, p20 = d["hist"][s]
        sn, sm = d["signed"][s]
        p20s = "   n/a " if p20 != p20 else f"{p20:6.1f}%"
        print(f"  {s}      {PHASE[s]:>3}   {n:7d}  {p15:6.1f}%  {p20s}  {sm:+7.1f}")
    if d.get("tail_n") is not None:
        print(f"  (signed from the {d['tail_n']}-accept raw tail)")
    for t in d.get("traces", []):
        print("  " + t)
    print()


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--rm32", nargs="*", default=[])
    ap.add_argument("--fw50", nargs="*", default=[])
    ap.add_argument("--fw50-all", action="store_true", help="every map-NNN.txt with NNN >= 500")
    a = ap.parse_args()
    fw = [pathlib.Path(p) for p in a.fw50]
    if a.fw50_all:
        fw += sorted(
            (pathlib.Path(p) for p in glob.glob(str(FW50_CHAR / "map-*.txt"))
             if int(re.search(r"map-(\d+)", p).group(1)) >= 500),
            key=lambda p: int(re.search(r"map-(\d+)", p.name).group(1)),
        )
    if not (a.rm32 or fw):
        ap.error("nothing to compare")
    for p in a.rm32:
        show(parse_rm32(pathlib.Path(p)))
    for p in fw:
        show(parse_fw50(p))
    return 0


if __name__ == "__main__":
    sys.exit(main())
