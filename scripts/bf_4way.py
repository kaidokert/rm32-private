#!/usr/bin/env python3
"""AM32-Configurator passthrough transport check — headless.

The web Configurator drives BF's 4-way interface; this exercises the
same path without a browser: MSP_SET_4WAY_IF (245) switches the FC
port into 4-way mode, then 4-way frames (0x2F cmd addr16 len params
crc16-xmodem) talk THROUGH the FC to the ESC's bootloader:

  InterfaceTestAlive (0x30)  — FC alive in 4-way mode
  ProtocolGetVersion (0x31)  — 4-way protocol version
  DeviceInitFlash    (0x37)  — FC resets the ESC into its BOOTLOADER
                               and returns the device signature — the
                               full host->MSP->4way->signal-wire->
                               bootloader->back round trip
  InterfaceExit      (0x34)  — leave 4-way; the ESC's (patched) boot-
                               loader auto-boots the app on idle line

A valid signature = the Configurator transport works end-to-end; the
browser UI is just a client of exactly this chain.

Usage: bf_4way.py [--port COM42]
"""
import argparse
import struct
import sys
import time

import serial

MSP_SET_4WAY_IF = 245


def crc8_dvb_s2(data):
    crc = 0
    for b in data:
        crc ^= b
        for _ in range(8):
            crc = ((crc << 1) ^ 0xD5) & 0xFF if crc & 0x80 else (crc << 1) & 0xFF
    return crc


def msp1_frame(cmd, payload=b""):
    body = bytes([len(payload), cmd]) + payload
    crc = 0
    for b in body:
        crc ^= b
    return b"$M<" + body + bytes([crc])


def crc16_xmodem(data):
    crc = 0
    for b in data:
        crc ^= b << 8
        for _ in range(8):
            crc = ((crc << 1) ^ 0x1021) & 0xFFFF if crc & 0x8000 else (crc << 1) & 0xFFFF
    return crc


def fourway_frame(cmd, addr=0, params=b"\x00"):
    body = bytes([0x2F, cmd, (addr >> 8) & 0xFF, addr & 0xFF, len(params) & 0xFF]) + params
    crc = crc16_xmodem(body)
    return body + struct.pack(">H", crc)


def fourway_read(p, quiet=1.2):
    buf = b""
    t = time.time()
    while time.time() - t < quiet:
        chunk = p.read(4096)
        if chunk:
            buf += chunk
            t = time.time()
        # complete frame? 0x2E cmd addr16 len params ack crc16
        i = buf.find(b"\x2e")
        if i >= 0 and len(buf) >= i + 5:
            plen = buf[i + 4] or 256
            need = i + 5 + plen + 1 + 2
            if len(buf) >= need:
                fr = buf[i:need]
                return {
                    "cmd": fr[1],
                    "addr": (fr[2] << 8) | fr[3],
                    "params": fr[5:5 + plen],
                    "ack": fr[5 + plen],
                }
    return None


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", default="COM42")
    a = ap.parse_args()
    p = serial.Serial(a.port, 115_200, timeout=0.05)
    ok = False
    try:
        # Enter 4-way mode; MSP reply payload = ESC count.
        p.write(msp1_frame(MSP_SET_4WAY_IF))
        time.sleep(0.4)
        pre = p.read(4096)
        print(f"[4way] enter reply bytes: {pre.hex() if pre else 'none'}")

        p.write(fourway_frame(0x30))  # InterfaceTestAlive
        r = fourway_read(p)
        print(f"[4way] TestAlive: {r}")

        p.write(fourway_frame(0x31))  # ProtocolGetVersion
        r = fourway_read(p)
        print(f"[4way] GetVersion: {r}")

        # DeviceInitFlash esc#0 — resets the ESC into its bootloader and
        # returns the device signature (THE round trip). The first
        # attempt races the ESC's signal-timeout self-reset (~0.5-2 s
        # to bootloader) — retry like the Configurator does.
        for attempt in range(4):
            p.write(fourway_frame(0x37, params=b"\x00"))
            r = fourway_read(p, quiet=3.0)
            print(f"[4way] DeviceInitFlash try {attempt + 1}: {r}")
            if r and r["ack"] == 0 and len(r["params"]) >= 3:
                sig = r["params"].hex()
                print(f"[4way] ESC BOOTLOADER SIGNATURE: {sig}")
                ok = True
                break
            time.sleep(2.0)
    finally:
        try:
            p.write(fourway_frame(0x34))  # InterfaceExit
            time.sleep(0.5)
            print("[4way] exited 4-way mode")
        finally:
            p.close()
    print(f"verdict: {'PASS — Configurator transport end-to-end' if ok else 'CHECK'}")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
