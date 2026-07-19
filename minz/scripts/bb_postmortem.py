"""Post-mortem black-box read: decode the bb ring straight out of RAM
via probe-rs, immune to the UART-dump losses that ate most fatal bb
dumps on 2026-07-18 (script abort windows + binary interleave).

POST-MORTEM ONLY: probe-rs read halts the core briefly. Run this
after a kill (motor off) - never against a spinning motor (the
halt perturbs live timing; see the bench notes on probe-rs reads).

The ring persists after death by construction: post-kill no ISR
writes events, so even a thawed ring holds the final 64 events.

Usage:
  python scripts/bb_postmortem.py            # read + decode
  python scripts/bb_postmortem.py --elf path/to/elf
"""

import argparse
import pathlib
import re
import subprocess

PROBE = "0483:374f:0037002F3234510836303532"
CHIP = "STM32L431KCUx"
BB_LEN = 64
# Keep in sync with minz_core::blackbox::EV_NAMES.
EV_NAMES = ["REF", "BLD", "DRK", "ACC", "NOZ", "DIS", "DSY", "ENG",
            "STV", "RAQ", "RSD", "RSC"]

DEFAULT_ELF = (pathlib.Path(__file__).resolve().parent.parent
               / "target/thumbv7em-none-eabihf/release/examples/motor_tester2")


def sym_addrs(elf):
    out = subprocess.run(
        ["arm-none-eabi-nm", str(elf)], capture_output=True, text=True,
        check=True).stdout
    want = {}
    for name in ["BB_T", "BB_TYPE", "BB_SEC", "BB_DATA", "BB_IDX"]:
        # Rust-mangled statics end <name>17h<hash>E[.0]; the old bare
        # prefix match confused BB_T with BB_TYPE (odd-address read).
        m = re.search(rf"^([0-9a-f]+) [bBdD] .*{name}17h[0-9a-f]+E(\.0)?$",
                      out, re.M)
        if not m:
            m = re.search(rf"^([0-9a-f]+) [bBdD] .*{name}$", out, re.M)
        if not m:
            raise SystemExit(f"symbol {name} not found in {elf}")
        want[name] = int(m.group(1), 16)
    return want


def rd(addr, words, width):
    out = subprocess.run(
        ["probe-rs", "read", "--chip", CHIP, "--probe", PROBE,
         f"b{width}", f"0x{addr:08x}", str(words)],
        capture_output=True, text=True, check=True).stdout
    vals = []
    for tok in out.split():
        try:
            vals.append(int(tok, 16))
        except ValueError:
            pass
    return vals


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--elf", default=str(DEFAULT_ELF))
    a = ap.parse_args()
    s = sym_addrs(a.elf)
    t = rd(s["BB_T"], BB_LEN, 16)
    ty = rd(s["BB_TYPE"], BB_LEN, 8)
    sec = rd(s["BB_SEC"], BB_LEN, 8)
    data = rd(s["BB_DATA"], BB_LEN, 16)
    idx = rd(s["BB_IDX"], 1, 32)[0] % BB_LEN
    print(f"# bb post-mortem (idx={idx}); oldest -> newest, dt in 10us "
          f"ticks between events")
    prev_t = None
    for k in range(BB_LEN):
        i = (idx + k) % BB_LEN
        if ty[i] == 0xFF:
            continue
        name = EV_NAMES[ty[i]] if ty[i] < len(EV_NAMES) else f"?{ty[i]}"
        dt = (t[i] - prev_t) & 0xFFFF if prev_t is not None else 0
        prev_t = t[i]
        print(f"bb +{dt*10:6d}us {name} s{sec[i]} d={data[i]}")


if __name__ == "__main__":
    main()
