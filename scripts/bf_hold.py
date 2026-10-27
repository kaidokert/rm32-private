#!/usr/bin/env python3
"""One Betaflight motor hold on one board, with closed-loop evidence.

Routes the motor to the board through the relay (refuses if the relay
check fails), tails the board's debug log during the hold, drives
`bf_motor.py --telem` (BF eRPM read mid-hold) and prints:

    bf:  <BF dshot_telemetry_info row for that motor>
    run: <the ESC's [run] session summary>
    verdict: CLOSED-LOOP | CHOKE | NO-RUN-LINE

CLOSED-LOOP = exactly one start and zc_pk >= 1000 (the protocol_matrix
criterion). The whole ESC log goes to --log.

Usage: bf_hold.py BOARD PCT [--seconds 6] [--log FILE] [--stay]
       BOARD = l431 (BF motor 0) | binz (BF motor 1)
"""
import argparse
import re
import subprocess
import sys
import threading
import time
from pathlib import Path

import serial

HERE = Path(__file__).parent
sys.path.insert(0, str(HERE))
from bench_ports import port  # noqa: E402

INDEX = {"l431": 0, "binz": 1}
RUN_RE = re.compile(r"\[run\] starts=(\d+) ms=\d+ zc_pk=(\d+)")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("board", choices=sorted(INDEX))
    ap.add_argument("pct", type=float)
    ap.add_argument("--seconds", type=float, default=6.0)
    ap.add_argument("--log", default=None)
    ap.add_argument("--stay", action="store_true",
                    help="no FC reboot after the hold (see bf_motor.py)")
    a = ap.parse_args()
    if not 0 < a.pct <= 100:
        ap.error("pct must be in (0, 100]")
    py = sys.executable
    relay = subprocess.run([py, str(HERE / "relay.py"), "set", a.board],
                           capture_output=True, text=True)
    check = subprocess.run([py, str(HERE / "relay.py"), "check", a.board],
                           capture_output=True, text=True)
    if relay.returncode or check.returncode:
        print(f"relay refused: {relay.stdout}{relay.stderr}{check.stdout}{check.stderr}")
        return 2
    time.sleep(0.5)

    chunks = []
    stop = threading.Event()
    esc = serial.Serial(port(a.board), 115_200, timeout=0.1)

    def tail():
        while not stop.is_set():
            chunks.append(esc.read(8192))

    th = threading.Thread(target=tail)
    th.start()
    try:
        time.sleep(1.0)
        idx = INDEX[a.board]
        cmd = [py, str(HERE / "run_capped.py"), "--secs", "60", "--", py,
               str(HERE / "bf_motor.py"), "--index", str(idx), "--pct",
               str(a.pct), "--seconds", str(a.seconds), "--telem"]
        if a.stay:
            cmd.append("--stay")
        r = subprocess.run(cmd, capture_output=True, text=True)
        # [run] prints 0.1 s after the stop, or on the ESC's reset after
        # the FC exit-reboot.
        time.sleep(4.0)
    finally:
        stop.set()
        th.join()
        esc.close()
    raw = b"".join(chunks).decode("ascii", "replace")
    if a.log:
        Path(a.log).write_text(raw, encoding="utf-8")
    bf_rows = [ln for ln in r.stdout.splitlines()
               if ln.strip().startswith(f"{idx + 1} ")]
    runs = [ln[ln.index("[run]"):] for ln in raw.splitlines() if "[run]" in ln]
    print(f"bf:  {bf_rows[-1].strip() if bf_rows else '(no BF row) ' + r.stdout[-200:]}")
    for ln in runs:
        print(f"run: {ln}")
    if not runs:
        print("verdict: NO-RUN-LINE")
        return 1
    m = RUN_RE.search(runs[0])
    ok = m and int(m.group(1)) == 1 and int(m.group(2)) >= 1000
    print(f"verdict: {'CLOSED-LOOP' if ok else 'CHOKE'}")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
