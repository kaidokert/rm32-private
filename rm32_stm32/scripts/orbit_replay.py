"""Replay recorded runs through the core's wrong-phase-orbit trip
(rm32/src/main_state.rs, ORBIT_TRIP_* in constants.rs) and candidate
replacements, at every phase of the 100 ms duty-snapshot counter.

    python scripts/orbit_replay.py [--rule R ...] [--inject] captures/l431/profile_<tag>-*.csv

Rules:
  firmware  today's trip: static line duty*3+2500 mA (vbat-scaled), exempt
            while |duty - snapshot| > 150, snapshot re-taken every 100 ms
            (tumbling window).
  sliding   same line, exempt while duty moved > 150 within the last
            --hold ms (sliding window, no snapshot phase).
  speedrise same line, exempt while eHz rose more than --rise % over the
            last 50 ms.
  bemf      line from the motor model instead of duty alone:
            I = D * (D*V - eHz/KV) / R_OHM, trip above it by --margin mA.

--inject adds a synthetic wrong-phase orbit to each run's longest settled
segment at 60-80 % duty: the current steps to 11 A for 300 ms while duty
and eHz stay as recorded (the July orbit signature: 10-13 A at duty ~1400,
plausible zero-crossings). A rule must trip on it.

The recorder stores duty, actual_current (mA), eHz and bus mV every 10 ms
(fast profiles) or 50 ms (slow); the trip runs at 1 kHz, so each sample is
held for 10 or 50 ticks (current and eHz linearly interpolated). 50 ms
runs smooth out short current peaks, so they understate false trips; use
them for the injection test. `running && zero_crosses > 1000` is assumed
true while duty > 0 and old_routine is 0.
"""

import argparse
import csv
import sys

SLOPE, OFFSET, TRIP_MS, CAL_CV, TRANSIENT = 3, 2500, 30, 816, 150


def ticks(rows, per):
    for a, b in zip(rows, rows[1:]):
        for k in range(per):
            f = k / per
            yield (int(a["duty"]), int(a["old_routine"]),
                   int(a["ma"]) * (1 - f) + int(b["ma"]) * f,
                   float(a["ehz"]) * (1 - f) + float(b["ehz"]) * f,
                   int(a["mv"]) // 10)


def static_line(duty, cv):
    line = duty * SLOPE + OFFSET
    return line * cv // CAL_CV if cv > 600 else line


def run(seq, rule, phase, a):
    age, snap, count = phase, 0, 0
    duties, ehzs = [], []
    for n, (duty, old, ma, ehz, cv) in enumerate(seq):
        duties.append(duty)
        ehzs.append(ehz)
        if rule == "firmware":
            age += 1
            if age >= 100:
                snap, age = duty, 0
            exempt = abs(duty - snap) > TRANSIENT
            line = static_line(duty, cv)
        elif rule == "sliding":
            w = duties[-a.hold:]
            exempt = max(w) - min(w) > TRANSIENT
            line = static_line(duty, cv)
        elif rule == "speedrise":
            past = ehzs[-51] if len(ehzs) > 50 else ehzs[0]
            exempt = past > 0 and ehz > past * (1 + a.rise / 100)
            line = static_line(duty, cv)
        else:  # bemf
            d, v = duty / 2000, cv / 100
            exempt = False
            line = 1000 * d * max(d * v - ehz / a.kv, 0) / a.r_ohm + a.margin
        if duty > 0 and not old and not exempt and ma > line:
            count += 1
            if count > TRIP_MS:
                return n
        else:
            count = 0
    return None


def inject(seq):
    """Synthetic orbit on the longest run of ticks at 60-80 % duty."""
    best, start, cur = (0, 0), None, 0
    for n, (duty, old, *_rest) in enumerate(seq):
        if 1200 <= duty <= 1600 and not old:
            start = n if start is None else start
            if n - start + 1 > best[0]:
                best = (n - start + 1, start)
        else:
            start = None
    length, s = best
    if length < 1000:
        return None, None
    at = s + length // 2
    out = list(seq)
    for n in range(at, at + 300):
        duty, old, _ma, ehz, cv = out[n]
        out[n] = (duty, old, 11000.0, ehz, cv)
    return out, at


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("csv", nargs="+")
    ap.add_argument("--rule", action="append", choices=["firmware", "sliding", "speedrise", "bemf"])
    ap.add_argument("--hold", type=int, default=300, help="sliding: window, ms")
    ap.add_argument("--rise", type=float, default=2.0, help="speedrise: %% per 50 ms")
    ap.add_argument("--kv", type=float, default=425.0, help="bemf: eHz per volt")
    ap.add_argument("--r-ohm", type=float, default=0.387, help="bemf: winding resistance")
    ap.add_argument("--margin", type=float, default=3000.0, help="bemf: mA above the model")
    ap.add_argument("--inject", action="store_true")
    a = ap.parse_args()
    rules = a.rule or ["firmware"]
    for path in a.csv:
        rows = list(csv.DictReader(open(path)))
        dt = float(rows[1]["t_s"]) - float(rows[0]["t_s"])
        per = round(dt * 1000)
        if per not in (10, 50):
            sys.exit(f"refuse: {path} recorder period {dt * 1000:.1f} ms, need 10 or 50 ms")
        seq = list(ticks(rows, per))
        cols = []
        for rule in rules:
            phases = range(100) if rule == "firmware" else range(1)
            hits = sum(run(seq, rule, p, a) is not None for p in phases)
            cell = f"{rule} {hits}/{len(phases)}"
            if a.inject:
                inj, at = inject(seq)
                if inj is None:
                    cell += " inj=n/a"
                else:
                    caught = [run(inj, rule, p, a) for p in phases]
                    lat = [c - at for c in caught if c is not None and c >= at]
                    cell += f" inj={len(lat)}/{len(phases)}" + (f"@{max(lat)}ms" if lat else "")
            cols.append(cell)
        print("  ".join(f"{c:28}" for c in cols), path.split("profile_")[-1])


if __name__ == "__main__":
    main()
