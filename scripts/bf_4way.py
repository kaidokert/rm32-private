#!/usr/bin/env python3
"""Read AM32/rm32 ESC identity + parameters through Betaflight 4-way
passthrough — the same path am32.ca uses (MSP_SET_4WAY_IF, then BLHeli
4-way frames bridged by the FC to the ESC bootloader's half-duplex
serial on the signal wire).

Protocol reference: am32-configurator src/communication/four_way.ts
(frame 0x2F cmd addrH addrL len params crc16-xmodem; reply 0x2E ...
params ack crc), src/mcu.ts (signature -> eeprom offset, LAYOUT_SIZE
0xB8, filename string in the 32 bytes below the EEPROM offset).

Usage:
  python bf_4way.py [--bf COM42] [--target 0] [--no-exit]

The FC reboots on exit so its motor output resumes and the ESC
bootloader jumps back to the app.
"""

import argparse
import struct
import sys
import time

import serial

from softuart_cmd import FIELDS  # offset table, single source of truth

MSP_SET_4WAY_IF = 245

CMDS = {
    "InterfaceTestAlive": 0x30,
    "ProtocolGetVersion": 0x31,
    "InterfaceGetName": 0x32,
    "InterfaceGetVersion": 0x33,
    "InterfaceExit": 0x34,
    "DeviceReset": 0x35,
    "DeviceInitFlash": 0x37,
    "DeviceRead": 0x3A,
    "DeviceWrite": 0x3B,
}

ACK_NAMES = {
    0x00: "ACK_OK", 0x01: "ACK_I_UNKNOWN_ERROR", 0x02: "ACK_I_INVALID_CMD",
    0x03: "ACK_I_INVALID_CRC", 0x04: "ACK_I_VERIFY_ERROR",
    0x05: "ACK_D_INVALID_COMMAND", 0x06: "ACK_D_COMMAND_FAILED",
    0x07: "ACK_D_UNKNOWN_ERROR", 0x08: "ACK_I_INVALID_CHANNEL",
    0x09: "ACK_I_INVALID_PARAM", 0x0F: "ACK_D_GENERAL_ERROR",
}

MCU_VARIANTS = {  # am32.ca src/mcu.ts
    0x1F06: ("STM32F051", 0x7C00),
    0x3506: ("ARM64K", 0xF800),
    0x1506: ("NXP ESC_8KB_PAGE", 0xE000),
}
LAYOUT_SIZE = 0xB8
PORTS = "ABCDEFGHIJKLMNOPQRSTUVWXYZ"


def crc16_xmodem(data):
    crc = 0
    for b in data:
        crc ^= b << 8
        for _ in range(8):
            crc = ((crc << 1) ^ 0x1021) & 0xFFFF if crc & 0x8000 else (crc << 1) & 0xFFFF
    return crc


def msp1_frame(cmd, payload=b""):
    hdr = bytes([len(payload), cmd]) + payload
    csum = 0
    for b in hdr:
        csum ^= b
    return b"$M<" + hdr + bytes([csum])


def fourway_frame(cmd, params=b"\x00", address=0):
    body = bytes([0x2F, cmd, (address >> 8) & 0xFF, address & 0xFF,
                  0 if len(params) == 256 else len(params)]) + params
    crc = crc16_xmodem(body)
    return body + bytes([crc >> 8, crc & 0xFF])


def read_fourway_reply(p, deadline_s=1.5):
    """Read one 0x2E-framed reply; returns (cmd, address, params, ack)."""
    buf = b""
    t0 = time.time()
    while time.time() - t0 < deadline_s:
        buf += p.read(256)
        i = buf.find(b"\x2e")
        if i < 0:
            continue
        if len(buf) < i + 5:
            continue
        n = buf[i + 4] or 256
        need = i + 5 + n + 3  # params + ack + crc16
        if len(buf) < need:
            continue
        frame = buf[i:need]
        ack = frame[5 + n]
        crc_rx = (frame[6 + n] << 8) | frame[7 + n]
        if crc16_xmodem(frame[: 6 + n]) != crc_rx:
            raise IOError("4way reply CRC mismatch")
        return frame[1], (frame[2] << 8) | frame[3], frame[5 : 5 + n], ack
    raise TimeoutError("no 4way reply")


