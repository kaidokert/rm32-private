"""Host capture for examples/envelope-ladder.rs.

Parses the 22-byte binary telemetry stream (0x5B 0xA9 sync, XOR checksum)
interleaved with text lines (RUNG,/COAST,/BB,/END,), writes a run directory
with frames.csv + events.log, and reports stream-loss and PASS/FAIL.

Modes:
  python scripts/envelope_ladder.py                # capture a ladder run
  python scripts/envelope_ladder.py --provoke      # PASS gate 1: provoke
      an overcurrent kill and verify a parseable blackbox arrives.

Kill-guarded: on ANY exit path the script sends 'k' (firmware kill) before
closing the port.
"""

import argparse
import os
import re
import struct
import sys
import time

import serial

PORT = "COM7"
BAUD = 2_000_000
FRAME_LEN = 22
SYNC = b"\x5b\xa9"


class StreamParser:
    def __init__(self):
        self.buf = bytearray()
        self.frames = []
        self.text_lines = []
        self._text = bytearray()
        self.bad_checksum = 0

    def feed(self, data: bytes):
        self.buf.extend(data)
        while True:
            i = self.buf.find(SYNC)
            if i < 0:
                # no sync: all but the last byte is text (last byte could be
                # the first half of a split sync marker)
                if len(self.buf) > 1:
                    self._take_text(self.buf[:-1])
                    del self.buf[:-1]
                return
            if i > 0:
                self._take_text(self.buf[:i])
                del self.buf[:i]
            if len(self.buf) < FRAME_LEN:
                return
            frame = bytes(self.buf[:FRAME_LEN])
            x = 0
            for b in frame[2:20]:
                x ^= b
            if x == frame[20]:
                self.frames.append(self._decode(frame))
                del self.buf[:FRAME_LEN]
            else:
                self.bad_checksum += 1
                del self.buf[:2]  # resync past this sync marker

    def _take_text(self, chunk):
        self._text.extend(chunk)
        while b"\n" in self._text:
            line, _, rest = bytes(self._text).partition(b"\n")
            self._text = bytearray(rest)
            s = line.decode("ascii", errors="replace").strip()
            if s:
                self.text_lines.append((time.time(), s))

    @staticmethod
    def _decode(f):
        seq, state = f[2], f[3]
        vals = struct.unpack_from("<8H", f, 4)
        phase, vph1, vph2, vph3, is_mv, vm, fchz, amp = vals
        return dict(
            t=time.time(), seq=seq, rung=state & 0x0F, coast=bool(state & 0x40),
            phase=phase, vph1=vph1, vph2=vph2, vph3=vph3, is_mv=is_mv,
            vm=vm, fchz=fchz, amp=amp,
        )


def seq_loss(frames):
    if len(frames) < 2:
        return 0, 0
    lost = 0
    for a, b in zip(frames, frames[1:]):
        gap = (b["seq"] - a["seq"]) & 0xFF
        if gap > 1:
            lost += gap - 1
    return lost, lost + len(frames)


def run(provoke: bool, out_root: str, duration: float):
    s = serial.Serial(PORT, BAUD, timeout=0.05)
    p = StreamParser()
    run_dir = os.path.join(out_root, time.strftime("run_%Y%m%d_%H%M%S"))
    os.makedirs(run_dir, exist_ok=True)
    ended = False
    provoked = False
    t0 = time.time()
    try:
        while time.time() - t0 < duration:
            data = s.read(4096)
            if data:
                p.feed(data)
            if provoke and not provoked and len(p.frames) > 200:
                print("stream alive; sending provoke 'p'")
                s.write(b"p")
                provoked = True
            for _, line in p.text_lines:
                if line.startswith("END,"):
                    ended = True
            if ended:
                time.sleep(1.0)
                p.feed(s.read(65536))
                break
    finally:
        try:
            s.write(b"k")  # kill-guard: firmware safes the stage
            s.flush()
        except Exception:
            pass
        s.close()

    # Persist.
    with open(os.path.join(run_dir, "frames.csv"), "w", encoding="utf-8") as f:
        f.write("t,seq,rung,coast,phase,vph1,vph2,vph3,is_mv,vm,fchz,amp\n")
        for fr in p.frames:
            f.write(
                f"{fr['t']:.4f},{fr['seq']},{fr['rung']},{int(fr['coast'])},"
                f"{fr['phase']},{fr['vph1']},{fr['vph2']},{fr['vph3']},"
                f"{fr['is_mv']},{fr['vm']},{fr['fchz']},{fr['amp']}\n"
            )
    with open(os.path.join(run_dir, "events.log"), "w", encoding="utf-8", errors="replace") as f:
        for t, line in p.text_lines:
            f.write(f"{t:.4f} {line}\n")

    lost, total = seq_loss(p.frames)
    loss_pct = 100.0 * lost / max(1, total)
    print(f"\nrun dir: {run_dir}")
    print(f"frames: {len(p.frames)}  bad_checksum: {p.bad_checksum}  "
          f"seq loss: {lost}/{total} = {loss_pct:.3f}%")
    # Binary frame bytes prefix some text lines; match tokens anywhere.
    alltext = "\n".join(l for _, l in p.text_lines)
    rungs = re.findall(r"RUNG,\d+,\d+,\d+,\d+,\d+,\d+,\d+,\d+,\d+", alltext)
    coasts = re.findall(r"COAST,\d+,\d+,\d+,\d+", alltext)
    bbs = re.findall(r"BB,\d+,[A-Z_]+,\d+", alltext)
    ends = re.findall(r"END,[a-zA-Z :]+,\d+", alltext)
    print(f"rung summaries: {len(rungs)}  coasts: {len(coasts)}  "
          f"blackbox lines: {len(bbs)}  end: {ends}")

    if provoke:
        ok = any("KILL_OC" in l for l in bbs) and any("PROVOKE" in l for l in bbs)
        got_dump = len(bbs) >= 2
        print(f"\nPASS GATE 1 (provoked OC -> parseable blackbox): "
              f"{'PASS' if ok and got_dump else 'FAIL'}")
        return 0 if ok and got_dump else 1
    ok_loss = loss_pct < 0.1
    # Graceful end evidence: an END,complete/envelope line OR a blackbox
    # DONE event (the text END line can be lost in binary interleaving).
    graceful_bb = any(",DONE," in b for b in bbs)
    killed_bb = any(re.search(r",KILL_[A-Z]+,", b) for b in bbs)
    ok_end = graceful_bb or any("complete" in l or "envelope" in l for l in ends)
    no_kill = not killed_bb and not any("kill" in l for l in ends)
    print(f"\nPASS stream loss <0.1%: {'PASS' if ok_loss else 'FAIL'}")
    print(f"PASS ladder ended gracefully (no kill): "
          f"{'PASS' if ok_end and no_kill else 'FAIL'}")
    return 0 if ok_loss and ok_end and no_kill else 1


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("--provoke", action="store_true")
    ap.add_argument("--duration", type=float, default=240.0)
    ap.add_argument("--out", default=os.path.join(os.path.dirname(__file__), "..", "data"))
    args = ap.parse_args()
    sys.exit(run(args.provoke, args.out, args.duration))
