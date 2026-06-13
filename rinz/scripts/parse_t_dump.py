#!/usr/bin/env python3
"""
Parse all 't' command dumps from a motor_tester log.

Each triplet in the dump is:  A(ch17) C(ch14) B(ch5)  (2 hex chars each, 8-bit)
Triggered at CCR4=ARR/2: phases with duty>50% read HIGH, duty<50% read LOW.
Sectors are defined by (STEP/8), aligned to the A drive phase's sine zero-crossing.

Phase offsets in the sine table (from motor_tester.rs):
  va = SINE48[(STEP+4)%48]   ->  A drive peaks at s1 (STEP=8)
  vb = SINE48[(STEP+36)%48]  ->  B drive peaks at s3 (STEP=24)
  vc = SINE48[(STEP+20)%48]  ->  C drive peaks at s5 (STEP=40)

So in a mark=none dump, the expected HIGH sectors are:
  A(ch17): fully HIGH s0,s1  (positive STEP 45-47,0-19; partial s5,s2) -> peak s0
  B(ch5):  fully HIGH s2,s3  (positive STEP 13-35;      partial s1,s4) -> peak s2
  C(ch14): fully HIGH s4,s5  (positive STEP 29-47,0-3;  partial s3,s0) -> peak s4

Usage:
  python parse_t_dump.py [logfile]          # summary + averages
  python parse_t_dump.py [logfile] --raw    # add per-sector raw sample tables
"""
import re, sys
from pathlib import Path
from collections import defaultdict

HIGH = 80   # threshold: avg above this = HIGH half-cycle
LOW  = 25   # avg below this = LOW half-cycle

def parse_all(path):
    lines = Path(path).read_text().splitlines()
    mark_re   = re.compile(r'mark=(\S+)')
    header_re = re.compile(r't: (\d+) rev @ (\d+) Hz\s+(\S+)\s+(\S+)\s+(\S+)\s+(\d+)b')
    sector_re = re.compile(r'\[s(\d)\]:\s*([\da-f ]+)', re.IGNORECASE)

    current_mark = 'none'
    dumps = []
    i = 0
    while i < len(lines):
        line = lines[i]
        m = mark_re.search(line)
        if m:
            current_mark = m.group(1)
        h = header_re.search(line)
        if h:
            freq   = int(h.group(2))
            labels = [h.group(3), h.group(4), h.group(5)]
            sectors = {}
            j = i + 1
            while j < len(lines):
                s = sector_re.match(lines[j])
                if not s:
                    break
                sid = int(s.group(1))
                raw = s.group(2).replace(' ', '')
                trips = [raw[k:k+6] for k in range(0, len(raw), 6) if len(raw[k:k+6]) == 6]
                sectors[sid] = tuple(
                    [int(t[p*2:p*2+2], 16) for t in trips] for p in range(3)
                )
                j += 1
            dumps.append({'mark': current_mark, 'freq': freq,
                          'labels': labels, 'sectors': sectors})
            i = j
            continue
        i += 1
    return dumps


def savgs(sectors, ci):
    return [sum(sectors[s][ci]) / len(sectors[s][ci]) for s in range(6)]


def wave(avgs):
    return ''.join('#' if a > HIGH else ('.' if a < LOW else ':') for a in avgs)


def peak_sector(avgs):
    return avgs.index(max(avgs))


def print_raw(d, labels):
    """Print per-sector raw sample values as three parallel rows (A / C / B)."""
    for sid in range(6):
        sec = d['sectors'].get(sid)
        if sec is None:
            continue
        n = len(sec[0])
        avg = [sum(sec[ci]) / n for ci in range(3)]
        print(f"    [s{sid}] {n} samp   avg: "
              f"{labels[0]}={avg[0]:5.1f}  {labels[1]}={avg[1]:5.1f}  {labels[2]}={avg[2]:5.1f}")
        for ci, label in enumerate(labels):
            vals = '  '.join(f'{v:3d}' for v in sec[ci])
            print(f"      {label:<10} {vals}")
        print()


