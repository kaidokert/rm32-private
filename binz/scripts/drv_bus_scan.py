"""Decode the diagnostic 16-frame DMA bus tail (BS85), including CRC."""

from __future__ import annotations

import argparse
import base64
import re
import struct
import zlib
from pathlib import Path

from drv_interval_tail import decode as decode_events


HEADER = re.compile(r"^BUSSCAN n=(\d+) total=(\d+) fields=([^\r\n]+)$", re.M)
BASELINE = re.compile(r"^FASTBUS baseline_bus=(\d+) baseline_vref=(\d+)", re.M)
LAST_EVENT = re.compile(r"^BUSSCAN [^\r\n]*last_event_us=(\d+)", re.M)


def decode(text: str) -> list[dict[str, int]]:
    match = HEADER.search(text)
    if not match:
        raise ValueError("missing BUSSCAN header")
    count, total = map(int, match.groups()[:2])
    if count != min(total, 16) or "stamp_lo,stamp_hi,bus,vref,ia,ib,ic,duty" not in match[3]:
        raise ValueError("inconsistent BUSSCAN header")
    rows: list[dict[str, int]] = []
    for line in text[match.end() :].splitlines():
        if not line.startswith("BS85 "):
            if rows:
                break
            continue
        raw = base64.a85decode(line[5:].encode("ascii"))
        if len(raw) != 20 or zlib.crc32(raw[:16]) != int.from_bytes(raw[16:], "little"):
            raise ValueError("invalid BS85 CRC or length")
        lo, hi, bus, vref, ia, ib, ic, duty = struct.unpack("<8H", raw[:16])
        rows.append(
            dict(stamp_us=lo | hi << 16, bus=bus, vref=vref,
                 ia=ia, ib=ib, ic=ic, duty=duty)
        )
    if len(rows) != count:
        raise ValueError(f"expected {count} BS85 frames, decoded {len(rows)}")
    if any(b["stamp_us"] <= a["stamp_us"] for a, b in zip(rows, rows[1:])):
        raise ValueError("BUSSCAN stamps not strictly increasing")
    return rows


def timestamp_events(events: list[dict[str, int]], last_event_us: int) -> list[dict[str, int]]:
    """IT86/IT87 gap belongs to the event at its end; anchor the final event exactly."""
    stamp = last_event_us
    stamped = []
    for event in reversed(events):
        stamped.append(dict(event, stamp_us=stamp))
        stamp -= event["gap_us"]
    stamped.reverse()
    return stamped


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("capture", type=Path)
    args = parser.parse_args()
    text = args.capture.read_text(encoding="utf-8", errors="replace")
    rows = decode(text)
    baseline = BASELINE.search(text)
    reference = tuple(map(int, baseline.groups())) if baseline else None
    for row in rows:
        ratio = (
            100 * row["bus"] * reference[1] / (reference[0] * row["vref"])
            if reference and row["vref"] else float("nan")
        )
        print(
            f"stamp_us={row['stamp_us']} bus={row['bus']} vref={row['vref']} "
            f"reference_pct={ratio:.2f} ia={row['ia']} ib={row['ib']} "
            f"ic={row['ic']} duty={row['duty']}"
        )
    anchor = LAST_EVENT.search(text)
    # BUSSCAN is useful on its own (including early ADC faults).  Only align
    # events when an optional IT86/IT87 recorder was included in this report.
    if anchor and rows and re.search(r"^INTERVALTAIL ", text, re.M):
        events = timestamp_events(decode_events(text), int(anchor[1]))
        for event in events:
            if rows[0]["stamp_us"] <= event["stamp_us"] <= rows[-1]["stamp_us"]:
                print(
                    f"event_us={event['stamp_us']} ordinal={event['ordinal']} "
                    f"gap_us={event['gap_us']} average_half_us={event['average_interval_half_us']}"
                    + (f" origin={event['origin']}" if 'origin' in event else '')
                )


if __name__ == "__main__":
    main()
