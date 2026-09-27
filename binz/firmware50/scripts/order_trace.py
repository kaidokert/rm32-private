"""Validate and join E381 compact tails by acceptance ordinal, not nearest time.

All control timestamps are TIM17 microseconds modulo65536. Only ORDERB's
before/after fields are TIM2 ticks at the declared8MHz. Brackets wider than2us
are flagged, never silently turned into precise phase. Sag rows are processing
observations; a join cannot establish physical zero-crossing validity or ADC
aperture time. This tool reports evidence, not a motor qualification.
"""
from pathlib import Path
import argparse
import json

import cohort


def delta(a, b, bits=16):
    return (b - a) & ((1 << bits) - 1)


def parse(path):
    lines = Path(path).read_text(encoding="utf-8").splitlines()
    heads = [cohort.fields(s) for s in lines if s.startswith("ORDERSNAP ")]
    if len(heads) != 1 or lines.count("ORDEREND") != 1:
        raise ValueError("missing/multiple order header/end")
    h = {k: int(v) for k, v in heads[0].items()}
    if (h.get("row_v"), h.get("fine_hz"), h.get("frozen")) != (1, 8_000_000, 1):
        raise ValueError("order version/units/freeze mismatch")
    a = [list(map(int, s.split()[1:])) for s in lines if s.startswith("ORDERA ")]
    b = [list(map(int, s.split()[1:])) for s in lines if s.startswith("ORDERB ")]
    for rows, name, width in [(a, "acc", 6), (b, "com", 7)]:
        if not rows or len(rows) != h.get(name + "_len") or len(rows) > 128:
            raise ValueError(f"incomplete {name} tail")
        if h.get(name + "_total", -1) < len(rows) or any(len(r) != width for r in rows):
            raise ValueError(f"invalid {name} count/schema")
        if len({r[0] for r in rows}) != len(rows):
            raise ValueError(f"duplicate {name} ordinal")
        if any(not 1 <= r[-1] <= 6 for r in rows):
            raise ValueError(f"invalid {name} sector")
        for r in rows:
            bounds = ([32, 16, 16, 16, 16, 8] if name == "acc" else
                      [32, 16, 16, 16, 32, 32, 8])
            if any(not 0 <= v < (1 << bits) for v, bits in zip(r, bounds)):
                raise ValueError(f"out-of-range {name} field")
        if any(delta(p[0], r[0], 32) != 1 for p, r in zip(rows, rows[1:])):
            raise ValueError(f"noncontiguous {name} ordinals")
    return h, a, b


def paired(a, b):
    accepts = {r[0]: r for r in a}
    out = []
    for ident, service, scheduled, bridge, before, after, step in b:
        if ident not in accepts:
            continue  # bounded tails need not start/end on the same ordinal
        _, raw, interval, avg, wait, accepted_step = accepts[ident]
        if step != (accepted_step % 6) + 1:
            raise ValueError("paired sector identity mismatch")
        elapsed = delta(raw, bridge)
        if elapsed >= 32768:
            raise ValueError("ambiguous/negative entry-to-bridge delta")
        service_late = delta(scheduled, service)
        if service_late >= 32768 or delta(service, bridge) >= 32768:
            raise ValueError("ambiguous/negative service or bridge ordering")
        out.append(dict(id=ident, entry_us=raw, interval_us=interval, average_us=avg,
                        wait_us=wait, scheduled_us=scheduled, service_us=service,
                        bridge_us=bridge, entry_to_bridge_us=elapsed,
                        deadline_error_us=elapsed - wait,
                        service_late_us=service_late,
                        bracket_ticks=delta(before, after, 32),
                        fine_before=before, step=step))
    if not out:
        raise ValueError("no paired accepted/commutated events")
    return out


def errors(path):
    try:
        _, a, b = parse(path)
        paired(a, b)
        return []
    except (ValueError, KeyError) as exc:
        return [f"order trace: {exc}"]


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("capture", type=Path)
    args = ap.parse_args()
    h, a, b = parse(args.capture)
    rows = paired(a, b)
    print(json.dumps(dict(header=h, pairs=len(rows),
        wide_brackets=sum(r["bracket_ticks"] > 16 for r in rows),
        max_deadline_error_us=max(r["deadline_error_us"] for r in rows))))
    for row in rows:
        print(json.dumps(row))


if __name__ == "__main__":
    main()