def main():
    args = sys.argv[1:]
    show_raw = '--raw' in args
    args = [a for a in args if a != '--raw']
    path = args[0] if args else 'logs/all_marked.txt'

    dumps = parse_all(path)
    if not dumps:
        sys.exit("No dumps found.")

    by_mark = defaultdict(list)
    for d in dumps:
        by_mark[d['mark']].append(d)

    labels = dumps[0]['labels']  # e.g. ['A(ch17)', 'C(ch14)', 'B(ch5)']

    print(f"\n  {len(dumps)} dumps  ({', '.join(f'{k}x{len(v)}' for k,v in by_mark.items())})")
    print(f"  CCR4=ARR/2 trigger: duty>50% -> HIGH (~106), duty<50% -> LOW (~0)\n")

    # ── mark=none: show baseline 3-phase sine profiles ──────────────────────
    none_dumps = by_mark.get('none', [])
    if none_dumps:
        print("  BASELINE (mark=none) — three 120-deg-offset sine phases")
        print("  " + "-" * 60)
        print(f"  {'channel':<12} {'s0':>6}{'s1':>6}{'s2':>6}{'s3':>6}{'s4':>6}{'s5':>6}"
              f"  waveform     peak-at")
        for ci, label in enumerate(labels):
            all_avgs = [savgs(d['sectors'], ci) for d in none_dumps]
            avgs = [sum(a[s] for a in all_avgs) / len(all_avgs) for s in range(6)]
            ps = peak_sector(avgs)
            vals = ''.join(f'{a:6.1f}' for a in avgs)
            print(f"  {label:<12} {vals}  {wave(avgs):<12} s{ps}")
        print()

    # ── per-mark per-dump detail ─────────────────────────────────────────────
    mark_order = ['A(ch17)', 'B(ch5)', 'C(ch14)', 'none']
    for mk in mark_order:
        if mk not in by_mark:
            continue
        for run_i, d in enumerate(by_mark[mk]):
            nsamp = sum(len(d['sectors'][s][0]) for s in d['sectors'])
            print(f"  mark={mk:<14} run {run_i+1}  {d['freq']} Hz  {nsamp} samp")
            print(f"  {'channel':<12} {'s0':>6}{'s1':>6}{'s2':>6}{'s3':>6}{'s4':>6}{'s5':>6}"
                  f"  waveform")
            for ci, label in enumerate(labels):
                avgs = savgs(d['sectors'], ci)
                print(f"  {label:<12} {''.join(f'{a:6.1f}' for a in avgs)}  {wave(avgs)}")
            if show_raw:
                print()
                print_raw(d, labels)
            else:
                print()

    # ── cross-check: compare each marked dump against mark=none baseline ─────
    if none_dumps:
        print("  PHASE IDENTITY (from mark=none peak sectors)")
        print("  " + "-" * 60)
        none_avgs = []
        for ci in range(3):
            all_a = [savgs(d['sectors'], ci) for d in none_dumps]
            avg_a = [sum(a[s] for a in all_a) / len(all_a) for s in range(6)]
            none_avgs.append(avg_a)

        expected_peaks = {
            # Fully-HIGH sectors per drive phase (CCR4=ARR/2 trigger):
            # A (offset +4):  positive STEP in [45..47,0..19] -> fully HIGH in s0,s1  -> peak s0
            # B (offset +36): positive STEP in [13..35]       -> fully HIGH in s2,s3  -> peak s2
            # C (offset +20): positive STEP in [29..47,0..3]  -> fully HIGH in s4,s5  -> peak s4
            # Both fully-HIGH sectors tie at ~106; max() picks the first (lower index).
            'A': 0,
            'B': 2,
            'C': 4,
        }

        for ci, label in enumerate(labels):
            ps      = peak_sector(none_avgs[ci])
            w       = wave(none_avgs[ci])
            exp_ch  = next((k for k, v in expected_peaks.items() if v == ps), None)
            match = f"= drive {exp_ch}" if exp_ch else f"peak s{ps} (unexpected)"
            print(f"  {label:<12}  waveform {w}  peak s{ps}  {match}")
        print()


if __name__ == '__main__':
    main()
