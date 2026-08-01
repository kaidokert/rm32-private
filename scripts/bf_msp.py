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

SCARS: (1) BF's MSP2 inline dshot-command path acks but never puts the
command on the wire on this build — use CLI `dshotprog 0 <cmd>` for
bench command delivery instead. (2) Interleaving MSP_MOTOR (104)
requests into the --arm-fly RC stream reproducibly PREVENTS ARMING
(2/2 fail with it, 2/2 arm without); poll MSP_MOTOR_TELEMETRY (139)
and MSP_STATUS (101) only.

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


MSP_STATUS = 101
MSP_MOTOR_TELEMETRY = 139


def parse_motor_telemetry(payload):
    """MSP_MOTOR_TELEMETRY reply: u8 count, then per motor
    u32 rpm, u16 invalidPct*100, u8 tempC, u16 voltage, u16 current,
    u16 consumption (13 bytes each). Returns motor 0's dict."""
    if not payload or len(payload) < 1 + 13:
        return None
    rpm, inv, temp, volt, curr, cons = struct.unpack_from("<IHBHHH", payload, 1)
    return {"rpm": rpm, "inv": inv / 100.0, "temp": temp,
            "volt": volt, "curr": curr, "cons": cons}


def read_msp_replies(p, want_cmd, quiet=0.3):
    """Collect raw MSP v1 reply payloads for want_cmd from the stream."""
    buf = b""
    t = time.time()
    out = []
    while time.time() - t < quiet:
        buf += p.read(4096)
    i = 0
    while True:
        j = buf.find(b"$M>", i)
        if j < 0 or j + 5 > len(buf):
            break
        size = buf[j + 3]
        cmd = buf[j + 4]
        end = j + 5 + size + 1
        if end > len(buf):
            break
        if cmd == want_cmd:
            out.append(buf[j + 5:j + 5 + size])
        i = j + 3
    return out


def arm_probe(port, hold_s=6.0):
    """Three-phase arming-flags probe: RC stream + MSP_STATUS interleave.
    Prints the raw STATUS payload per phase — the arming-disable word
    identifies itself by which bits clear as RC/AUX conditions change."""
    p = serial.Serial(port, 115_200, timeout=0.05)
    phases = []
    try:
        for name, aux, secs in (
            ("stream-noarm", 1000, 5.0),
            ("stream-arm", 1900, hold_s),
            ("stream-disarm", 1000, 2.0),
        ):
            t0 = time.time()
            last = None
            while time.time() - t0 < secs:
                p.write(rc_frame(1000, aux))
                p.write(msp1_frame(MSP_STATUS))
                time.sleep(0.05)
                got = read_msp_replies(p, MSP_STATUS, quiet=0.05)
                if got:
                    last = got[-1]
            phases.append((name, last))
    finally:
        p.close()
    for name, payload in phases:
        print(f"{name:14} {(payload.hex() if payload else 'NO REPLY')}")
    return 0


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", default="COM42")
    ap.add_argument("--cmd", type=int, help="DSHOT command 0-47")
    ap.add_argument("--motor", type=int, default=255, help="motor index, 255=all")
    ap.add_argument("--repeat", type=int, default=6,
                    help="sends (AM32 needs 6 for non-beacon commands)")
    ap.add_argument("--arm-test", action="store_true",
                    help="MSP RC injection flight-arm cycle")
    ap.add_argument("--arm-probe", action="store_true",
                    help="3-phase arming-flags probe (MSP_STATUS in-stream)")
    ap.add_argument("--arm-fly", type=float, metavar="PCT",
                    help="MSP flight: arm, raise throttle to PCT for --hold, disarm")
    ap.add_argument("--hold", type=float, default=8.0)
    a = ap.parse_args()
    if a.arm_fly is not None:
        # Full flight-realistic loop: stream disarmed, arm via AUX, ramp
        # the RC THROTTLE channel (BF mixes -> motor), hold, throttle
        # down, disarm. EDT activates via BF's own arm-time cmd-13 burst.
        thr = int(1000 + a.arm_fly * 10)
        p = serial.Serial(a.port, 115_200, timeout=0.05)
        try:
            for name, throttle, aux, secs in (
                ("prestream", 1000, 1000, 5.0),
                ("arm", 1000, 1900, 3.0),
                ("fly", thr, 1900, a.hold),
                ("throttle-down", 1000, 1900, 2.0),
                ("disarm", 1000, 1000, 2.0),
            ):
                print(f"[fly] {name}", flush=True)
                t0 = time.time()
                last_telem = None
                last_cmd = 0
                while time.time() - t0 < secs:
                    p.write(rc_frame(throttle, aux))
                    p.write(msp1_frame(MSP_MOTOR_TELEMETRY))
                    time.sleep(0.05)
                    got = read_msp_replies(p, MSP_MOTOR_TELEMETRY, quiet=0.02)
                    if got:
                        t = parse_motor_telemetry(got[-1])
                        if t:
                            last_telem = t
                if last_telem:
                    print(f"[fly] {name}: telem {last_telem}", flush=True)
        finally:
            # Kill guard: disarm stream then close (stream loss => BF
            # failsafe also disarms).
            try:
                t0 = time.time()
                while time.time() - t0 < 1.0:
                    p.write(rc_frame(1000, 1000))
                    time.sleep(0.05)
            finally:
                p.close()
        return 0
    if a.arm_probe:
        return arm_probe(a.port, a.hold)
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
