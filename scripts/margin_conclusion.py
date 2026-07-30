#!/usr/bin/env python3
"""One-shot margin-theory conclusion runner.

Sequence (run when bench power returns):
  1. verify target powered (probe-rs sees the M4)
  2. flash bootloader + current rm32 build (advance lever + void autopsy)
  3. pack health check (bench_status semantics)
  4. advance A/B: BASE / ADV8 / ADV24  (advance_ab.run_arm)
  5. one BASE storm rep reading the [void ...] snapshot line — the
     initiator discriminator (EXTI masked vs comparator pinned)

Prints a final verdict block. Aborts (pack/identity) are marked
clearly and never counted as evidence.
"""
import re
import subprocess
import sys
import time

from advance_ab import run_arm
from bench_lib import Bench, reset_board

PROBE = "0483:374f:0037002F3234510836303532"
CHIP = "STM32L431KCUx"
BOOTLOADER = "E:/m/robot/esc/AM32-bootloader/obj/AM32_L431_BOOTLOADER_PA2_V18.bin"
ELF = "E:/m/robot/esc/rm32/rm32_stm32/target/thumbv7em-none-eabihf/release/rm32_firmware"


def sh(args):
    return subprocess.run(args, capture_output=True, text=True)


def main():
    port = sys.argv[1] if len(sys.argv) > 1 else "COM41"

    r = sh(["probe-rs", "info", "--probe", PROBE])
    if "Cortex-M4" not in (r.stdout + r.stderr):
        print("ABORT: target not powered/visible on SWD")
        return 1
    print("1) target alive")

    for label, args in (("bootloader", ["--binary-format", "bin",
                                        "--base-address", "0x08000000", BOOTLOADER]),
                        ("rm32 ELF", ["--binary-format", "elf", ELF])):
        r = sh(["probe-rs", "download", "--chip", CHIP, "--probe", PROBE] + args)
        if "Finished" not in (r.stdout + r.stderr):
            print(f"ABORT: flash {label} failed:\n{r.stdout}\n{r.stderr}")
            return 1
    print("2) flashed bootloader + rm32")

    identity, causes = reset_board(port)
    if identity != "rm32":
        print(f"ABORT: identity={identity} causes={causes}")
        return 1
    with Bench(port) as b:
        b.hold(0, 1.0)
        inf = b.info()
    if inf is None or inf.volts < 11.6:
        print(f"ABORT: pack not healthy at rest "
              f"({inf.volts if inf else '?'} V < 11.6)")
        return 1
    print(f"3) pack rest {inf.volts:.2f} V — healthy\n")

    results = {}
    for label in ("BASE", "ADV8", "ADV24"):
        print(f"== arm {label} ==")
        verdict, detail = run_arm(port, label, 8.0, watch_climb=(label == "ADV24"))
        results[label] = verdict
        print(f"  -> {verdict}  {detail}\n")
        time.sleep(5)

    # Void-snapshot rep only matters if BASE stormed (initiator present)
    void_line = None
    if results.get("BASE") == "STORM":
        print("== void-snapshot rep (BASE, read [void ...]) ==")
        identity, _ = reset_board(port)
        if identity == "rm32":
            with Bench(port) as b:
                if b.engage_from_stop(55):
                    for pct in (70, 80, 90, 93, 96):
                        b.hold(pct, 2.2)
                    t0 = time.time()
                    while time.time() - t0 < 5.0:
                        b.hold(100, 0.8)
                        n = len(b.buf)
                        b.cmd(b"i", settle=0.4)
                        seg = "".join(chr(x) if 32 <= x < 127 or x == 10 else "."
                                      for x in b.buf[n:])
                        m = re.search(r"\[void [^\]]+\]", seg)
                        if m:
                            void_line = m.group(0)
                            print(f"  {void_line}")
                            break
        print()

    print("=" * 60)
    print("MARGIN-THEORY VERDICT")
    print(f"  BASE  (adv 16): {results.get('BASE')}")
    print(f"  ADV8  (later commutation, +margin): {results.get('ADV8')}")
    print(f"  ADV24 (earlier, -margin): {results.get('ADV24')}")
    print(f"  void snapshot: {void_line or '(none captured)'}")
    print()
    b_, a8, a24 = (results.get(k) for k in ("BASE", "ADV8", "ADV24"))
    if b_ == "STORM" and a8 == "HELD" and a24 in ("STORM", "STORM-CLIMB"):
        print("  => CONFIRMED both directions: wall is commutation-timing")
        print("     (demag) margin. Fix = advance reduction at top duty.")
    elif b_ == "STORM" and a8 == "HELD":
        print("  => margin fix WORKS (ADV8 holds 100%); ADV24 leg "
              f"inconclusive ({a24}).")
    elif b_ == "STORM" and a8 == "STORM":
        print("  => margin theory FALSIFIED (later commutation does not "
              "move the wall). Initiator hunt continues via void snapshot.")
    else:
        print("  => inconclusive/aborted arms — do not draw conclusions.")
    print("  CPU margin: NO (settled independently: clone padding gate,")
    print("  on-time fire latency, and the void's entries=0).")
    return 0


if __name__ == "__main__":
    sys.exit(main())
