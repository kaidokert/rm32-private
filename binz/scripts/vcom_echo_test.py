"""VCOM echo verification for binz examples/vcom-echo.rs on NUCLEO-G071RB.

Opens the ST-Link V2-1 VCP (COM7, 2 Mbaud), then checks:
  1. boot banner (reset the board with this script running to see it),
  2. single-line round trip,
  3. 16 KiB full-duplex blast, byte-exact, with sustained rate.

Usage: python scripts/vcom_echo_test.py [PORT] [BAUD]
"""

import sys
import threading
import time

import serial

PORT = sys.argv[1] if len(sys.argv) > 1 else "COM7"
BAUD = int(sys.argv[2]) if len(sys.argv) > 2 else 2_000_000


def main():
    s = serial.Serial(PORT, BAUD, timeout=0.05)
    time.sleep(0.2)
    banner = s.read(200)
    print("banner:", repr(banner) if banner else "<none - board already running>")

    s.reset_input_buffer()
    s.write(b"hello, board!\n")
    time.sleep(0.15)
    echo = s.read(64)
    print("line echo:", repr(echo), "OK" if echo == b"hello, board!\n" else "FAIL")

    payload = bytes(range(256)) * 64  # 16 KiB, all byte values
    s.reset_input_buffer()
    rx = bytearray()
    t0 = time.time()
    quiet = None
    first = None
    w = threading.Thread(target=lambda: (s.write(payload), s.flush()))
    w.start()
    while time.time() - t0 < 20:
        chunk = s.read(4096)
        if chunk:
            first = first or time.time()
            rx.extend(chunk)
            quiet = None
        elif len(rx) >= len(payload):
            break
        else:
            quiet = quiet or time.time()
            if time.time() - quiet > 3:
                break
    w.join()
    ok = bytes(rx) == payload
    rate = len(rx) / (time.time() - (3 if quiet else 0) - first) if first else 0
    print(f"blast: {len(rx)}/{len(payload)} full_match={ok} rate={rate:.0f} B/s")
    s.close()
    sys.exit(0 if ok and echo == b"hello, board!\n" else 1)


if __name__ == "__main__":
    main()
