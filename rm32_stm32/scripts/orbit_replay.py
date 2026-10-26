"""Replay recorded runs through the core's wrong-phase-orbit trip
(rm32/src/main_state.rs, ORBIT_TRIP_* in constants.rs) at every phase of
its 100 ms duty-snapshot counter, and report for how many phases the trip
would have fired.

    python scripts/orbit_replay.py captures/l431/profile_<tag>-*.csv

The recorder stores duty, actual_current (mA) and bus mV every 10 ms (fast
profiles); the trip runs at 1 kHz, so each 10 ms sample is held for 10
ticks (current linearly interpolated). `running && zero_crosses > 1000` is
assumed true while duty > 0 and old_routine is 0, which the recorder cannot
show exactly. A run whose recorder period is not 10 ms is refused.
"""

import csv
import sys

SLOPE, OFFSET, TRIP_MS, CAL_CV, TRANSIENT = 3, 2500, 30, 816, 150


def ticks(rows):
    for a, b in zip(rows, rows[1:]):
        for k in range(10):
            f = k / 10
            yield (int(a["duty"]), int(a["old_routine"]),
                   int(a["ma"]) * (1 - f) + int(b["ma"]) * f, int(a["mv"]) // 10)


def trips(seq, phase):
    age, snap, count = phase, 0, 0
    for n, (duty, old, ma, cv) in enumerate(seq):
        age += 1
        if age >= 100:
            snap, age = duty, 0
        transient = abs(duty - snap) > TRANSIENT
        line = duty * SLOPE + OFFSET
        if cv > 600:
            line = line * cv // CAL_CV
        if duty > 0 and not old and not transient and ma > line:
            count += 1
            if count > TRIP_MS:
                return n
        else:
            count = 0
    return None


def main():
    for path in sys.argv[1:]:
        rows = list(csv.DictReader(open(path)))
        dt = float(rows[1]["t_s"]) - float(rows[0]["t_s"])
        if abs(dt - 0.01) > 1e-6:
            sys.exit(f"refuse: {path} recorder period {dt * 1000:.0f} ms, need 10 ms")
        seq = list(ticks(rows))
        hits = [p for p in range(100) if trips(seq, p) is not None]
        print(f"{len(hits):3d}/100 phases trip  {path}")


if __name__ == "__main__":
    main()
