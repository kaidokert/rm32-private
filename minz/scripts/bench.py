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

    def cl_active_now(self):
        """Robust engage check: the newest q-line's flags field
        (bit0 = CL_ACTIVE). Immune to the RX echo loss that made the
        'i'-echo check kill healthy locks (the fake half of the
        engage lottery, 2026-07-19)."""
        buf = self.key("", 0.3)
        hits = re.findall(
            r"q \d+ \d+ \d+ (\d+) \d+ -?\d+ -?\d+ -?\d+ \d+ (\d+)",
            buf)
        if not hits:
            return None
        iv, flags = hits[-1]
        if int(flags) & 1 and int(iv):
            return 1_000_000 // (6 * int(iv))
        return 0

    def engage(self, attempts=10):
        self.kill()
        self.press_until("D", "AM32")
        self.press_until("M", "SWIFT")
        self.press_until("Q", "qlog=on", tries=6)
        for att in range(attempts):
            self.key("Y", 4.0)
            fe = None
            for _ in range(4):
                fe = self.cl_active_now()
                if fe:
                    return att + 1, fe
            # fallback: the old echo check (q-lines may be off)
            m = re.search(r"cl: ACTIVE f_e=(\d+)Hz", self.key("i", 1.0))
            if m:
                return att + 1, int(m.group(1))
            self.key("w", 0.5)
            time.sleep(2.5)
        return None, None


