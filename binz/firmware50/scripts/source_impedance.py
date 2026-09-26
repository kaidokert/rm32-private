"""Source impedance of the bench power path, from captures alone (E350).

The operator was owed a sag-versus-current slope, and I had recorded it as a
desk measurement only they could take. It is already in every capture:

* `ref_bus`  -- the bridge-OFF reference, taken before the run drives
                (`measure::capture_baseline`): the **unloaded** rail.
* `filt_bus` -- the sag guard's ~207 ms EWMA at the stop: the **loaded** rail.
* `hold_ma`  -- the mean current over the hold.

So `ref_bus - filt_bus` is the sag under load and `hold_ma` the load, one point
per run. Regressing sag against current over a rung ladder gives the total source
impedance seen at the board -- PSU regulation plus leads plus connector, which is
exactly the quantity that decides whether a rail dip is the supply's fault.

Codes to millivolts uses the firmware's own relation, inverted from
`measure::capture_baseline`'s floor arithmetic:

    bus_mV = code * BUS_DIVIDER_X100 * vdda_mv / (100 * ADC_RAIL)

`vdda_mv` is `3000 * VREFINT_CAL / vref`, and VREFINT_CAL is a factory word not
present in a capture, so VDDA is assumed (see `--vdda`); it enters linearly and a
1 % error in it is a 1 % error in the impedance, far below the fit's own spread.

## What limits this, stated because the numbers look tidier than they are

`filt_bus` and `ref_bus` are whole ADC codes and one code is ~9.6 mV, so each
sag point is quantised to ~0.7 A of equivalent current. The fit is
quantisation-limited, not noise-limited; its confidence interval is wide and is
reported. `hold_ma` is also known to over-read by ~6 % against a metered
anchor, which biases the slope low by the same proportion.

It is still decisive for the question asked, because the alternative hypothesis
needs an impedance several times the measured one.

    python scripts/source_impedance.py captures/2026-09-25/q5*-advref*.txt
    python scripts/source_impedance.py --self-check
"""
from __future__ import annotations

import argparse
import glob
import re
import statistics as st
import sys

BUS_DIVIDER_X100 = 1_194   # run::policy
ADC_RAIL = 4_095           # protection
MIN_PLAUSIBLE_FILT = 600   # below this the rail was not powered (E346)
MIN_CURRENT_SPAN_MA = 300  # refuse a fit with no lever arm

FIELDS = ("target_duty_tenths", "reason", "ref_bus", "filt_bus", "hold_ma")


def mv_per_code(vdda_mv: float) -> float:
    return BUS_DIVIDER_X100 * vdda_mv / (100 * ADC_RAIL)


def fit(points: list[tuple[float, float]]) -> dict:
    """Least squares of sag_mV against current_mA. Returns mOhm and its 95% CI."""
    n = len(points)
    xs = [p[0] for p in points]
    ys = [p[1] for p in points]
    mx, my = st.mean(xs), st.mean(ys)
    sxx = sum((x - mx) ** 2 for x in xs)
    if sxx == 0:
        raise ValueError("no current variation")
    b = sum((x - mx) * (y - my) for x, y in zip(xs, ys)) / sxx
    a = my - b * mx
    resid = [y - (a + b * x) for x, y in zip(xs, ys)]
    s = (sum(r * r for r in resid) / (n - 2)) ** 0.5
    se = s / sxx ** 0.5
    return {"mohm": b * 1000, "ci95": 1.96 * se * 1000, "intercept_mv": a,
            "resid_mv": s, "n": n, "span_ma": max(xs) - min(xs)}


def self_check() -> int:
    """A synthetic path of known impedance must be recovered."""
    bad = []
    for truth in (50.0, 100.0, 250.0):
        pts = [(ma, truth / 1000 * ma + 20.0) for ma in range(1500, 2400, 60)]
        got = fit(pts)["mohm"]
        ok = abs(got - truth) < 0.5
        print(f"  synthetic {truth:.0f} mOhm -> recovered {got:.1f}"
              + ("" if ok else "   <-- MISMATCH"))
        if not ok:
            bad.append(truth)
    # And a flat path must not be reported as impedance.
    flat = fit([(ma, 100.0) for ma in range(1500, 2400, 60)])["mohm"]
    if abs(flat) > 1e-9:
        print(f"  a flat sag recovered {flat:.3f} mOhm -- must be 0")
        bad.append("flat")
    if bad:
        print(f"SELF-CHECK FAILED: {bad}")
        return 1
    print("self-check OK: known impedances recovered, a flat path reads zero")
    return 0


