#!/usr/bin/env python3
"""3D-mode per-direction ladder with dual-source speed evidence.

For each direction (BF 3D CLI mapping: 1500 center, deadband
~1406-1514) steps rungs of the half-range and captures, per rung:
  - the ESC's internal view via the PA0 soft-UART 'i' probe:
    fwd flag, e_com_time (eRPM_int = 60e6/ecom), ci, duty, adj
  - BF's GCR-decode view: dshot_telemetry_info eRPM
The cross-check separates real overspeed (both agree) from telemetry
artifacts (they disagree). Post-run [sr] gives dsy/exc for the
raggedness question. Kill guard: motor 1500 (3D stop) on every exit.

Usage: dir3d_ladder.py [--bf COM42] [--esc COM41] [--rungs 15,30,45,60,75]
"""
import argparse
import re
import subprocess
import sys
import time

import serial

import softuart_cmd as su

MOTOR_RE = re.compile(r"^\s*1\s+\S+\s+(\d+)\s+(\d+)", re.M)
I_RE = re.compile(
    r"\[i mode=(\S+) newinput=(\d+) adj=(\d+) fwd=(\d) ecom=(\d+) "
    r"ci=(\d+) duty=(\d+) zc=(\d+) vbat_mv=(\d+)\]"
)
SR_RE = re.compile(r"dsy=(\d+) exc=(\d+) wex=(\d+) cm=(\d+)")

DB_LO, DB_HI, R_MIN, F_MAX = 1406, 1514, 1000, 2000


def cli(cmd):
    r = subprocess.run([sys.executable, "bf_cli.py", cmd, "--stay"],
                       capture_output=True, text=True)
    return r.stdout


def esc_probe(p):
    """Two-baud 'i' round trip on the shared adapter; returns dict."""
    lines = su.run_tokens(p, ["i:2.5"], quiet=True)
    for line in reversed(lines):
        m = I_RE.search(line)
        if m:
            ecom = int(m.group(5))
            return {
                "mode": m.group(1), "adj": int(m.group(3)),
                "fwd": int(m.group(4)), "ecom": ecom,
                "erpm_int": (60_000_000 // ecom) if ecom else 0,
                "ci": int(m.group(6)), "duty": int(m.group(7)),
                "vbat": int(m.group(9)),
            }
    return None


def bf_erpm():
    m = MOTOR_RE.search(cli("dshot_telemetry_info"))
    return (int(m.group(1)), int(m.group(2))) if m else (None, None)


def read_sr(p):
    lines = su.run_tokens(p, ["i:0.1"], quiet=True)  # just to flush
    deadline = time.time() + 10
    buf = ""
    p.baudrate = 115_200
    while time.time() < deadline:
        buf += p.read(4096).decode("ascii", "replace")
        m = None
        for m in SR_RE.finditer(buf):
            pass
        if m:
            return {"dsy": int(m.group(1)), "exc": int(m.group(2)),
                    "cm": int(m.group(4))}
    return None


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--bf", default="COM42")
    ap.add_argument("--esc", default="COM41")
    ap.add_argument("--rungs", default="15,30,45,60,75")
    ap.add_argument("--hold", type=float, default=2.0)
    a = ap.parse_args()
    rungs = [int(x) for x in a.rungs.split(",")]

    p = serial.Serial(a.esc, 115_200, timeout=0.05)
    rows = []
    t0 = time.time()

    def say(m):
        print(f"[t+{time.time()-t0:5.1f}s] {m}", flush=True)

    try:
        pre = read_sr(p)
        say(f"pre counters: {pre}")
        for name, sign in (("REVERSE", -1), ("FORWARD", +1)):
            say(f"{name} ladder")
            for pct in rungs:
                if sign < 0:
                    v = DB_LO - int((DB_LO - R_MIN) * pct / 100)
                else:
                    v = DB_HI + int((F_MAX - DB_HI) * pct / 100)
                cli(f"motor 0 {v}")
                time.sleep(a.hold)
                probe = esc_probe(p)
                erpm_bf, rpm_bf = bf_erpm()
                rows.append((name, pct, v, probe, erpm_bf, rpm_bf))
                pr = (f"fwd={probe['fwd']} adj={probe['adj']} "
                      f"duty={probe['duty']} eRPM_int={probe['erpm_int']} "
                      f"ci={probe['ci']} vbat={probe['vbat']}"
                      if probe else "NO PROBE")
                say(f"  {pct:3d}% (v={v}): {pr} | BF eRPM={erpm_bf}")
            cli("motor 0 1500")
            say(f"{name} done, neutral 4s")
            time.sleep(4)
        # direction-flip-through-neutral transitions
        say("flip transitions: R30 -> neutral -> F30 -> neutral -> R30")
        for v in (1284, 1500, 1660, 1500, 1284, 1500):
            cli(f"motor 0 {v}")
            time.sleep(2.0)
        post = read_sr(p)
        say(f"post counters: {post}")
    finally:
        try:
            # 3D KILL GUARD: 1500 = neutral/stop. NEVER send motor 1000
            # here — in BF-3D that is FULL REVERSE (learned the hard
            # way: a 1000-teardown left BF streaming max reverse; only
            # the ESC's refuse-to-arm-at-nonzero-throttle gate saved
            # the bench).
            cli("motor 0 1500")
        finally:
            p.baudrate = 115_200
            p.close()

    print("\n=== 3D LADDER ===")
    for name, pct, v, probe, erpm_bf, rpm_bf in rows:
        if probe:
            ratio = (erpm_bf / probe["erpm_int"]) if (erpm_bf and probe["erpm_int"]) else 0
            print(f"  {name:7} {pct:3d}%  fwd={probe['fwd']} adj={probe['adj']:4d} "
                  f"duty={probe['duty']:4d} int={probe['erpm_int']:6d} "
                  f"bf={erpm_bf} ratio={ratio:.2f} vbat={probe['vbat']}")
        else:
            print(f"  {name:7} {pct:3d}%  NO PROBE  bf={erpm_bf}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