def nm_addr(sym):
    out = sh("arm-none-eabi-nm", ELF)
    exact = None
    mangled = None
    for ln in out.splitlines():
        s = ln.strip()
        if s.endswith(sym):
            exact = int(ln.split()[0], 16)
        elif sym + "17h" in s:
            # rust-mangled statics carry a 17h<hash> suffix
            mangled = int(ln.split()[0], 16)
    if exact is not None:
        return exact
    if mangled is not None:
        return mangled
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
    per_rung = 1.6 if a.step >= 10 else 0.55
    # +25 s slack: engage attempts + echo-verified key retries eat
    # into the window (m5 incident: the wait expired mid-climb and
    # the script killed a HEALTHY rung-72 ladder, logged as "silent
    # death" — as did m2/m3/m4).
    wait = per_rung * (a.top - 10) / a.step + a.hold + 25.0
    for run in range(a.retries):
        b = Bench()
        died = None
        no_engage = False
        try:
            att, fe = b.engage()
            if not att:
                print(f"run {run}: NO ENGAGE")
                no_engage = True
        finally:
            pass
        if no_engage:
            import pathlib
            tag = a.tag or f"step{a.step}"
            capfile = (pathlib.Path("captures")
                       / f"ladder_cap_{tag}_r{run}_noengage.bin")
            capfile.write_bytes(bytes(b.cap))
            print(f"engage-fail capture -> {capfile} ({len(b.cap)} bytes)")
            b.kill()
            b.close()
            time.sleep(3.0)
            continue
        try:
            print(f"engaged attempt {att}: {fe} Hz; ladder step={a.step}")
            # Q-log always; MAGPIE stream optional (--nostream): at
            # ~1900 Hz the per-window serialize+TX load starves main.
            if not a.nostream and not b.press_until("g", "stream=on", tries=4):
                print("stream-on not confirmed (may be off)")
            if not b.press_until("Q", "qlog=on", tries=4):
                print("qlog-on not confirmed (may be off)")
            if a.jarm:
                b.key("J", 0.5)  # arm the >4A WAX trigger (autopsy)
            if not b.press_until("L", "auto-ladder step = 1"):
                print("1%-arm not confirmed (echo loss) - proceeding")
            if a.step >= 10:
                if not b.press_until("L", "auto-ladder step = 10"):
                    print("10%-arm not confirmed - may be at 1%")
            print(f"climb + hold ~{wait:.0f}s, hands off the wire...")
            t0 = time.monotonic()
            tail = b""
            last_rx = time.monotonic()
            probed = False
            while time.monotonic() - t0 < wait:
                chunk = b.ser.read(8192)
                b.cap.extend(chunk)
                if chunk:
                    last_rx = time.monotonic()
                tail = (tail + chunk)[-4096:]
                # WEDGE CATCH: q-lines flow at 25 Hz; >6 s of TOTAL
                # silence with no kill print = main is wedged and the
                # IWDG will fire at ~31 s. Probe-read the clock state
                # LIVE (memory reads don't halt the core).
                if not probed and time.monotonic() - last_rx > 6.0:
                    probed = True
                    print("WEDGE DETECTED - live probe:")
                    try:
                        syms = ["COMP_COUNT", "TIM1_UP_COUNT",
                                "LPTIM2_COUNT", "TIM7_COUNT",
                                "USART2_COUNT", "TIM1_CC_COUNT"]
                        addrs = {s: nm_addr(s) for s in syms}
                        c1 = probe_words(0xE0001004, 1)[0]
                        r1 = {s: probe_words(a, 1)[0]
                              for s, a in addrs.items()}
                        t1 = time.monotonic()
                        time.sleep(1.0)
                        c2 = probe_words(0xE0001004, 1)[0]
                        r2 = {s: probe_words(a, 1)[0]
                              for s, a in addrs.items()}
                        dt = time.monotonic() - t1
                        ph = probe_words(nm_addr("BEACON_PHASE"), 1)[0]
                        print(f"  CYCCNT delta {(c2 - c1) & 0xFFFFFFFF} "
                              f"phase={ph}")
                        for s in syms:
                            rate = ((r2[s] - r1[s]) & 0xFFFFFFFF) / dt
                            print(f"  {s}: {rate:.0f}/s")
                        # Sample the wedged thread PC: pend PendSV via
                        # ICSR, the handler stores main's PC/LR.
                        for _ in range(3):
                            sh("probe-rs", "write", "--chip", CHIP,
                               "--probe", PROBE, "b32", "0xE000ED04",
                               "0x10000000")
                            time.sleep(0.2)
                        pc = probe_words(nm_addr("MST_PC"), 1)[0]
                        lr = probe_words(nm_addr("MST_LR"), 1)[0]
                        pcs = probe_words(nm_addr("MST_PCS"), 8)
                        print(f"  wedged PC={pc:08x} LR={lr:08x}")
                        print("  PC ring: " +
                              " ".join(f"{p:08x}" for p in pcs))
                        # TX black-hole check: DMA1_CH4 + USART1 state
                        ccr4 = probe_words(0x40020044, 1)[0]
                        nd1 = probe_words(0x40020048, 1)[0]
                        time.sleep(0.2)
                        nd2 = probe_words(0x40020048, 1)[0]
                        uisr = probe_words(0x4001381C, 1)[0]
                        dmaisr = probe_words(0x40020000, 1)[0]
                        print(f"  DMA CCR4={ccr4:08x} CNDTR4 {nd1}->{nd2} "
                              f"DMA_ISR={dmaisr:08x} USART1_ISR={uisr:08x}")
                    except (SystemExit, IndexError, TypeError) as e:
                        print(f"  probe failed: {e}")
                # Kill prints are '!! <CAPS>' TEXT; a bare b"!!" scan
                # false-fired on 0x21 0x21 inside binary MAGPIE
                # frames and aborted healthy climbs (m1 incident:
                # clean 980 Hz climb killed by the monitor itself).
                # Match the actual kill strings + the boot banner.
                for marker in (b"!! CL", b"!! VBAT", b"!! OVERCURRENT",
                               b"!! MAIN", b"motor_tester2: clocks"):
                    if marker in tail:
                        died = tail[tail.find(marker):][:80]
                        break
                if died:
                    break
        finally:
            b.kill()
            b.close()
        import pathlib
        tag = a.tag or f"step{a.step}"
        capfile = pathlib.Path("captures") / f"ladder_cap_{tag}_r{run}.bin"
        capfile.write_bytes(bytes(b.cap))
        print(f"serial capture -> {capfile} ({len(b.cap)} bytes)")
        if died:
            msg = died.decode("ascii", "replace").strip()
            print(f"run {run}: DIED mid-ladder: {msg}".encode(
                "ascii", "replace").decode())
            cmd_postmortem(a)
            time.sleep(3.0)
            continue
        rows = read_ladder_log()
        top = rows[-1][0] if rows else 0
        if top >= a.top - 2:
            cmd_readback(a)
            return
        print(f"run {run}: silent death at rung {top}; retrying")
        cmd_postmortem(a)
        time.sleep(3.0)
    print("LADDER FAILED all retries")


