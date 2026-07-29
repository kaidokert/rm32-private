#!/usr/bin/env python3
"""Plot a frozen WAXWING window: phase A/B, COMP VALUE bit, step vs tick.

The bedrock read: at the lock-loss tail, does a phase arc sweep across
its neutral while the COMP VALUE bit stays flat (analog deaf window), or
does VALUE flip at the crossings (comparator alive -> silence is
elsewhere)?
"""
import sys
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt

sys.argv0 = sys.argv[0]
from wax_decode import parse, decode  # reuse the strict parser


def main():
    path = sys.argv[1]
    tail = int(sys.argv[2]) if len(sys.argv) > 2 else 1024
    head, ci, arr, frozen, tuples = parse(path)
    s = decode(tuples, head)
    s = s[-tail:]
    t = list(range(len(s)))
    a = [x[0] for x in s]
    b = [x[1] for x in s]
    step = [x[3] for x in s]
    comp = [x[4] for x in s]

    fig, ax = plt.subplots(3, 1, figsize=(16, 9), sharex=True,
                           gridspec_kw={"height_ratios": [3, 1, 1]})
    ax[0].plot(t, a, lw=0.9, color="#2b6cb0", label="phase A (ch9)")
    ax[0].plot(t, b, lw=0.9, color="#c05621", label="phase B (ch10)")
    ax[0].set_ylabel("mid-ON sample (LSB)")
    ax[0].legend(loc="upper right", fontsize=8)
    ax[0].set_title(f"{path}  frozen={frozen}  ci={ci} ({ci*0.5:.0f}us)  "
                    f"arr={arr}  (tail {len(s)} ticks -> freeze at right)")
    ax[0].grid(alpha=0.25)

    ax[1].step(t, comp, where="post", lw=0.9, color="#276749")
    ax[1].set_ylabel("COMP VALUE\n(1=post-ZC)")
    ax[1].set_yticks([0, 1])
    ax[1].grid(alpha=0.25)

    ax[2].step(t, step, where="post", lw=0.9, color="#6b46c1")
    ax[2].set_ylabel("comm step")
    ax[2].set_xlabel("tick (50 us) before freeze")
    ax[2].set_yticks(range(0, 8))
    ax[2].grid(alpha=0.25)

    out = path.rsplit(".", 1)[0] + f"_tail{len(s)}.png"
    fig.tight_layout()
    fig.savefig(out, dpi=110)
    print("wrote", out)

    # numeric lock-loss tail readout: per-step, did comp flip inside it?
    print("\nper-step comp behavior (last 40 steps before freeze):")
    segs = []
    i = 0
    while i < len(s):
        st = s[i][3]
        j = i
        flips = 0
        prev = s[i][4]
        while j < len(s) and s[j][3] == st:
            if s[j][4] != prev:
                flips += 1
                prev = s[j][4]
            j += 1
        segs.append((st, j - i, flips))
        i = j
    for (st, ln, fl) in segs[-40:]:
        tag = "  <-- SILENT (no flip)" if fl == 0 and ln >= 3 else ""
        print(f"  step {st}  len={ln:3d} ticks  comp_flips={fl}{tag}")


if __name__ == "__main__":
    main()
