"""minz ZC-trace capture (differential climb trace, minz side).

Mirrors zctrace_capture.py: engage the closed loop, enable the MZT
record stream (Z key), climb the same 60->80 profile, capture raw,
decode 5B AA records, CSV out.

Record (LE): 5B AA | flags (bits0-2 sector, bit7 refined) |
period u16 (us, commutation-to-commutation) | estimate u16 (us) |
delay u16 (us armed) | duty u16 (CCR counts) | t10 u16 (10us anchor)
| qzc_off u16 (us from window open; FFFF = none).

Run WITHOUT the MAGPIE stream (g off) - wire budget.
"""

import argparse
import pathlib
import re
import struct
import sys
import time

import serial

REC = 21


def decode(buf):
    recs = []
    i = 0
    while i + REC <= len(buf):
        if buf[i] == 0x5B and buf[i + 1] == 0xAC:
            fs = buf[i + 2]
            (per, estb, est, dly, duty, t10, qoff, raw_iv, stiff) = struct.unpack_from(
                "<HHHHHHHHH", buf, i + 3)
            if 20 <= per <= 30000 and est <= 30000:
                recs.append(dict(sector=fs & 7, refined=bool(fs & 0x80),
                                 period_us=per, est_before_us=estb,
                                 est_us=est, delay_us=dly,
                                 duty=duty, t10=t10, qzc_off=qoff,
                                 raw_iv=raw_iv, stiff=stiff))
                i += REC
                continue
        i += 1
    return recs


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--climb", nargs=2, type=int, default=[60, 80])
    ap.add_argument("--dwell", type=float, default=3.0)
    ap.add_argument("--step-wait", type=float, default=0.7)
    ap.add_argument("--tag", default="mzt")
    ap.add_argument("--config", choices=["base", "j", "zt", "both"],
                    default="zt", help="reset-matrix telemetry config")
    a = ap.parse_args()
    lo, hi = a.climb

    ser = serial.Serial("COM41", 2_000_000, timeout=0.05)
    cap = bytearray()

    def key(k, wait):
        ser.write(k.encode())
        ser.flush()
        t0 = time.monotonic()
        buf = b""
        while time.monotonic() - t0 < wait:
            b = ser.read(8192)
            buf += b
            cap.extend(b)
        return buf.decode("utf-8", "replace")

    def press_until(k, want, tries=3):
        for _ in range(tries):
            if want in key(k, 0.5):
                return True
        return False

    try:
        press_until("D", "AM32")
        press_until("M", "SWIFT")
        # EDGE_MODE stays at the boot default (mode 0, both edges):
        # the mode-3 experiment REGRESSED mid-rung robustness
        # (deaths at 1100-1400 Hz vs mode-0 1790 Hz) - see the
        # EDGE_MODE doc comment in motor_tester2.rs.
        engaged = False
        for _ in range(8):
            # R6 POLLING START (operator directive 2026-07-18: "just
            # blatantly copy what am32 does"): Y arms the rotor-paced
            # start from standstill (AM32 old_routine semantics) and
            # auto-hands-off to CL - no fixed-frequency catch, no
            # lottery, no cold-boot warm-up sensitivity.
            key("Y", 5.0)
            m = re.search(r"cl: ACTIVE f_e=(\d+)Hz", key("i", 1.2))
            # junk-crawl guard (mzt_fatal4: a 29 Hz "lock" passed the
            "            # ACTIVE-only check and OC-tripped): require a sane f_e.",
            if m and int(m.group(1)) > 250:
                engaged = True
                break
            key("w", 1.0)
        if not engaged:
            raise SystemExit("no engage in 8")
        print(f"engaged; config={a.config}")
        if a.config in ("j", "both"):
            key("J", 0.5)
        if a.config in ("zt", "both"):
            # stateful toggle: confirm via echo (the D/M lesson)
            for _ in range(3):
                if "zt trace = on" in key("Z", 0.4):
                    break
        n0 = len(cap)
        died = False

        def climb_to(target):
            """Echo-verified amp stepping: RX keys DROP under full ZT
            stream load (USART2 prio 4 overruns) - the mzt_90a ladder
            silently capped at amp ~55 while "COMPLETED". Press, then
            confirm the amp= echo advanced; resend on loss."""
            cur = None
            for _ in range(300):
                out = key("a", 0.15)
                m = re.findall(r"amp=(\d+)", out)
                if m:
                    cur = int(m[-1])
                    if cur >= target:
                        return cur
            return cur

        # climb from the engage amp to lo under trace (echo-verified)
        reached = climb_to(lo)
        print(f"climbed to amp {reached}")
        time.sleep(1.0)
        # ZOMBIE LESSON (mzt_beacon): "cl: ACTIVE f_e=" is frozen
        # statics - it stays ACTIVE with the commutation chain dead
        # and 1.5 A DC in the winding. Under ZT the honest liveness
        # is RECORD FLOW (records stop when commutation stops).
        # RETRIED flow check (mzt_90f/g: transient TX outages at
        # amp 60 false-killed the band climb; txrs= counts the
        # DMA wedge heals behind them).
        flow = 0
        for _ in range(4):
            n1 = len(cap)
            key("", 1.0)
            flow = bytes(cap[n1:]).count(b"\x5b\xab")
            if flow:
                break
        if flow == 0:
            died = True
            print("DIED pre-band (no record flow x4) - fatal capture")
        else:
            print(f"at {lo}: {flow} rec/s; tracing band climb")
            key("", a.dwell)
            reached = climb_to(hi)
            print(f"band climb reached amp {reached}")
            # SILENT dwell: the i-poll flooded the wire and cost the
            # crisis records (mzt_fatal6 queue drops at the death) -
            # capture quietly, detect death post-hoc.
            key("", a.dwell)
        # stream OFF first, then a CLEAN liveness read (mzt_fatal8:
        "        # the i-echo interleaved with trace binary and false-",
        "        # negatived - the script killed a healthy motor).",
        if a.config in ("zt", "both"):
            for _ in range(3):
                if "zt trace = off" in key("Z", 0.4):
                    break
        # comms-DELTA liveness: two reads 1.2 s apart. A zombie
        # prints ACTIVE forever but its comms counter is frozen.
        c1 = re.search(r"comms=(\d+)", key("i", 1.5))
        time.sleep(1.2)
        c2 = re.search(r"comms=(\d+)", key("i", 1.5))
        if c1 and c2:
            died = int(c2.group(1)) <= int(c1.group(1))
            if died:
                print(f"ZOMBIE/DEAD: comms frozen at {c2.group(1)}")
        else:
            # Echoes shredded by a still-streaming ZT wire (mzt_final2:
            # a fully-alive top-of-profile run was classified SILENT-
            # DEATH). RECORD FLOW is the ground-truth liveness - one
            # record = one commutation, unfakeable.
            n2 = len(cap)
            key("", 1.0)
            died = bytes(cap[n2:]).count(b'\x5b\xab') == 0
            print(f"liveness via record flow: {'DEAD' if died else 'alive'}")
        seg = bytes(cap[n0:])
        print("outcome:", "DIED (fatal capture)" if died else "survived")
    finally:
        key("w", 0.5)
        key("w", 0.3)
        ser.close()

    if 'seg' not in dir():
        seg = bytes(cap)
    raw_path = pathlib.Path("captures") / f"{a.tag}_raw.bin"
    raw_path.write_bytes(cap)
    recs = decode(bytes(cap))
    csv_path = pathlib.Path("captures") / f"{a.tag}_trace.csv"
    with open(csv_path, "w") as f:
        f.write("sector,refined,period_us,est_before_us,est_us,delay_us,duty,t10,qzc_off,raw_iv_us,stiff_us\n")
        for r in recs:
            f.write(f"{r['sector']},{int(r['refined'])},{r['period_us']},"
                    f"{r['est_before_us']},{r['est_us']},{r['delay_us']},"
                    f"{r['duty']},{r['t10']},{r['qzc_off']},"
                    f"{r['raw_iv']},{r['stiff']}\n")
    # TERMINAL CLASSIFICATION (mandatory for autopsy eligibility)
    rawb = bytes(cap)
    reboot = rawb.count(b"reset: csr=")
    kill = b"!! " in rawb
    lkm = re.findall(rb"lk=(\d+)", rawb)
    lk_last = lkm[-1].decode() if lkm else "?"
    if reboot:
        cz = re.findall(rb"reset: csr=[0-9a-f]+ iwdg=(\d)", rawb)
        term = f"REBOOT (banners={reboot}, iwdg={cz[-1].decode() if cz else chr(63)})"
    elif died and kill:
        term = f"KILL (lk={lk_last})"
    elif died:
        term = f"SILENT-DEATH (lk={lk_last}, no banner, no kill print)"
    else:
        term = "COMPLETED"
    print(f"TERMINAL: {term}")
    print(f"trace window: {len(seg)} B -> {len(recs)} records")
    print(f"raw: {raw_path}  csv: {csv_path}")
    if recs:
        print("sample:", recs[len(recs) // 2])


if __name__ == "__main__":
    main()
