#!/usr/bin/env python3
"""Diff the four motor ISR roots' instructions between two linked ELFs.

Campaign 8 adds a second zero-cost recorder to the roots (`chain::ChainLog`,
E154). "Production untouched" is only a claim until the machine code is
compared, and the whole-image hash cannot show it: any library edit moves
addresses and static layout. So compare the roots themselves, with addresses
and their operands normalised away:

* mnemonic and operands are kept;
* absolute addresses, `pc`-relative comment targets and symbol suffixes are
  replaced by `@`, because a root that is byte-identical still lands at a
  different address in a different image;
* branch *offsets within the root* are kept as relative displacements, so a
  reordered branch still shows up.

Usage:  python scripts/isr_diff.py OLD.elf NEW.elf [--objdump arm-none-eabi-objdump]
Exit 0 if every root matches instruction for instruction, 1 otherwise.
"""
import argparse
import re
import subprocess
import sys

ROOTS = ('DMA1_CHANNEL1', 'ADC_COMP', 'TIM16', 'TIM6_DAC_LPTIM1')
HEX = re.compile(r'0x[0-9a-f]+|\b[0-9a-f]{6,8}\b')


def disasm(objdump, elf):
    """{root: [normalised instruction, with intra-root branches' displacement]}."""
    out = subprocess.check_output([objdump, '-d', elf], text=True, errors='replace')
    raw, cur = {}, None
    for line in out.splitlines():
        m = re.match(r'^([0-9a-f]+) <([^>]+)>:$', line.strip())
        if m:
            cur = m.group(2)
            if cur in ROOTS:
                raw[cur] = []
            continue
        if cur in raw:
            m = re.match(r'^\s*([0-9a-f]+):\s+((?:[0-9a-f]{4} ?)+)\s*\t(.*)$', line)
            if m:
                addr = int(m.group(1), 16)
                text = m.group(3).split('@')[0].strip()
                # Keep a branch's target address before normalising it away, so
                # its displacement can be compared.
                tgt = re.search(r'\b([0-9a-f]{6,8})\b\s*<', text)
                target = int(tgt.group(1), 16) if tgt else None
                text = re.sub(r'<[^>]*>', '@', text)
                raw[cur].append((addr, HEX.sub('@', text), target))
    bodies = {}
    for root, insns in raw.items():
        at = {a: i for i, (a, _, _) in enumerate(insns)}
        body = []
        for i, (_, text, target) in enumerate(insns):
            if target is not None and target in at:
                # A branch inside this root: keep where it goes, relative to
                # the branch, so a reordered target shows up as a difference.
                body.append(f'{text} >>{at[target] - i:+d}')
            else:
                body.append(text)
        bodies[root] = body
    return bodies


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('old')
    ap.add_argument('new')
    ap.add_argument('--objdump', default='arm-none-eabi-objdump')
    a = ap.parse_args()
    old, new = disasm(a.objdump, a.old), disasm(a.objdump, a.new)
    bad = 0
    for r in ROOTS:
        o, n = old.get(r), new.get(r)
        if o is None or n is None:
            print(f'{r}: MISSING ({"old" if o is None else "new"})')
            bad += 1
            continue
        if o == n:
            print(f'{r}: identical, {len(n)} instructions')
            continue
        bad += 1
        print(f'{r}: DIFFERS, {len(o)} -> {len(n)} instructions')
        for i in range(max(len(o), len(n))):
            x = o[i] if i < len(o) else '--'
            y = n[i] if i < len(n) else '--'
            if x != y:
                print(f'  [{i}] {x!r} -> {y!r}')
    return 1 if bad else 0


if __name__ == '__main__':
    sys.exit(main())
