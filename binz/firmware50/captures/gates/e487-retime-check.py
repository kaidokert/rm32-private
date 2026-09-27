"""Exact image, conservative conditional instruction model; not hardware WCET."""
import hashlib
import pathlib
import subprocess
import sys
root = pathlib.Path(__file__).resolve().parents[2]
sys.path.insert(0, str(root / 'scripts'))
import isr_audit as audit
import isr_cycles as cycles
elf = root / 'captures/elf/0EE71575.e486i-slow-entry.elf'
assert hashlib.sha256(elf.read_bytes()).hexdigest().upper() == '0EE715753A7DB386C6D22DEB9514A8BA79B12E14E424B9BD1C285CBB76B5AC4A'
asm = subprocess.check_output(['arm-none-eabi-objdump', '-Cd', str(elf)], text=True)
rows = {r[0]: r for f in audit.parse(asm).values() for r in f.cfg}
def total(lo, hi):
    return sum(cycles.cost(op, args) + 2 + (4 if op in audit.BRANCHES else 0)
               for at, op, _, args in rows.values() if lo <= at <= hi)
# All alternatives counted. The only stage loop writes three stack words.
outer = total(0x8009744, 0x800979c)
stage = total(0x8008db0, 0x8008e18) + 2 * total(0x8008df4, 0x8008dfc)
# r2=5<<6=320. memcpy4 executes 20 iterations of its 16-byte loop,
# then calls generic memcpy with remainder zero. Prefix alternatives overcount.
copy = total(0x800ad56, 0x800adca) + 19 * total(0x800ad98, 0x800adb0)
zero = total(0x800a3de, 0x800a3e6) + total(0x800a414, 0x800a41c) + total(0x800a4e4, 0x800a4e6)
print(f'outer={outer}, stage3={stage}, copy320={copy}, zero_tail={zero} cycles')
print(f'Conditional sum={outer+stage+copy+zero} cycles={(outer+stage+copy+zero)/64:.3f} us')
print('No hardware-WCET claim: flash/bus arbitration assumptions unmeasured. One-time transition only.')
