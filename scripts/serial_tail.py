#!/usr/bin/env python3
"""Dumb timestamped serial tail — the BF-phase log watcher.

The benchuart toolchain talks 2M with a command channel; the BF-phase
build (debuguart only) is a TX-only 115200 log on PB6. This just opens
the port at the requested baud and timestamps every line, so boot
banners, reset causes, [loop] heartbeats, and protocol-detection flags
are visible without any command traffic. Ctrl-C or --seconds to stop.
"""
import argparse
import sys
import time

import serial


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("port", nargs="?", default="COM41")
    ap.add_argument("--baud", type=int, default=115_200)
    ap.add_argument("--seconds", type=float, default=0, help="0 = forever")
    a = ap.parse_args()
    p = serial.Serial(a.port, a.baud, timeout=0.05)
    t0 = time.time()
    buf = b""
    try:
        while not a.seconds or time.time() - t0 < a.seconds:
            buf += p.read(4096)
            while b"\n" in buf:
                line, buf = buf.split(b"\n", 1)
                txt = line.decode("ascii", "replace").rstrip()
                if txt:
                    safe = txt.encode("ascii", "replace").decode("ascii")
                    print(f"[{time.time()-t0:7.2f}] {safe}", flush=True)
    except KeyboardInterrupt:
        pass
    finally:
        p.close()
    return 0


if __name__ == "__main__":
    sys.exit(main())
