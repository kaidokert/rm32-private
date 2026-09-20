"""Host capture for examples/sixstep-bemf.rs — the ON-window BEMF instrument.

Reads the `BW,n,step,float_ph,s,vf,mean,vm_neu` text stream over COM7 while
the firmware runs its accelerating six-step ramp, writes bemf.csv, and reports
whether the floating phase crosses either neutral candidate as speed rises
(the question that gates closed-loop BEMF lock).

The firmware self-terminates (stage-safed) at the end of its ramp, so this
script just reads until `BEMF,done` or a timeout — no motor command channel.

  python scripts/bemf_capture.py [--secs 12] [--out run_bemf]
"""

import argparse
import os
import sys
import time

import serial

PORT = "COM7"
BAUD = 2_000_000


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--secs", type=float, default=12.0)
    ap.add_argument("--out", default="run_bemf")
    args = ap.parse_args()

    os.makedirs(args.out, exist_ok=True)
    rows = []
    started = None
    aborted = None
    t0 = time.time()
    with serial.Serial(PORT, BAUD, timeout=0.2) as ser:
        buf = bytearray()
        while time.time() - t0 < args.secs:
            data = ser.read(4096)
            if data:
                buf.extend(data)
                while b"\n" in buf:
                    line, _, rest = buf.partition(b"\n")
                    buf = bytearray(rest)
                    s = line.decode("ascii", "replace").strip()
                    if not s:
                        continue
                    if s.startswith("BEMF,start"):
                        started = s
                        print("  ", s)
                    elif s.startswith("BEMF,done"):
                        print("  ", s)
                        t0 = 0  # force exit
                        break
                    elif s.startswith("BEMF,abort"):
                        aborted = s
                        print("  !! ", s)
                        t0 = 0
                        break
                    elif s.startswith("BW,"):
                        p = s.split(",")
                        if len(p) == 8:
                            try:
                                rows.append([int(x) for x in p[1:]])
                            except ValueError:
                                pass

    # Write CSV.
    csv = os.path.join(args.out, "bemf.csv")
    with open(csv, "w") as f:
        f.write("n,step,float_ph,s,vf,mean,vm_neu\n")
        for r in rows:
            f.write(",".join(str(x) for x in r) + "\n")

    print(f"\ncaptured {len(rows)} samples -> {csv}")
    if started:
        print("  ", started)
    if aborted:
        print("  ABORTED:", aborted)
    if not rows:
        print("  NO DATA — check COM7 / firmware running")
        return

    # Per-step ZC analysis: within each (n,step) the 12 samples sweep the
    # commutation window. A real spinning-rotor BEMF shows vf sweeping ACROSS
    # the neutral (sign of vf-neutral changes) — a stalled rotor shows vf
    # pinned on one side. Report the crossing fraction and vf swing per rung.
    from collections import defaultdict
    steps = defaultdict(list)
    for n, step, fph, s, vf, mean, vm in rows:
        steps[(n, step)].append((vf, mean, vm))

    def crosses(vals, key):
        signs = [1 if v[0] > v[key] else -1 for v in vals]
        return (min(signs) < 0) and (max(signs) > 0)

    # Group by rung n to see the trend as speed rises (step_us shrinks).
    ns = sorted(set(n for (n, _st) in steps))
    print("\n  rung   vf_min vf_max  swing  cross(mean) cross(vm/2)  n_steps")
    for n in ns[:: max(1, len(ns) // 25)]:
        sv = [steps[(n, st)] for st in range(6) if (n, st) in steps]
        allv = [x for grp in sv for x in grp]
        if not allv:
            continue
        vfs = [x[0] for x in allv]
        cm = sum(crosses(grp, 1) for grp in sv)
        cv = sum(crosses(grp, 2) for grp in sv)
        print(f"  {n:5d}  {min(vfs):5d} {max(vfs):5d}  {max(vfs)-min(vfs):5d}"
              f"      {cm}/{len(sv)}        {cv}/{len(sv)}       {len(sv)}")

    # Overall verdict.
    tot = len(steps)
    cm_tot = sum(crosses(v, 1) for v in steps.values())
    cv_tot = sum(crosses(v, 2) for v in steps.values())
    print(f"\n  TOTAL steps with a neutral crossing: mean {cm_tot}/{tot}"
          f" ({100*cm_tot//max(1,tot)}%), vm/2 {cv_tot}/{tot} ({100*cv_tot//max(1,tot)}%)")
    print("  (a spinning rotor with real BEMF -> high crossing fraction as"
          " the ramp speeds up; a stalled rotor -> near 0%)")


if __name__ == "__main__":
    main()