def cmd4(p, name, params=b"\x00", address=0, retries=6, deadline=1.5):
    for attempt in range(retries):
        p.reset_input_buffer()
        p.write(fourway_frame(CMDS[name], params, address))
        try:
            cmd, addr, out, ack = read_fourway_reply(p, deadline)
        except (TimeoutError, IOError) as e:
            if attempt == retries - 1:
                raise
            time.sleep(0.25)
            continue
        if ack == 0:
            return out
        if attempt == retries - 1:
            raise IOError(f"{name}: {ACK_NAMES.get(ack, hex(ack))}")
        time.sleep(0.25)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--bf", default="COM42")
    ap.add_argument("--target", type=int, default=0)
    ap.add_argument("--no-exit", action="store_true",
                    help="stay in passthrough (no FC reboot)")
    ap.add_argument("--set", action="append", default=[], metavar="FIELD=VAL",
                    help="read-modify-write a settings byte (repeatable); "
                         "dir_reversed is preserved unless explicitly set")
    a = ap.parse_args()

    p = serial.Serial(a.bf, 115_200, timeout=0.3)
    try:
        # Enter 4-way passthrough (am32.ca step 1)
        p.write(msp1_frame(MSP_SET_4WAY_IF))
        time.sleep(0.4)
        p.reset_input_buffer()

        name = cmd4(p, "InterfaceGetName")
        ver = cmd4(p, "InterfaceGetVersion")
        proto = cmd4(p, "ProtocolGetVersion")
        print(f"interface: {name.decode('ascii', 'replace')} "
              f"v{ver[0]}.{ver[1] if len(ver) > 1 else 0} protocol={proto[0]}")

        # DeviceInitFlash resets the ESC into its bootloader and probes it.
        # First contact can be slow (ESC signal-timeout + reboot) — long
        # deadline + retries mirror am32.ca's initRetries behavior.
        info = cmd4(p, "DeviceInitFlash", bytes([a.target]), retries=10,
                    deadline=3.0)
        signature = (info[1] << 8) | info[0]
        boot_input = info[2]
        iface_mode = info[3] if len(info) > 3 else None
        mcu_name, eep = MCU_VARIANTS.get(signature, (f"UNKNOWN sig", None))
        pin = f"P{PORTS[boot_input >> 4]}{boot_input & 0xF}" \
            if (boot_input >> 4) < len(PORTS) else hex(boot_input)
        print(f"ESC #{a.target}: signature=0x{signature:04X} ({mcu_name}) "
              f"bootloader_pin={pin} interface_mode={iface_mode}")
        if eep is None:
            sys.exit("unknown signature — no EEPROM offset")

        # File name string sits in the 32 bytes below the EEPROM offset.
        raw = cmd4(p, "DeviceRead", bytes([32]), eep - 32)
        fname = raw.split(b"\x00")[0].decode("ascii", "replace")
        print(f"firmware file: {fname!r}")

        # Full settings layout.
        settings = cmd4(p, "DeviceRead", bytes([LAYOUT_SIZE]), eep)

        if a.set:
            block = bytearray(settings)
            dir_before = block[FIELDS["dir_reversed"]]
            for kv in a.set:
                fname_, _, val = kv.partition("=")
                off = FIELDS[fname_]
                print(f"set {fname_}[{off}]: {block[off]} -> {int(val)}")
                block[off] = int(val) & 0xFF
            if block[FIELDS["dir_reversed"]] != dir_before and not any(
                    kv.startswith("dir_reversed=") for kv in a.set):
                sys.exit("refusing implicit dir_reversed change")
            # am32.ca writeSettings: DeviceWrite the full block, then verify.
            cmd4(p, "DeviceWrite", bytes(block), eep, retries=4, deadline=3.0)
            back = cmd4(p, "DeviceRead", bytes([LAYOUT_SIZE]), eep)
            if bytes(back) != bytes(block):
                sys.exit("write verify FAILED")
            print("write verified; dir_reversed preserved =",
                  back[FIELDS["dir_reversed"]])
            settings = back
        print(f"\nEEPROM @0x{eep:04X} ({LAYOUT_SIZE} bytes):")
        for i in range(0, LAYOUT_SIZE, 16):
            chunk = settings[i : i + 16]
            print(f"  {eep + i:04x}: {chunk.hex(' ')}")

        print("\ndecoded parameters:")
        print(f"  {'boot_byte':28s} [  0] = {settings[0]}")
        ver_maj, ver_min = settings[1], settings[2]
        print(f"  {'version':28s} [1,2] = {ver_maj}.{ver_min}")
        for fname_, off in sorted(FIELDS.items(), key=lambda kv: kv[1]):
            if off < len(settings):
                print(f"  {fname_:28s} [{off:3d}] = {settings[off]}")
    finally:
        try:
            if not a.no_exit:
                p.write(fourway_frame(CMDS["InterfaceExit"]))
                time.sleep(0.5)
                p.reset_input_buffer()
                # FC reboot so motor output resumes and the ESC's
                # bootloader jumps back into the app.
                p.write(b"#\n")
                time.sleep(1.0)
                p.write(b"exit\n")
                time.sleep(0.3)
        except Exception:
            pass
        p.close()


if __name__ == "__main__":
    main()
