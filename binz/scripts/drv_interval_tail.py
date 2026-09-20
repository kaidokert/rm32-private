"""Decode IT85 accepted-event tails and summarize complete electrical cycles.

The wire field named ``reference_half_us`` is EV_ACC's ``this_zc``: the newly
measured interval, not ``S.average_interval``.  E857 uses the latter for its
speed-scaled watchdog, so these captures cannot exactly replay that policy.
"""
from __future__ import annotations

import argparse
import base64
import re
import struct
import zlib
from pathlib import Path


HEADER = re.compile(
    r"^INTERVALTAIL n=(\d+) skipped=(\d+) total=(\d+) fields=([^\r\n]+)$", re.M
)


def decode(text: str) -> list[dict[str, int]]:
    match = HEADER.search(text)
    if not match:
        raise ValueError("missing INTERVALTAIL header")
    count, first, total = map(int, match.groups()[:3])
    fields = match.group(4)
    version = 3 if "event_limit_us+origin2" in fields else (2 if "average_interval_half_us" in fields else 1)
    tag = {1: "IT85 ", 2: "IT86 ", 3: "IT87 "}[version]
    per_row = 4 if version == 1 else 2
    if count != min(total, 128) or first != total - count:
        raise ValueError("inconsistent interval-tail bounds")
    events: list[dict[str, int]] = []
    for line in text[match.end() :].splitlines():
        if not line.startswith(tag):
            if events:
                break
            continue
        raw = base64.a85decode(line[len(tag) :].encode("ascii"))
        if len(raw) != 24 or zlib.crc32(raw[:20]) != int.from_bytes(raw[20:], "little"):
            raise ValueError("invalid IT85 record")
        words = struct.unpack("<10H", raw[:20])
        ordinal = words[0] | words[1] << 16
        if ordinal != first + len(events):
            raise ValueError("non-contiguous IT85 ordinal")
        for index in range(per_row):
            if len(events) >= count:
                break
            if version == 1:
                gap, measured = words[2 + 2 * index : 4 + 2 * index]
                event = {
                    "ordinal": ordinal + index,
                    "gap_us": gap,
                    "measured_interval_half_us": measured,
                }
            else:
                gap, measured, average, packed_limit = words[2 + 4 * index : 6 + 4 * index]
                limit = packed_limit & 0x3FFF if version == 3 else packed_limit
                event = {
                    "ordinal": ordinal + index,
                    "gap_us": gap,
                    "measured_interval_half_us": measured,
                    "average_interval_half_us": average,
                    "event_limit_us": limit,
                }
                if version == 3:
                    event["origin"] = packed_limit >> 14
            events.append(event)
    if len(events) != count:
        raise ValueError(f"expected {count} events, decoded {len(events)}")
    return events


def complete_cycles(events: list[dict[str, int]]) -> list[dict[str, object]]:
    """Return ordinal-aligned groups containing exactly all six sector visits."""
    grouped: dict[int, list[dict[str, int]]] = {}
    for event in events:
        grouped.setdefault(event["ordinal"] // 6, []).append(event)
    cycles = []
    for ordinal, group in grouped.items():
        if len(group) != 6:
            continue
        group.sort(key=lambda event: event["ordinal"])
        if [event["ordinal"] % 6 for event in group] != list(range(6)):
            raise ValueError("non-contiguous electrical cycle")
        cycles.append(
            {
                "cycle_ordinal": ordinal,
                "period_us": sum(event["gap_us"] for event in group),
                "gaps_us": tuple(event["gap_us"] for event in group),
            }
        )
    return cycles


def speed_event_limit_us(average_half_us: int) -> int:
    if average_half_us >= 667:
        return 1000
    return max(200, (average_half_us * 3 + 1) >> 1)


def speed_watch_violations(events: list[dict[str, int]]) -> list[dict[str, int]]:
    """Return exact pre-accept deadline violations from an IT86-v2 capture."""
    if any("event_limit_us" not in event for event in events):
        raise ValueError("IT85 does not record the speed-watch deadline")
    return [
        event
        for event in events
        if event["ordinal"] != 0 and event["gap_us"] > event["event_limit_us"]
    ]


def verify_speed_watch(
    events: list[dict[str, int]], final_limit_us: int | None = None
) -> dict[str, int]:
    """Verify IT86's recorded pre-event deadline state transition by transition."""
    if not events or any("event_limit_us" not in event for event in events):
        raise ValueError("IT86 speed-watch fields required")
    for previous, current in zip(events, events[1:]):
        if previous["gap_us"] > previous["event_limit_us"]:
            raise ValueError("accepted events continued after a stale-event violation")
        expected = previous["event_limit_us"]
        average = previous["average_interval_half_us"]
        if average >= 64:
            expected = min(expected, speed_event_limit_us(average))
        if current["event_limit_us"] != expected:
            raise ValueError(
                f"deadline transition {previous['ordinal']}->{current['ordinal']}: "
                f"expected {expected}, got {current['event_limit_us']}"
            )
    last = events[-1]
    expected_final = last["event_limit_us"]
    if last["gap_us"] <= expected_final and last["average_interval_half_us"] >= 64:
        expected_final = min(
            expected_final, speed_event_limit_us(last["average_interval_half_us"])
        )
    if final_limit_us is not None and final_limit_us != expected_final:
        raise ValueError(
            f"final deadline expected {expected_final}, got {final_limit_us}"
        )
    return {
        "events": len(events),
        "transitions": len(events) - 1,
        "violations": len(speed_watch_violations(events)),
        "expected_final_limit_us": expected_final,
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("capture", type=Path)
    args = parser.parse_args()
    events = decode(args.capture.read_text(encoding="utf-8", errors="replace"))
    if events:
        cycles = complete_cycles(events)
        print(
            f"events={len(events)} complete_cycles={len(cycles)} "
            f"gap_min_us={min(e['gap_us'] for e in events)} "
            f"gap_max_us={max(e['gap_us'] for e in events)} "
            f"measured_min_half_us={min(e['measured_interval_half_us'] for e in events)} "
            f"measured_max_half_us={max(e['measured_interval_half_us'] for e in events)}"
        )
        if cycles:
            periods = [cycle["period_us"] for cycle in cycles]
            print(
                f"cycle_first_us={periods[0]} cycle_last_us={periods[-1]} "
                f"cycle_min_us={min(periods)} cycle_max_us={max(periods)}"
            )
        if "event_limit_us" in events[0]:
            watch = verify_speed_watch(events)
            print(
                f"watch_transitions={watch['transitions']} "
                f"watch_violations={watch['violations']} "
                f"expected_final_limit_us={watch['expected_final_limit_us']} "
                f"limit_min_us={min(e['event_limit_us'] for e in events)} "
                f"average_min_half_us={min(e['average_interval_half_us'] for e in events)} "
                f"average_max_half_us={max(e['average_interval_half_us'] for e in events)}"
            )


if __name__ == "__main__":
    main()
