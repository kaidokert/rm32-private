#!/usr/bin/env python3
"""The discriminator: verbatim detector, no J, 100% hold ABORTED at the
FIRST desync — then dump the late-window log. Answers: do >1.5x late
windows exist BEFORE the first detector fire (real initiator), or only
after the kick (the response manufactures its own 'initiator', and the
fci values were kick-contaminated)?

Usage: verbatim_lw.py [port]
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
    first_dsy_at = None
    with Bench(port) as b:
        if not b.engage_from_stop(55):
            print("engage failed")
            return 1
        # verbatim detector, no J — pure production config
        for p in (70, 80, 90, 93, 96):
            b.hold(p, 2.2)
        s = b.info()
        print(f"96%: {s}")
        t0 = time.time()
        while time.time() - t0 < 12.0:
            b.hold(100, 0.4)
            inf = b.info(retries=1)
            if inf is None:
                continue
            if inf.dsy > 0:
                first_dsy_at = time.time() - t0
                print(f"[first dsy] t={first_dsy_at:.2f}s {inf}")
                break
        txt_all = "".join(chr(x) if 32 <= x < 127 or x == 10 else "."
                           for x in b.buf)
        import re as _re
        ff = _re.findall(r"\[ff [^\]]+\]", txt_all)
        if ff:
            print("FORENSICS:", ff[-1])
    print("killed (aborted at first fire)")
    # dump LW at idle
    import serial
    p = serial.Serial(port, 2_000_000, timeout=0.05)
    p.reset_input_buffer()
    p.write(b"L")
    p.flush()
    raw = bytearray()
    t0 = time.time()
    while time.time() - t0 < 2.5:
        raw += p.read(200000)
        if b"LW END" in raw:
            break
        time.sleep(0.1)
    p.write(b"w")
    p.flush()
    p.close()
    txt = raw.decode("latin1", errors="replace")
    lw = re.findall(r"^LW (\d+) (\d+)$", txt, re.M)
    n = re.search(r"LW n=(\d+)", txt)
    print(f"late-window log (lifetime n={n.group(1) if n else '?'}):")
    for t_ms, tz in lw:
        v = int(tz)
        kind = "EARLY" if v & 0x8000 else "late "
        print(f"  t={int(t_ms)/1000:8.3f}s  {kind} tz={v & 0x7FFF}")
    print("\nREADING: if ALL events cluster AT/after the first-dsy moment")
    print("=> the response manufactures the late windows (fci was kick-")
    print("contaminated; the true initiator is subtler than 1.5x).")
    print("If events precede it => real pre-fire late windows exist.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
