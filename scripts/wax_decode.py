#!/usr/bin/env python3
"""Decode a WAXWING WX dump and render the arc-vs-VALUE verdict.

The bedrock question: at the frozen wall dropout, does a floating-phase
BEMF arc sweep across neutral through a window where the COMP VALUE bit
(T1S bit15) never flips? If yes, the deaf window is ANALOG (the hardware
comparator's differential never crossed threshold at the arc) and is
CPU-load-independent by construction — budget cannot be the cause. If
the VALUE bit DOES flip at every arc, the silence is elsewhere.

Row layout (per 20 kHz tick, oldest-first after head rotation):
  a   = phase A (JDR1, 12-bit, mid-ON sample)
  b   = phase B (JDR2, 12-bit)
  pos = TIM2.CNT (0.5 us tick)
  t1s: bit15 = COMP VALUE (1 = flipped to post-ZC level), bits12-14 =
       step, bits0-11 = TIM1.CNT
"""
import re
import sys


def parse(path):
    data = open(path, "rb").read()
    txt = data.decode("latin1", errors="replace")
    m = re.search(r"WX n=1024 head=(\d+) ci=(\d+) arr=(\d+) frozen=(\d+)", txt)
    if not m:
        print("no WX header found")
        sys.exit(1)
    head, ci, arr, frozen = (int(m.group(i)) for i in range(1, 5))
    start = m.end()
    end = txt.find("WX END", start)
    body = txt[start:end if end > 0 else len(txt)]
    # A WX data line is EXACTLY 16 hex quads (4 tuples). Parse line-strict
    # so interleaved/trailing text ([loop], i-lines) can't inject fake
    # tuples. A partial/truncated dump simply yields fewer valid lines.
    quad = re.compile(r"^[0-9a-fA-F]{4}$")
    tuples = []
    for ln in body.split("\n"):
        toks = ln.split()
        if len(toks) != 16 or not all(quad.match(t) for t in toks):
            continue
        vals = [int(t, 16) for t in toks]
        for i in range(0, 16, 4):
            tuples.append(tuple(vals[i:i + 4]))
    return head, ci, arr, frozen, tuples


def decode(tuples, head):
    """Rotate to time order (oldest = head) and split t1s."""
    n = len(tuples)
    if n < 1024:
        head = 0  # partial dump; use as-is
    order = [(head + i) % n for i in range(n)]
    out = []
    for idx in order:
        a, b, pos, t1s = tuples[idx]
        comp = (t1s >> 15) & 1
        step = (t1s >> 12) & 7
        out.append((a, b, pos, step, comp))
    return out


def runs(samples):
    """Contiguous runs of constant comp bit: (start, len, comp, steps,
    a_range, b_range)."""
    r = []
    i = 0
    while i < len(samples):
        comp = samples[i][4]
        j = i
        amin = amax = samples[i][0]
        bmin = bmax = samples[i][1]
        steps = set()
        while j < len(samples) and samples[j][4] == comp:
            amin = min(amin, samples[j][0]); amax = max(amax, samples[j][0])
            bmin = min(bmin, samples[j][1]); bmax = max(bmax, samples[j][1])
            steps.add(samples[j][3])
            j += 1
        r.append((i, j - i, comp, steps, amax - amin, bmax - bmin))
        i = j
    return r


def sparkline(vals, lo, hi, h=8):
    ramp = " .:-=+*#@"
    out = []
    for v in vals:
        if hi == lo:
            out.append(" ")
        else:
            k = int((v - lo) / (hi - lo) * (len(ramp) - 1))
            out.append(ramp[max(0, min(len(ramp) - 1, k))])
    return "".join(out)


def main():
    if len(sys.argv) < 2:
        print("usage: wax_decode.py <capture.bin>")
        sys.exit(1)
    head, ci, arr, frozen, tuples = parse(sys.argv[1])
    print(f"WX head={head} ci={ci} ({ci*0.5:.1f}us) arr={arr} "
          f"frozen={frozen} tuples={len(tuples)}")
    if not frozen:
        print("WARNING: ring NOT frozen -- churn overwrote the deaf window; "
              "the dump is a live ~52ms snapshot, not the dropout post-mortem.")
    s = decode(tuples, head)
    rs = runs(s)

    lens = sorted(r[1] for r in rs)
    med = lens[len(lens) // 2] if lens else 0
    # expected commutation cadence in ticks: ci(0.5us) -> tick(50us=100*0.5us)
    exp_ticks = ci / 100.0 if ci else 0
    print(f"comp-bit runs: {len(rs)}  median_run={med} ticks  "
          f"expected_comm_cadence~={exp_ticks:.1f} ticks")

    # Deaf windows: comp bit constant far longer than a commutation, with
    # a real phase-voltage arc (large swing) inside -> arc present,
    # comparator silent.
    print("\n-- candidate deaf windows (comp constant >> cadence, arc present) --")
    flagged = 0
    thresh = max(3 * med, exp_ticks * 2, 6)
    for (start, length, comp, steps, ar, br) in rs:
        arc = max(ar, br)
        if length >= thresh and arc > 300:
            flagged += 1
            print(f"  tick {start:4d}+{length:3d}  comp={comp}  "
                  f"step={sorted(steps)}  arcA={ar:4d} arcB={br:4d}  "
                  f"(comparator SILENT {length} ticks while phase swept {arc} LSB)")
    if not flagged:
        print("  none -- comp bit tracked the arcs (VALUE flipped at each "
              "crossing). Deaf-silence NOT reproduced in this window.")

    # Trace the last 160 ticks before the freeze (the dropout region).
    tail = s[-160:]
    av = [t[0] for t in tail]
    bv = [t[1] for t in tail]
    cv = [t[4] for t in tail]
    stv = [t[3] for t in tail]
    lo = min(min(av), min(bv)); hi = max(max(av), max(bv))
    print(f"\n-- last {len(tail)} ticks before freeze (A/B range {lo}..{hi}) --")
    print("A   " + sparkline(av, lo, hi))
    print("B   " + sparkline(bv, lo, hi))
    print("cmp " + "".join("^" if c else "." for c in cv))
    print("stp " + "".join(str(x) for x in stv))

    # Verdict scaffold.
    print("\n-- verdict input --")
    flips = sum(1 for i in range(1, len(s)) if s[i][4] != s[i - 1][4])
    print(f"  total comp-bit flips over {len(s)} ticks: {flips}")
    print(f"  deaf windows (arc present, comparator silent): {flagged}")
    if flagged:
        print("  => ANALOG deaf window CONFIRMED at the wall: the arc crossed")
        print("     but COMP2_VALUE never flipped. Budget cannot cause this.")
    else:
        print("  => arc-vs-VALUE deaf silence NOT seen in this capture.")


if __name__ == "__main__":
    main()
