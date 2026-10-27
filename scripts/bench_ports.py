#!/usr/bin/env python3
"""Resolve bench serial ports by USB identity (COM numbers move on replug).

    python scripts/bench_ports.py          # print the map
    from bench_ports import port; port("bf")

Roles (2026-10-27 FC bench, see docs/gc_LAB_NOTEBOOK.md):
  bf     F411 Discovery Betaflight USB CLI/MSP   0483:5740
  l431   L431 PB6 log + PA0 soft-UART (CH340)    1a86:7523
  binz   binz G071 bench UART (FTDI)             0403:6001 AH02LR5AA
  relay  motor-routing relay board (CP210x)      10c4:ea60
A missing or ambiguous role is an error, never a guess.
"""
from __future__ import annotations

import sys

import serial.tools.list_ports

ROLES = {
    "bf": (0x0483, 0x5740, None),
    "l431": (0x1A86, 0x7523, None),
    "binz": (0x0403, 0x6001, "AH02LR5AA"),
    "relay": (0x10C4, 0xEA60, None),
}


def port(role: str) -> str:
    vid, pid, ser = ROLES[role]
    hits = [
        p.device
        for p in serial.tools.list_ports.comports()
        if p.vid == vid and p.pid == pid and (ser is None or p.serial_number == ser)
    ]
    if len(hits) != 1:
        raise RuntimeError(f"bench port '{role}' ({vid:04x}:{pid:04x}): found {hits}")
    return hits[0]


def main() -> int:
    bad = 0
    for role in ROLES:
        try:
            print(f"{role:6s} {port(role)}")
        except RuntimeError as e:
            print(e)
            bad = 1
    return bad


if __name__ == "__main__":
    sys.exit(main())
