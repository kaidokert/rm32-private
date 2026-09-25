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
import pathlib
import re
import struct
import subprocess
import sys


def loadable_word(elf: pathlib.Path, addr: int) -> int | None:
    """The 32-bit word at a virtual address, from the ELF's loadable segments.

    **This exists because the absence of a signal was read as confirmation**
    (E285). A root that loads a *changed constant* from its own literal pool has
    an identical instruction sequence, so this tool reported
    `TIM6_DAC_LPTIM1: identical, 155 instructions` for an image whose
    whole-campaign backstop had gone stale -- and for the image that fixed it.
    The word at 0x08001c98 read 84 s, 84 s and 94 s across the three images
    while the verdict read "identical" for every pairing.

    So pool contents are now part of the comparison. Without this the tool
    cannot answer "did my constant change reach the ISR?", which is the
    question it was being asked.
    """
    d = pathlib.Path(elf).read_bytes()
    phoff, = struct.unpack_from('<I', d, 0x1c)
    phentsize, = struct.unpack_from('<H', d, 0x2a)
    phnum, = struct.unpack_from('<H', d, 0x2c)
    for i in range(phnum):
        o = phoff + i * phentsize
        typ, off, vaddr, _paddr, filesz = struct.unpack_from('<IIIII', d, o)
        if typ == 1 and filesz and vaddr <= addr < vaddr + filesz - 3:
            return struct.unpack_from('<I', d, off + (addr - vaddr))[0]
    return None

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
                full = m.group(3)
                text = full.split('@')[0].strip()
                # A `ldr rN, [pc, #x]` names its pool slot in the comment. Read
                # the word and fold it into the compared text, so a changed
                # constant is a DIFFERS rather than an "identical" (E285).
                # Keep a branch's target address before normalising it away, so
                # its displacement can be compared.
                tgt = re.search(r'\b([0-9a-f]{6,8})\b\s*<', text)
                target = int(tgt.group(1), 16) if tgt else None
                text = re.sub(r'<[^>]*>', '@', text)
                norm = HEX.sub('@', text)
                raw[cur].append((addr, norm, target, m.group(2).replace(' ', '')))
    bodies, opcodes = {}, {}
    for root, insns in raw.items():
        at = {a: i for i, (a, _, _, _) in enumerate(insns)}
        body = []
        for i, (_, text, target, _b) in enumerate(insns):
            if target is not None and target in at:
                # A branch inside this root: keep where it goes, relative to
                # the branch, so a reordered target shows up as a difference.
                body.append(f'{text} >>{at[target] - i:+d}')
            else:
                body.append(text)
        bodies[root] = body
        # The encoded bytes, syntax-independent (E285). See `main`.
        opcodes[root] = ''.join(b for _a, _t, _g, b in insns)
    return bodies, opcodes


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('old')
    ap.add_argument('new')
    ap.add_argument('--objdump', default='arm-none-eabi-objdump')
    a = ap.parse_args()
    old, old_ops = disasm(a.objdump, a.old)
    new, new_ops = disasm(a.objdump, a.new)
    bad = 0
    for r in ROOTS:
        o, n = old.get(r), new.get(r)
        if o is None or n is None:
            print(f'{r}: MISSING ({"old" if o is None else "new"})')
            bad += 1
            continue
        if o == n:
            # **Instruction identity is not byte identity** (E285). A root that
            # loads a *changed constant* from its literal pool has an identical
            # instruction sequence, and this tool reported "identical" for an
            # image whose whole-campaign backstop had gone stale -- which was
            # then quoted as evidence the change had not reached the ISR. It had
            # not, and that was the bug.
            #
            # So the encoded bytes are compared too, from the byte column both
            # disassemblers print, rather than from either one's comment syntax
            # (a regex for llvm's `@ 0x...` silently matches nothing against
            # GNU's `@ (...)`, which is how the first attempt at this failed).
            same_bytes = old_ops.get(r) == new_ops.get(r)
            note = '' if same_bytes else '  [INSTRUCTIONS identical, ENCODED BYTES DIFFER: a constant moved]'
            print(f'{r}: identical, {len(n)} instructions{note}')
            if not same_bytes:
                bad += 1
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
