#!/usr/bin/env python3
"""Send bench verbs over the PA0 soft-UART link and capture replies.

Two-baud protocol (see softuart_check.py): each token's text is sent
at 9600 (the ESC's PA0 RX rate), then the port switches to 115200 to
capture the ESC's PB6 debug output for the token's delay window.

Vocabulary (rm32::bench_input): `c` eeprom hex dump, `<n>o` latch
config offset, `<n>v` write value via the A4 ring, `S` save settings,
`i` info line, `B` recorder dump, `w` kill.

Usage:
    softuart_cmd.py [--port COM41] TOKEN:DELAY [TOKEN:DELAY ...]
    softuart_cmd.py --set FIELD=VALUE [--save]     # by field name
    softuart_cmd.py --get FIELD [FIELD ...]        # from the 'c' dump
Example — persist round trip on offset 32:
    softuart_cmd.py c:3 32o:1 129v:1 S:3 c:3 32o:1 128v:1 S:3 c:3
"""
import argparse
import re
import sys
import time

import serial

# Byte offsets into EepromConfig (rm32/src/config.rs, repr(C) all-u8;
# == the persisted EEPROM page layout). Anchored against a live dump:
# motor_poles=14 at [27], servo cal 128/128/128/50 at [32..36].
FIELDS = {
    "eeprom_version": 1, "version_major": 3, "version_minor": 4,
    "max_ramp": 5, "minimum_duty_cycle": 6, "disable_stick_calibration": 7,
    "absolute_voltage_cutoff": 8, "current_p": 9, "current_i": 10,
    "current_d": 11, "active_brake_power": 12,
    "dir_reversed": 17, "bi_direction": 18, "use_sine_start": 19,
    "comp_pwm": 20, "variable_pwm": 21, "stuck_rotor_protection": 22,
    "advance_level": 23, "pwm_frequency": 24, "startup_power": 25,
    "motor_kv": 26, "motor_poles": 27, "brake_on_stop": 28,
    "stall_protection": 29, "beep_volume": 30, "telemetry_on_interval": 31,
    "servo_low_threshold": 32, "servo_high_threshold": 33,
    "servo_neutral": 34, "servo_dead_band": 35,
    "low_voltage_cut_off": 36, "low_cell_volt_cutoff": 37,
    "rc_car_reverse": 38, "sine_mode_changeover_throttle_level": 40,
    "drag_brake_strength": 41, "driving_brake_strength": 42,
    "temperature_limit": 43, "current_limit": 44, "sine_mode_power": 45,
    "input_type": 46, "auto_advance": 47,
}

EEP_RE = re.compile(r"\[eep (\d+)\] ((?:[0-9a-f]{2} )+)")


def run_tokens(p, tokens, raw=False, quiet=False):
    """Send each TOKEN:DELAY; return all captured display lines."""
    captured = []
    for tok in tokens:
        text, _, delay = tok.rpartition(":")
        delay = float(delay)
        p.baudrate = 9_600
        time.sleep(0.05)
        # Trailing newline terminates any pending accumulator state.
        p.write(text.encode() + b"\n")
        p.flush()
        time.sleep(0.05 + 0.002 * len(text))
        p.baudrate = 115_200
        p.reset_input_buffer()  # drop garbled-while-9600 input
        deadline = time.time() + delay
        buf = b""
        while time.time() < deadline:
            buf += p.read(4096)
        if not quiet:
            print(f">>> {text!r}")
        for line in buf.decode("ascii", "replace").splitlines():
            line = line.strip()
            if not line:
                continue
            keep = raw or line.startswith(("[eep", "[cfg", "[i ", "[su",
                                           "SR", "[bench", "[su]"))
            if keep:
                captured.append(line)
                if not quiet:
                    print(f"    {line}")
    return captured


def read_eeprom_bytes(p):
    """Run 'c' and return {offset: value} for every dumped byte."""
    lines = run_tokens(p, ["c:4"], quiet=True)
    out = {}
    for line in lines:
        m = EEP_RE.search(line)
        if not m:
            continue
        base = int(m.group(1))
        for i, hx in enumerate(m.group(2).split()):
            out[base + i] = int(hx, 16)
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", default="COM41")
    ap.add_argument("tokens", nargs="*", metavar="TOKEN:DELAY")
    ap.add_argument("--raw", action="store_true",
                    help="print all captured lines, not just []-tagged ones")
    ap.add_argument("--set", metavar="FIELD=VALUE",
                    help="write a config byte by field name (A4 ring)")
    ap.add_argument("--save", action="store_true",
                    help="with --set: also save to EEPROM and verify")
    ap.add_argument("--get", nargs="+", metavar="FIELD",
                    help="read fields from the persisted EEPROM dump")
    a = ap.parse_args()

    p = serial.Serial(a.port, 115_200, timeout=0.05)
    rc = 0
    try:
        time.sleep(0.5)
        p.reset_input_buffer()

        if a.set:
            field, _, value = a.set.partition("=")
            off = FIELDS[field]
            value = int(value, 0)
            toks = [f"{off}o:1.5", f"{value}v:1.5"]
            if a.save:
                toks.append("S:3")
            run_tokens(p, toks)
            if a.save:
                got = read_eeprom_bytes(p).get(off)
                ok = got == value
                print(f"{field}[{off}] persisted={got} "
                      f"{'OK' if ok else 'MISMATCH'}")
                rc = 0 if ok else 1
        elif a.get:
            eep = read_eeprom_bytes(p)
            if not eep:
                print("no [eep] dump captured")
                rc = 1
            for field in a.get:
                off = FIELDS[field]
                print(f"{field}[{off}] = {eep.get(off)}")
        else:
            run_tokens(p, a.tokens, raw=a.raw)
    finally:
        p.baudrate = 115_200
        p.close()
    return rc


if __name__ == "__main__":
    sys.exit(main())