def read_ladder_log():
    base = nm_addr("LADDER_LOG")
    w = probe_words(base, 384)
    rows = []
    for amp in range(10, 96):
        iv, ir, cc, vb = (w[amp * 4], w[amp * 4 + 1],
                          w[amp * 4 + 2], w[amp * 4 + 3])
        if iv or cc:
            rows.append((amp, iv, ir, cc, vb))
    return rows


def save_ladder_csv(rows, tag):
    import pathlib
    path = pathlib.Path("captures") / f"ladder_{tag}.csv"
    with open(path, "w") as f:
        f.write("amp,interval_us,f_e_hz,isns_raw,comms,vbat_raw\n")
        for amp, iv, ir, cc, vb in rows:
            hz = 1e6 / (6 * iv) if iv else 0
            f.write(f"{amp},{iv},{hz:.0f},{ir},{cc},{vb}\n")
    print(f"-> {path}")


def cmd_readback(a):
    rows = read_ladder_log()
    top = rows[-1][0] if rows else 0
    print(f"=== TOP RUNG {top} ===")
    print("amp | f_e Hz | isns_raw | vbat_raw | comms")
    for amp, iv, ir, cc, vb in rows:
        if amp % 5 == 0 or amp >= top - 3:
            print(f"{amp:3d} | {1e6/(6*iv) if iv else 0:6.0f} | {ir:4d} | "
                  f"{vb:4d} | {cc}")
    tag = getattr(a, "tag", None) or f"step{getattr(a, 'step', 1)}"
    save_ladder_csv(rows, tag)


def cmd_postmortem(_):
    print(sh(sys.executable, "scripts/bb_postmortem.py"))
    # Chain-stop snapshot taken at the first high-speed reseed:
    # armed==fired -> no shot was armed (upstream accept never ran);
    # armed==fired+1 -> a shot armed but never fired (SNGSTRT class).
    try:
        snap = probe_words(nm_addr("RSD_SNAP"), 12)
        names = ["armed", "fired", "lptim2_isr", "lptim2_cnt",
                 "lptim2_arr", "lptim2_cr", "est_acc_us", "detail",
                 "last_delay_us", "last_est_us", "last_qzc_us", "now_1us"]
        print("RSD_SNAP:", ", ".join(
            f"{n}={v:#x}" if n.startswith("lptim2_") else f"{n}={v}"
            for n, v in zip(names, snap)))
        if any(snap):
            print(f"  armed-fired delta at stop = {snap[0] - snap[1]}")
    except SystemExit:
        pass
    # Arm provenance ring: last 8 LPTIM2 arms as (src, delay, iv, t).
    # src: 1=precheck 2=lottery 3=reseed-crawl 4=freerun 5=rescue
    try:
        ar = probe_words(nm_addr("ARM_RING"), 24)
        idx = probe_words(nm_addr("ARM_RING_IDX"), 1)[0]
        print("ARM_RING (oldest->newest):")
        for k in range(8):
            slot = ((idx + k) % 8) * 3
            w0, iv, t = ar[slot], ar[slot + 1], ar[slot + 2]
            if w0 == 0 and iv == 0:
                continue
            print(f"  src={w0 >> 24} delay={w0 & 0xFFFFFF}us iv={iv}us "
                  f"t={t}")
    except (SystemExit, IndexError):
        pass
    try:
        acc = probe_words(nm_addr("AVG_INTERVAL_ACC"), 1)[0]
        print(f"AVG_INTERVAL_ACC/6 (stiff avg) = {acc // 6} us")
    except SystemExit:
        pass
    # Accepted-key ring: which keys actually dispatched, oldest->newest
    try:
        klog = sh("probe-rs", "read", "--chip", CHIP, "--probe", PROBE,
                  "b8", hex(nm_addr("KEY_LOG")), "16")
        bts = [int(x, 16) for ln in klog.splitlines()
               for x in ln.split() if len(x) == 2]
        idx = probe_words(nm_addr("KEY_LOG_IDX"), 1)[0]
        seq = [bts[(idx + k) % 16] for k in range(16)]
        print("KEY_LOG (oldest->newest):",
              " ".join(chr(c) if 32 <= c < 127 else f"\\x{c:02x}"
                       for c in seq if c))
    except (SystemExit, IndexError):
        pass
    try:
        csr = probe_words(nm_addr("BOOT_CSR"), 1)[0]
        print(f"BOOT_CSR = {csr:08x} iwdg={(csr >> 29) & 1} "
              f"sft={(csr >> 28) & 1} bor={(csr >> 27) & 1} "
              f"pin={(csr >> 26) & 1}")
    except (SystemExit, IndexError):
        pass
    try:
        lk = probe_words(nm_addr("LAST_KILL"), 1)[0] & 0xFF
        names = {0: "never", 1: "desync", 2: "starved", 3: "overcurrent",
                 4: "sag"}
        print(f"LAST_KILL = {lk} ({names.get(lk, '?')})")
    except (SystemExit, IndexError):
        pass
    for sym in ("STORM_KILLS", "ZOMBIE_BACKSTOP_KILLS", "MAIN_STARVE_KILLS",
                "KEY_I_COUNT", "KEY_ANY_COUNT", "KEY_REJECT_COUNT",
                "USART2_COUNT",
                "SHOT_ARMED_COUNT", "LPTIM2_COUNT", "CHAIN_KICKS",
                "SAG_HOLD_COUNT", "BURST_TRIPS",
                "RECOV_COUNT", "RESEED_COUNT", "SLEW_CLAMP_COUNT",
                "WAIT_CLAMP_COUNT", "SPIN_TIMEOUTS"):
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


