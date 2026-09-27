"""Exact staged-image mask inspection, conditional cycle model, not WCET."""
import hashlib
import pathlib
import subprocess
import sys

root = pathlib.Path(__file__).resolve().parents[2]
sys.path.insert(0, str(root / "scripts"))
import isr_audit as audit
import isr_cycles as cycles

elf = root / "captures/elf/3023DE9E.e486h-slow-entry.elf"
assert hashlib.sha256(elf.read_bytes()).hexdigest().upper() == "3023DE9E694B3E2F679184FF83F9ADF02950A256FF8A788C8529EAAD6A34044A"
asm = subprocess.check_output(["arm-none-eabi-objdump", "-Cd", str(elf)], text=True)
rows = {r[0]: r for f in audit.parse(asm).values() for r in f.cfg}
start, end = 0x80035C8, 0x8003620
span = [row for address, row in sorted(rows.items()) if start <= address <= end]
assert span[0][1] == "cpsid" and span[-1][1] == "msr"
for address, op, target, args in span:
    assert op not in audit.DIRECT_CALLS
    assert not audit.opaque_transfer(op, args)
    if op in audit.BRANCHES:
        assert address < target <= end, (address, target)
cost = sum(cycles.cost(op, args) + 2 + (4 if op in audit.BRANCHES else 0)
           for _, op, _, args in span)
print(f"3023DE9E hold boundary {start:#x}..{end:#x}: {len(span)} instructions, no calls/backedges")
print(f"All alternatives counted, +2 fetch cycles/instruction +4/branch: {cost} cycles, {cost/64:.3f} us")
print("Conditional model excludes unbounded bus arbitration; not hardware WCET or edge latency.")
