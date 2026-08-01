#!/usr/bin/env python3
"""MSP v2 client for Betaflight — the DSHOT-command side channel.

Sends MSP2_SEND_DSHOT_COMMAND (0x3003) so bench scripts can deliver
DSHOT commands 0-47 to the ESC through BF without the CLI (and without
BF's motor-init timing): beacons, direction, save-settings, and any
future rm32 bench verbs mapped onto unassigned command ids.

MSP2 frame: $ X < flag(0) cmd(u16le) size(u16le) payload crc8_dvb_s2
(crc over flag..payload). BF's handler payload: commandType(u8: 0 =
inline/blocking), motorIndex(u8: 255 = all motors), commandCount(u8),
then commandCount x command(u8).

The FC must NOT be in CLI mode (MSP is dead there). ESC must be armed
at zero throttle for command acceptance (AM32 gate: armed && !running).

Usage: bf_msp.py --cmd 1 [--repeat 6] [--port COM42] [--motor 255]
"""
import argparse
import struct
import sys
import time

import serial

MSP2_SEND_DSHOT_COMMAND = 0x3003


def crc8_dvb_s2(data):
    crc = 0
    for b in data:
        crc ^= b
        for _ in range(8):
            crc = ((crc << 1) ^ 0xD5) & 0xFF if crc & 0x80 else (crc << 1) & 0xFF
    return crc


def msp2_frame(cmd, payload=b""):
    body = struct.pack("<BHH", 0, cmd, len(payload)) + payload
    return b"$X<" + body + bytes([crc8_dvb_s2(body)])


def send_dshot_command(port, command, motor=255, count=1, quiet=0.2):
    payload = struct.pack("<BBB", 0, motor, 1) + bytes([command])
    p = serial.Serial(port, 115_200, timeout=0.05)
    try:
        for _ in range(count):
            p.write(msp2_frame(MSP2_SEND_DSHOT_COMMAND, payload))
            time.sleep(quiet)
        # Read back any MSP ack frames (informational).
        resp = p.read(256)
        return resp
    finally:
        p.close()


MSP_SET_RAW_RC = 200  # MSP v1


def msp1_frame(cmd, payload=b""):
    body = bytes([len(payload), cmd]) + payload
    crc = 0
    for b in body:
        crc ^= b
    return b"$M<" + body + bytes([crc])


def rc_frame(throttle=1000, aux1=1000):
    # AETR + 4 aux, center sticks, given throttle/aux1.
    ch = [1500, 1500, throttle, 1500, aux1, 1000, 1000, 1000]
    return msp1_frame(MSP_SET_RAW_RC, struct.pack("<8H", *ch))


def arm_test(port, hold_s=8.0):
    """Flight-arm BF via MSP RC injection: stream disarmed RC, raise
    AUX1 (arm switch), hold, drop AUX1, stop. BF failsafes if the
    stream dies — motors stop. Requires: feature RX_MSP + an
    `aux 0 0 0 1700 2100` arm range, set via CLI beforehand."""
    p = serial.Serial(port, 115_200, timeout=0.05)
    try:
        t0 = time.time()
        while time.time() - t0 < 7.0:  # failsafe recovery needs seconds
            p.write(rc_frame(1000, 1000))
            time.sleep(0.05)
        print("[arm] raising AUX1", flush=True)
        t0 = time.time()
        while time.time() - t0 < hold_s:
            p.write(rc_frame(1000, 1900))
            time.sleep(0.05)
        print("[arm] disarming", flush=True)
        t0 = time.time()
        while time.time() - t0 < 1.0:
            p.write(rc_frame(1000, 1000))
            time.sleep(0.05)
    finally:
        p.close()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", default="COM42")
    ap.add_argument("--cmd", type=int, help="DSHOT command 0-47")
    ap.add_argument("--motor", type=int, default=255, help="motor index, 255=all")
    ap.add_argument("--repeat", type=int, default=6,
                    help="sends (AM32 needs 6 for non-beacon commands)")
    ap.add_argument("--arm-test", action="store_true",
                    help="MSP RC injection flight-arm cycle")
    ap.add_argument("--hold", type=float, default=8.0)
    a = ap.parse_args()
    if a.arm_test:
        arm_test(a.port, a.hold)
        return 0
    if a.cmd is None:
        print("--cmd or --arm-test required")
        return 1
    resp = send_dshot_command(a.port, a.cmd, a.motor, a.repeat)
    print(f"sent cmd {a.cmd} x{a.repeat} to motor {a.motor}; "
          f"ack bytes: {len(resp)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