def cmd_peek(a):
    for sym in a.syms:
        try:
            addr = nm_addr(sym)
            if a.words > 1:
                print(f"{sym} = {probe_words(addr, a.words)}")
                continue
            # u8 statics land at unaligned addresses; b8 always works
            out = sh("probe-rs", "read", "--chip", CHIP, "--probe", PROBE,
                     "b8", hex(addr), "4")
            bts = [int(x, 16) for ln in out.splitlines()
                   for x in ln.split() if len(x) == 2]
            if not bts:
                print(f"{sym} @ {addr:#x}: no data ({out.strip()[:80]})")
                continue
            v = sum(b << (8 * i) for i, b in enumerate(bts[:4]))
            print(f"{sym} = {v} (b0={bts[0]})")
        except SystemExit as e:
            print(e)


def cmd_capdump(a):
    # Extract printable text runs from a mixed text+MAGPIE capture.
    data = open(a.file, "rb").read()
    run = bytearray()
    out = []
    for byte in data:
        if 32 <= byte < 127 or byte in (10, 13):
            run.append(byte)
        else:
            if len(run) >= a.min_len:
                out.append(run.decode())
            run = bytearray()
    if len(run) >= a.min_len:
        out.append(run.decode())
    text = "".join(out)
    for ln in text.splitlines():
        s = ln.strip()
        if s and not a.grep or (a.grep and a.grep in s):
            print(s)


