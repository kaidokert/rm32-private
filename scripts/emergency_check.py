#!/usr/bin/env python3
"""Check emergency_off on hardware: bridge pins and MOE before and after a panic.

    python scripts/emergency_check.py l431|binz

Needs a test image flashed: right after the reset-cause prints in bin/main.rs,
before the watchdog starts, it must put the bridge in complementary mode at
zero duty (low sides on, no current into a stopped motor), set MOE (and gate
ENABLE on binz), spin a while, then panic. That image is never committed. Reflash
the normal image afterwards.

Silent, no throttle. Fails loudly: exit 1 unless the pre-panic state really is
AF + MOE and the post-panic state really is GPIO output, latches low, MOE off.
"""
from __future__ import annotations

import subprocess
import sys
import threading
import time

import serial

from bench_ports import port

BOARDS = {
    "l431": dict(chip="STM32L431KCUx", probe="0483:374f:0037002F3234510836303532",
                 speed=["--speed", "100"], gpio=0x4800_0000, baud=115200),
    "binz": dict(chip="STM32G071RBTx", probe="0483:374b:066CFF343433464757233430",
                 speed=[], gpio=0x5000_0000, baud=115200),
}
TIM1_BDTR = 0x4001_2C44
TIM1_CCR1 = 0x4001_2C34
GPIO_STRIDE = 0x400
PA_PINS = (7, 8, 9, 10)
PB_PINS = (0, 1)
BINZ_ENABLE = (3, 1)  # PD1


def read(b: dict, addr: int, n: int = 1) -> list[int]:
    out = subprocess.run(
        ["probe-rs", "read", *b["speed"], "--chip", b["chip"], "--probe", b["probe"],
         "b32", hex(addr), str(n)],
        capture_output=True, text=True, timeout=30, check=True).stdout
    return [int(w, 16) for w in out.split() if len(w) == 8 or w.startswith("0x")][-n:]


def snapshot(name: str, b: dict) -> dict:
    g = b["gpio"]
    s = {
        "moder_a": read(b, g)[0],
        "odr_a": read(b, g + 0x14)[0],
        "moder_b": read(b, g + GPIO_STRIDE)[0],
        "odr_b": read(b, g + GPIO_STRIDE + 0x14)[0],
        "bdtr": read(b, TIM1_BDTR)[0],
        "ccr": read(b, TIM1_CCR1, 3),
    }
    if name == "binz":
        s["odr_d"] = read(b, g + BINZ_ENABLE[0] * GPIO_STRIDE + 0x14)[0]
    return s


def modes(s: dict) -> set[int]:
    return {(s["moder_a"] >> (2 * p)) & 3 for p in PA_PINS} | {
        (s["moder_b"] >> (2 * p)) & 3 for p in PB_PINS}


def show(tag: str, s: dict) -> None:
    print(tag, {k: ([hex(x) for x in v] if isinstance(v, list) else hex(v)) for k, v in s.items()})


def main() -> int:
    name = sys.argv[1]
    b = BOARDS[name]
    log: list[bytes] = []
    stop = threading.Event()
    ser = serial.Serial(port(name), b["baud"], timeout=0.2)

    def reader() -> None:
        while not stop.is_set():
            log.append(ser.read(256))

    t = threading.Thread(target=reader, daemon=True)
    t.start()
    subprocess.run(["probe-rs", "reset", *b["speed"], "--chip", b["chip"], "--probe", b["probe"]],
                   capture_output=True, timeout=60, check=True)
    t0 = time.time()
    time.sleep(8)
    pre = snapshot(name, b)
    show("pre ", pre)
    deadline = time.time() + 240  # asm::delay runs long on flash wait states
    while b"PANIC" not in b"".join(log) and time.time() < deadline:
        time.sleep(1)
    print(f"panic seen after {time.time() - t0:.0f} s" if b"PANIC" in b"".join(log) else "no panic in 240 s")
    time.sleep(1)
    post = snapshot(name, b)
    show("post", post)
    stop.set()
    t.join()
    ser.close()
    text = b"".join(log).decode("ascii", "replace")
    panic_lines = [l for l in text.splitlines() if "PANIC" in l]
    print("log:", panic_lines or "NO PANIC LINE")

    fails = []
    if modes(pre) != {2}:
        fails.append("pre: bridge pins not all in AF, test proves nothing")
    if not pre["bdtr"] & (1 << 15):
        fails.append("pre: MOE not set, test proves nothing")
    if any(pre["ccr"]):
        fails.append("pre: duty not zero")
    if modes(post) != {1}:
        fails.append("post: bridge pins not all GPIO output")
    if any(post["odr_a"] & (1 << p) for p in PA_PINS) or any(post["odr_b"] & (1 << p) for p in PB_PINS):
        fails.append("post: an output latch is high")
    if post["bdtr"] & (1 << 15):
        fails.append("post: MOE still set")
    if name == "binz":
        if not pre["odr_d"] & (1 << BINZ_ENABLE[1]):
            fails.append("pre: gate ENABLE low, test proves nothing")
        if post["odr_d"] & (1 << BINZ_ENABLE[1]):
            fails.append("post: gate ENABLE still high")
    if not panic_lines:
        fails.append("no PANIC line on the log")
    print("verdict:", "PASS" if not fails else "FAIL")
    for f in fails:
        print("  -", f)
    return 1 if fails else 0


if __name__ == "__main__":
    sys.exit(main())
