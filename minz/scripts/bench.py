"""THE bench driver - one parametrized tool, no more inline scripts.

Subcommands:
  engage                 Y-engage (R6 polling start), report f_e
  ladder [--top 90] [--hold 30]
                         autonomous firmware ladder + RAM readback
  readback               ladder RAM log only (post-run)
  postmortem             bb ring + key counters via probe
  flash-minz             build+flash the minz example
  flash-am32 [--trace]   bootloader+app+eeprom+spray
  sweep-am32 [--lo 50 --hi 100 --step 5]
                         AM32 throttle sweep with KISS readout
  kill                   motor off

Conventions baked in once: doubled-key protocol ('w' single),
echo-verified stateful toggles, comms-delta liveness, probe serial.
"""

import argparse
import re
import subprocess
import sys
import time

PROBE = "0483:374f:0037002F3234510836303532"
CHIP = "STM32L431KCUx"
PORT = "COM41"
BAUD = 2_000_000
ELF = ("target/thumbv7em-none-eabihf/release/examples/motor_tester2")


def sh(*args, timeout=300):
    return subprocess.run(list(args), capture_output=True, text=True,
                          timeout=timeout).stdout


class Bench:
    def __init__(self):
        import serial
        self.ser = serial.Serial(PORT, BAUD, timeout=0.05)
        self.cap = bytearray()

    def close(self):
        self.ser.close()

    def key(self, k, wait):
        payload = k if k in ("w", "") else "".join(c + c for c in k)
        self.ser.write(payload.encode())
        self.ser.flush()
        t0 = time.monotonic()
        buf = b""
        while time.monotonic() - t0 < wait:
            b = self.ser.read(8192)
            buf += b
            self.cap.extend(b)
        return buf.decode("utf-8", "replace")

    def press_until(self, k, want, tries=10):
        for _ in range(tries):
            if want in self.key(k, 0.6):
                return True
        return False

    def kill(self):
        self.key("w", 0.5)
        self.key("w", 0.3)

    def engage(self, attempts=10):
        self.kill()
        self.press_until("D", "AM32")
        self.press_until("M", "SWIFT")
        for att in range(attempts):
            self.key("Y", 5.0)
            m = re.search(r"cl: ACTIVE f_e=(\d+)Hz", self.key("i", 1.2))
            if m:
                return att + 1, int(m.group(1))
            self.key("w", 0.5)
            time.sleep(2.5)
        return None, None


def nm_addr(sym):
    out = sh("arm-none-eabi-nm", ELF)
    for ln in out.splitlines():
        if ln.strip().endswith(sym):
            return int(ln.split()[0], 16)
    raise SystemExit(f"symbol {sym} not found")


def probe_words(addr, n):
    out = sh("probe-rs", "read", "--chip", CHIP, "--probe", PROBE,
             "b32", hex(addr), str(n))
    return [int(x, 16) for ln in out.splitlines()
            for x in ln.split() if len(x) == 8]


def cmd_engage(_):
    b = Bench()
    try:
        att, fe = b.engage()
        print(f"engaged attempt {att}: {fe} Hz" if att else "NO ENGAGE")
    finally:
        b.kill()
        b.close()


def cmd_ladder(a):
    b = Bench()
    try:
        att, fe = b.engage()
        if not att:
            print("NO ENGAGE")
            return
        print(f"engaged attempt {att}: {fe} Hz; arming ladder")
        if not b.press_until("L", "auto-ladder = on"):
            print("ladder arm not confirmed (echo loss) - proceeding")
        wait = 0.55 * (a.top - 10) + a.hold
        print(f"climb + hold ~{wait:.0f}s, hands off the wire...")
        time.sleep(wait)
    finally:
        b.kill()
        b.close()
    cmd_readback(a)


def cmd_readback(_):
    base = nm_addr("LADDER_LOG")
    w = probe_words(base, 288)
    top = 0
    rows = []
    for amp in range(10, 96):
        iv, ir, cc = w[amp * 3], w[amp * 3 + 1], w[amp * 3 + 2]
        if iv or cc:
            top = amp
            rows.append((amp, iv, ir, cc))
    print(f"=== TOP RUNG {top} ===")
    print("amp | f_e Hz | isns_raw | comms")
    for amp, iv, ir, cc in rows:
        if amp % 5 == 0 or amp >= top - 3:
            print(f"{amp:3d} | {1e6/(6*iv) if iv else 0:6.0f} | {ir:4d} | {cc}")