def cmd_probe_adv(a):
    # Live A/B on a steady lock: engage, stream `secs` baseline,
    # press `key` x presses, stream `secs` more. Captures land as
    # captures/ab_{tag}_{A,B}.bin for plot_climb comparison.
    b = Bench()
    try:
        att, fe = b.engage()
        if not att:
            print("NO ENGAGE")
            return
        print(f"engaged attempt {att}: {fe} Hz")
        if not b.press_until("g", "stream=on", tries=6):
            print("WARN: stream-on unconfirmed")
        if not b.press_until("Q", "qlog=on", tries=6):
            print("WARN: qlog-on unconfirmed")
        mark0 = len(b.cap)
        t0 = time.monotonic()
        while time.monotonic() - t0 < a.secs:
            b.cap.extend(b.ser.read(8192))
        mark1 = len(b.cap)
        for n in range(a.presses):
            # echo-verified press (the wire eats singles at will)
            for _ in range(5):
                if a.echo in b.key(a.key, 0.4):
                    break
            else:
                print(f"press {n}: echo '{a.echo}' never seen")
        t0 = time.monotonic()
        while time.monotonic() - t0 < a.secs:
            b.cap.extend(b.ser.read(8192))
        mark2 = len(b.cap)
    finally:
        b.kill()
        b.close()
    import pathlib
    base = pathlib.Path("captures")
    (base / f"ab_{a.tag}_A.bin").write_bytes(bytes(b.cap[mark0:mark1]))
    (base / f"ab_{a.tag}_B.bin").write_bytes(bytes(b.cap[mark1:mark2]))
    print(f"A: {mark1-mark0} bytes, B: {mark2-mark1} bytes "
          f"-> captures/ab_{a.tag}_*.bin")


def cmd_waxdump(a):
    # Engage steady, take a WAXWING 'j' analog dump, save the capture.
    b = Bench()
    try:
        att, fe = b.engage()
        if not att:
            print("NO ENGAGE")
            return
        print(f"engaged attempt {att}: {fe} Hz; dumping...")
        time.sleep(1.0)
        mark = len(b.cap)
        b.key("j", 1.0)
        t0 = time.monotonic()
        while time.monotonic() - t0 < a.secs:
            b.cap.extend(b.ser.read(8192))
    finally:
        b.kill()
        b.close()
    import pathlib
    path = pathlib.Path("captures") / f"wax_{a.tag}.bin"
    path.write_bytes(bytes(b.cap[mark:]))
    print(f"-> {path} ({len(b.cap) - mark} bytes)")


def cmd_ztstats(a):
    # Per-step zt (ZC-to-ZC) means from an AM32 ZCTRACE csv — the
    # even/odd alternation discriminator, reference side.
    import csv as _csv
    per = {s: [] for s in range(1, 7)}
    with open(a.file) as f:
        for row in _csv.DictReader(f):
            s = int(row["step"])
            zt = float(row["zt_us"])
            if 1 <= s <= 6 and 0 < zt < 20000:
                per[s].append(zt)
    for s in range(1, 7):
        v = per[s]
        if not v:
            print(f"step {s}: n=0")
            continue
        v.sort()
        mean = sum(v) / len(v)
        print(f"step {s}: n={len(v)} mean={mean:.0f} med={v[len(v)//2]:.0f} "
              f"p10={v[len(v)//10]:.0f} p90={v[9*len(v)//10]:.0f}")


def cmd_engage_probe(a):
    # N instrumented engage attempts: stream + qlog on BEFORE the
    # first Y, per-attempt capture slices + marker classification.
    b = Bench()
    slices = []
    try:
        b.kill()
        b.press_until("D", "AM32")
        b.press_until("M", "SWIFT")
        b.press_until("g", "stream=on", tries=6)
        b.press_until("Q", "qlog=on", tries=6)
        for att in range(a.attempts):
            mark = len(b.cap)
            b.key("Y", 4.0)
            ok = False
            for _ in range(4):
                if b.cl_active_now():
                    ok = True
                    break
            slices.append((mark, len(b.cap), ok))
            if ok:
                print(f"attempt {att}: ENGAGED")
                b.kill()
                time.sleep(2.0)
            else:
                print(f"attempt {att}: FAILED")
                b.kill()
                time.sleep(2.5)
    finally:
        b.kill()
        b.close()
    import pathlib
    base = pathlib.Path("captures")
    markers = [b"r6: START", b"CL ARMED", b"cl: ACTIVE", b"DESYNC",
               b"ZC-STARVED", b"SAG", b"r6: ABORT", b"clocks"]
    for k, (m0, m1, ok) in enumerate(slices):
        sl = bytes(b.cap[m0:m1])
        (base / f"engage_{a.tag}_a{k}.bin").write_bytes(sl)
        seen = " ".join(m.decode() for m in markers if m in sl)
        print(f"a{k} {'OK ' if ok else 'FAIL'} {len(sl):7d}B markers: {seen}")


