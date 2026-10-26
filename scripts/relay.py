#!/usr/bin/env python3
"""Motor-routing relay board (COM19, 115200): one motor, two ESC boards.

All four relays switch together: OFF routes the motor to the L431, ON to
binz (mapped 2026-10-25 by a 15 % hold on each board in each position).

    python scripts/relay.py status          # print the board the motor is on
    python scripts/relay.py set l431|binz   # switch, then verify with `status`
    python scripts/relay.py check l431|binz # exit 1 unless routed to that board

Every exchange has a hard deadline; a missing prompt, a partial `status`
or relays that disagree with each other is an error, never a guess.
"""

from __future__ import annotations

import argparse
import re
import sys
import time

import serial

PORT = "COM19"
BAUD = 115200
DEADLINE_S = 3.0
ROUTE = {"l431": "OFF", "binz": "ON"}
STATE = re.compile(r"Relay (\d): (ON|OFF)")


class RelayError(RuntimeError):
    pass


def _cmd(s: serial.Serial, line: str) -> str:
    s.reset_input_buffer()
    s.write((line + "\n").encode())
    buf = b""
    end = time.monotonic() + DEADLINE_S
    while time.monotonic() < end:
        buf += s.read(256)
        if buf.rstrip().endswith(b">"):
            return buf.decode(errors="replace")
    raise RelayError(f"no prompt after {line!r} within {DEADLINE_S} s (got {buf!r})")


def _open() -> serial.Serial:
    s = serial.Serial(PORT, BAUD, timeout=0.05, write_timeout=1.0)
    # A fresh port can hold a partial line: flush the board's parser with a
    # bare newline (answered by a prompt) before the first real command.
    try:
        _cmd(s, "")
    except RelayError:
        pass
    return s


def routed(s: serial.Serial) -> str:
    text = _cmd(s, "status")
    states = dict(STATE.findall(text))
    if sorted(states) != ["1", "2", "3", "4"]:
        raise RelayError(f"incomplete status: {text!r}")
    vals = set(states.values())
    if len(vals) != 1:
        raise RelayError(f"relays disagree (motor split between boards): {states}")
    v = vals.pop()
    return next(b for b, want in ROUTE.items() if want == v)


def set_board(board: str) -> str:
    with _open() as s:
        _cmd(s, f"all {ROUTE[board].lower()}")
        time.sleep(0.2)  # contacts settle
        got = routed(s)
    if got != board:
        raise RelayError(f"asked for {board}, status says {got}")
    return got


def current_board() -> str:
    with _open() as s:
        return routed(s)


def main() -> int:
    ap = argparse.ArgumentParser()
    sp = ap.add_subparsers(dest="cmd", required=True)
    sp.add_parser("status")
    for c in ("set", "check"):
        p = sp.add_parser(c)
        p.add_argument("board", choices=sorted(ROUTE))
    a = ap.parse_args()
    try:
        if a.cmd == "status":
            print(f"motor -> {current_board()}")
        elif a.cmd == "set":
            print(f"motor -> {set_board(a.board)}")
        else:
            got = current_board()
            print(f"motor -> {got}")
            if got != a.board:
                return 1
    except (RelayError, serial.SerialException) as e:
        print(f"relay: {e}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