def cmd_postmortem(_):
    print(sh(sys.executable, "scripts/bb_postmortem.py"))
    for sym in ("STORM_KILLS", "ZOMBIE_BACKSTOP_KILLS", "MAIN_STARVE_KILLS",
                "KEY_I_COUNT", "KEY_ANY_COUNT", "USART2_COUNT",
                "SHOT_ARMED_COUNT", "LPTIM2_COUNT", "BURST_TRIPS",
                "RECOV_COUNT", "RESEED_COUNT", "SLEW_CLAMP_COUNT",
                "WAIT_CLAMP_COUNT"):
        try:
            v = probe_words(nm_addr(sym), 1)[0]
            print(f"{sym} = {v}")
        except SystemExit:
            pass


def cmd_flash_minz(_):
    print(sh("cargo", "build", "--release", "--target",
             "thumbv7em-none-eabihf", "--example", "motor_tester2",
             timeout=300)[-500:])
    print(sh("probe-rs", "download", "--chip", CHIP, "--probe", PROBE,
             "--binary-format", "elf", ELF)[-200:])
    print(sh("probe-rs", "reset", "--chip", CHIP, "--probe", PROBE)[-100:])


def cmd_flash_am32(a):
    bl = "E:/m/robot/esc/AM32-bootloader/obj/AM32_L431_BOOTLOADER_PA2_V18.bin"
    app = ("../../AM32/obj/AM32_VIMDRONES_L431_ZCTRACE.bin" if a.trace
           else "../../AM32/obj/AM32_VIMDRONES_L431_NOTRACE.bin")
    ee = ("C:/Users/kaido/AppData/Local/Temp/claude/"
          "E--m-robot-esc-rm32-minz/0097c268-1af2-4a87-aa51-3e8f066c0026/"
          "scratchpad/eeprom_page.bin")
    for base, path in (("0x08000000", bl), ("0x08001000", app),
                       ("0x0800F800", ee)):
        print(sh("probe-rs", "download", "--chip", CHIP, "--probe", PROBE,
                 "--binary-format", "bin", "--base-address", base,
                 path)[-120:].strip())
    print(sh("probe-rs", "reset", "--chip", CHIP, "--probe", PROBE)[-80:])


def cmd_sweep_am32(a):
    sys.path.insert(0, "scripts")
    import serial

    import zctrace_capture as z
    from am32_census import bootloader_spray, paced_write
    ser = serial.Serial(z.PORT, z.BAUD, timeout=0.05)
    cap = bytearray()

    def hold(pct, secs):
        t0 = time.monotonic()
        last = 0.0
        while time.monotonic() - t0 < secs:
            if time.monotonic() - last > 0.8:
                paced_write(ser, f"{pct}\n".encode())
                last = time.monotonic()
            cap.extend(ser.read(8192))

    try:
        bootloader_spray(ser)
        time.sleep(0.5)
        ser.reset_input_buffer()
        hold(0, 5.0)
        for att in range(4):
            hold(a.lo, 4.0)
            fr = [f for f in z.parse_kiss(bytes(cap[-20000:]))
                  if 3.0 < f[1] < 12.0]
            if fr and fr[-1][4] > 18000:
                print(f"started at {a.lo}%: erpm {fr[-1][4]}")
                break
            paced_write(ser, b"0\n")
            time.sleep(2.0)
        else:
            raise SystemExit("AM32 no start")
        for pct in range(a.lo + a.step, a.hi + 1, a.step):
            hold(pct, 3.0)
            fr = [f for f in z.parse_kiss(bytes(cap[-20000:]))
                  if 3.0 < f[1] < 12.0]
            if fr:
                f0 = fr[-1]
                print(f"{pct:3d}%: erpm {f0[4]:6d}  amps {f0[2]:5.2f}  "
                      f"vbat {f0[1]:.2f}")
    finally:
        paced_write(ser, b"0\n")
        time.sleep(1.0)
        paced_write(ser, b"0\n")
        ser.close()


def cmd_kill(_):
    b = Bench()
    b.kill()
    b.close()
    print("killed")


def main():
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="cmd", required=True)
    sub.add_parser("engage")
    p = sub.add_parser("ladder")
    p.add_argument("--top", type=int, default=90)
    p.add_argument("--hold", type=float, default=30.0)
    sub.add_parser("readback")
    sub.add_parser("postmortem")
    sub.add_parser("flash-minz")
    p = sub.add_parser("flash-am32")
    p.add_argument("--trace", action="store_true")
    p = sub.add_parser("sweep-am32")
    p.add_argument("--lo", type=int, default=50)
    p.add_argument("--hi", type=int, default=100)
    p.add_argument("--step", type=int, default=5)
    sub.add_parser("kill")
    a = ap.parse_args()
    {"engage": cmd_engage, "ladder": cmd_ladder, "readback": cmd_readback,
     "postmortem": cmd_postmortem, "flash-minz": cmd_flash_minz,
     "flash-am32": cmd_flash_am32, "sweep-am32": cmd_sweep_am32,
     "kill": cmd_kill}[a.cmd](a)


if __name__ == "__main__":
    main()
