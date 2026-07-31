#!/usr/bin/env python3
"""Jitter analog autopsy: J-armed climb to the wall on the VERBATIM
detector; the FIRST late window freezes the WAXWING ring (arc + comp
VALUE bit around the late accept). Kill, dump frozen ring + late log.

Decode with wax_decode/wax_plot on the saved capture.
Usage: jitter_autopsy.py [port]
"""
import re
import sys
import time

from bench_lib import Bench, reset_board


def main():
    port = sys.argv[1] if len(sys.argv) > 1 else "COM41"
    identity, _ = reset_board(port)
    if identity != "rm32":
        print(f"ABORT identity={identity}")
        return 1
    froze = False
    with Bench(port) as b:
        if not b.engage_from_stop(55):
            print("engage failed")
            return 1
        b.cmd(b"J", settle=0.3)   # arm WAXWING recording
        for p in (70, 80, 90, 93, 96):
            b.hold(p, 2.2)
        print(f"96%: {b.info()}")
        # ride toward the wall; the first late window freezes the ring
        t0 = time.time()
        while time.time() - t0 < 8.0:
            b.hold(100, 0.8)
            inf = b.info(retries=1)
            if inf is None:
                continue
            seg = "".join(chr(x) if 32 <= x < 127 else "."
                          for x in b.buf[-600:])
            if "FROZEN" in seg or inf.dsy > 5:
                froze = True
                print(f"[event] {inf}")
                break
    print("killed")
    if not froze:
        print("no event captured")
        return 1
    # dump frozen WAX + late log at idle
    import serial
    p = serial.Serial(port, 2_000_000, timeout=0.05)
    p.reset_input_buffer()
    p.write(b"L")
    p.flush()
    raw = bytearray()
    t0 = time.time()
    while time.time() - t0 < 2.0:
        raw += p.read(200000)
        if b"LW END" in raw:
            break
    p.write(b"x")
    p.flush()
    t0 = time.time()
    while time.time() - t0 < 6.0:
        c = p.read(200000)
        if c:
            raw += c
        if b"WX END" in raw:
            break
    p.write(b"w")
    p.flush()
    p.close()
    open("../captures/jitter_autopsy.bin", "wb").write(bytes(raw))
    txt = raw.decode("latin1", errors="replace")
    lw = re.findall(r"^LW .*$", txt, re.M)
    print(f"captured {len(raw)}B; late-log lines: {len(lw)}")
    for line in lw[:10]:
        print(" ", line)
    print("frozen WX present:", "WX n=" in txt,
          "| decode: python wax_decode.py ../captures/jitter_autopsy.bin")
    return 0


if __name__ == "__main__":
    sys.exit(main())