def cmd_starts(a):
    # The 20/20 gauntlet: N times — Y-start, quick sweep to ~30%
    # throttle, verify lock speed, kill. Verdict per iteration:
    # first-try engage + clean sweep = CLEAN.
    b = Bench()
    clean = 0
    try:
        b.kill()
        b.press_until("D", "AM32")
        b.press_until("M", "SWIFT")
        b.press_until("Q", "qlog=on", tries=6)
        for it in range(a.n):
            b.key("Y", 4.0)
            fe0 = None
            for _ in range(4):
                fe0 = b.cl_active_now()
                if fe0:
                    break
            if not fe0:
                print(f"start {it}: NO ENGAGE")
                b.kill()
                time.sleep(2.0)
                continue
            # sweep 15 -> 30: one +10 then five echo-verified +1
            for _ in range(5):
                if "amp" in b.key("s", 0.4):
                    break
            for n in range(5):
                for _ in range(5):
                    if "amp" in b.key("a", 0.4):
                        break
            time.sleep(1.5)
            fe1 = b.cl_active_now()
            ok = fe1 and fe1 > 450
            if ok:
                clean += 1
            print(f"start {it}: engaged {fe0}Hz -> sweep {fe1 or 0}Hz "
                  f"{'CLEAN' if ok else 'DIRTY'}")
            b.kill()
            time.sleep(2.0)
    finally:
        b.kill()
        b.close()
    print(f"=== {clean}/{a.n} clean ===")


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
    p.add_argument("--top", type=int, default=96)
    p.add_argument("--hold", type=float, default=30.0)
    p.add_argument("--step", type=int, default=1, choices=(1, 2, 10))
    p.add_argument("--tag", default=None)
    p.add_argument("--retries", type=int, default=4)
    p.add_argument("--nostream", action="store_true")
    p.add_argument("--jarm", action="store_true")
    p = sub.add_parser("readback")
    p.add_argument("--tag", default=None)
    p.add_argument("--step", type=int, default=1)
    sub.add_parser("postmortem")
    sub.add_parser("flash-minz")
    p = sub.add_parser("flash-am32")
    p.add_argument("--trace", action="store_true")
    p = sub.add_parser("sweep-am32")
    p.add_argument("--lo", type=int, default=50)
    p.add_argument("--hi", type=int, default=100)
    p.add_argument("--step", type=int, default=5)
    sub.add_parser("kill")
    p = sub.add_parser("peek")
    p.add_argument("syms", nargs="+")
    p.add_argument("--words", type=int, default=1)
    p = sub.add_parser("capdump")
    p.add_argument("file")
    p.add_argument("--min-len", type=int, default=6)
    p.add_argument("--grep", default=None)
    p = sub.add_parser("probe-adv")
    p.add_argument("--presses", type=int, default=7)
    p.add_argument("--secs", type=float, default=3.0)
    p.add_argument("--key", default="t")
    p.add_argument("--echo", default="")
    p.add_argument("--tag", default="probe")
    p = sub.add_parser("waxdump")
    p.add_argument("--tag", default="lock")
    p.add_argument("--secs", type=float, default=8.0)
    p = sub.add_parser("ztstats")
    p.add_argument("file")
    p = sub.add_parser("engage-probe")
    p.add_argument("--attempts", type=int, default=8)
    p.add_argument("--tag", default="ep")
    p = sub.add_parser("starts")
    p.add_argument("--n", type=int, default=3)
    a = ap.parse_args()
    {"engage": cmd_engage, "ladder": cmd_ladder, "readback": cmd_readback,
     "postmortem": cmd_postmortem, "flash-minz": cmd_flash_minz,
     "flash-am32": cmd_flash_am32, "sweep-am32": cmd_sweep_am32,
     "kill": cmd_kill, "peek": cmd_peek, "capdump": cmd_capdump,
     "probe-adv": cmd_probe_adv, "waxdump": cmd_waxdump,
     "ztstats": cmd_ztstats, "engage-probe": cmd_engage_probe,
     "starts": cmd_starts}[a.cmd](a)


if __name__ == "__main__":
    main()