def parse(path: str) -> dict | None:
    text = open(path, errors="replace").read()
    out = {}
    for k in FIELDS:
        m = re.search(rf"\b{k}=(-?\d+)", text)
        if not m:
            return None
        out[k] = int(m.group(1))
    return out


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("captures", nargs="*")
    ap.add_argument("--vdda", type=float, default=3300.0)
    ap.add_argument("--dip-codes", type=float, default=None,
                    help="a rail dip in codes; report the current excursion it "
                         "implies at the fitted impedance")
    ap.add_argument("--self-check", action="store_true")
    args = ap.parse_args()
    if args.self_check or not args.captures:
        return self_check()

    k = mv_per_code(args.vdda)
    paths = sorted({p for pat in args.captures for p in glob.glob(pat)})
    pts: list[tuple[float, float]] = []
    print(f"1 bus code = {k:.3f} mV (vdda assumed {args.vdda:.0f} mV)\n")
    print(f"{'capture':26s}{'rung':>5}{'rsn':>5}{'hold_ma':>9}"
          f"{'sag_codes':>11}{'sag_mV':>8}")
    for p in paths:
        r = parse(p)
        name = p.replace("\\", "/").split("/")[-1]
        if r is None:
            print(f"{name[:26]:26s}  REFUSED: fields missing")
            continue
        if r["filt_bus"] < MIN_PLAUSIBLE_FILT or r["hold_ma"] <= 0:
            print(f"{name[:26]:26s}{r['target_duty_tenths']:>5}{r['reason']:>5}"
                  f"  REFUSED: not a powered run with a hold")
            continue
        sag = r["ref_bus"] - r["filt_bus"]
        pts.append((r["hold_ma"], sag * k))
        print(f"{name[:26]:26s}{r['target_duty_tenths']:>5}{r['reason']:>5}"
              f"{r['hold_ma']:>9}{sag:>11}{sag * k:>8.0f}")

    if len(pts) < 4:
        print("\nREFUSED: fewer than 4 usable runs; no fit.")
        return 2
    f = fit(pts)
    if f["span_ma"] < MIN_CURRENT_SPAN_MA:
        print(f"\nREFUSED: current span {f['span_ma']:.0f} mA < "
              f"{MIN_CURRENT_SPAN_MA} mA; no lever arm for a slope.")
        return 2
    print(f"\nn={f['n']}  source impedance = {f['mohm']:.0f} "
          f"+- {f['ci95']:.0f} mOhm (95%)   intercept {f['intercept_mv']:.0f} mV")
    print(f"residual sd {f['resid_mv']:.1f} mV = {f['resid_mv'] / k:.2f} codes "
          f"-- the fit is QUANTISATION-limited (1 code = {k:.1f} mV)")
    print(f"current span {f['span_ma']:.0f} mA")
    print("`hold_ma` over-reads ~6% against the metered anchor, which biases "
          "this slope low by about the same.")

    if args.dip_codes:
        # **E352: this conversion is only valid for a depth measured against the
        # ADC's own floor.** The bridge-off control (`bin/idle-scan.rs`, capture
        # `captures/2026-09-24/e292-idlescan.txt`) shows `filt_bus - bus_min` is
        # 67-72 codes with NO load and NO switching, so most of a run's `bus_min`
        # depth is the sampling path, not the rail. Converting a raw `bus_min`
        # depth to an amperage over-attributes it -- which is exactly the error
        # E350 made. Require the caller to have subtracted the floor.
        print("
*** --dip-codes: pass a depth ALREADY NET of the ~70-code "
              "bridge-off ADC floor (E352). A raw bus_min depth is mostly "
              "instrument and converting it over-states the current. ***")
        if args.dip_codes > 60:
            print(f"    REFUSED: {args.dip_codes:.0f} codes is at or above the "
                  f"measured no-load floor, so it cannot be a net rail depth.")
            return 2
        mv = args.dip_codes * k
        lo, hi = f["mohm"] - f["ci95"], f["mohm"] + f["ci95"]
        print(f"\na dip of {args.dip_codes:.0f} codes = {mv:.0f} mV implies a "
              f"current excursion of")
        for label, z in (("fit", f["mohm"]), ("CI high-Z", hi), ("CI low-Z", lo)):
            if z > 0:
                print(f"  {mv / z:>6.1f} A above the operating point "
                      f"(at {z:.0f} mOhm, {label})")
        print(f"for an ordinary 2 A excursion to explain it the path would have "
              f"to be {mv / 2000 * 1000:.0f} mOhm")
    return 0


if __name__ == "__main__":
    sys.exit(main())
