"""Exact-image foreground mask inspection; conditional model, not hardware WCET."""
import hashlib
import pathlib
import re
import subprocess
import sys

root = pathlib.Path(__file__).resolve().parents[2]
sys.path.insert(0, str(root / "scripts"))
import isr_audit as audit
import isr_cycles as cycles

elf = root / "captures/elf/869B7E4E.e483-stop-write.elf"
assert hashlib.sha256(elf.read_bytes()).hexdigest().upper() == "869B7E4EC70F1C81E56ED6F61F0F40C5DD17DD5FAF4E461DEF6C9A25809DFDE5"
asm = subprocess.check_output(["arm-none-eabi-objdump", "-d", "-S", "-C", str(elf)], text=True)
plain = subprocess.check_output(["arm-none-eabi-objdump", "-d", "-C", str(elf)], text=True)
rows = {row[0]: row for fn in audit.parse(plain).values() for row in fn.cfg}
lines = asm.splitlines()
found = 0
for i, line in enumerate(lines):
    if "if firmware50::oneshot::arm_allowed(" not in line:
        continue
    start_line = next(x for x in reversed(lines[:i]) if "cpsid" in x)
    end_line = next(x for x in lines[i:] if re.match(r"\s*[0-9a-f]+:", x) and "msr" in x and "PRIMASK" in x)
    start = int(re.match(r"\s*([0-9a-f]+):", start_line)[1], 16)
    end = int(re.match(r"\s*([0-9a-f]+):", end_line)[1], 16)
    span = [row for address, row in sorted(rows.items()) if start <= address <= end]
    assert span[0][1] == "cpsid" and span[-1][1] == "msr"
    backward = []
    for address, mnemonic, target, operands in span:
        assert mnemonic not in audit.DIRECT_CALLS
        assert not audit.opaque_transfer(mnemonic, operands)
        if mnemonic in audit.BRANCHES:
            assert start <= target <= end
            if target < address:
                backward.append((target, address))
    # Only the source's three-phase mapping loop may repeat. Count every
    # alternative branch in each iteration (deliberately pessimistic).
    assert len(backward) <= 1
    def cost(row):
        _, op, _, args = row
        return cycles.cost(op, args) + 2 + (4 if op in audit.BRANCHES else 0)
    total = sum(map(cost, span))
    for lo, hi in backward:
        total += 2 * sum(cost(row) for row in span if lo <= row[0] <= hi)
    print(f"{start:#x}..{end:#x}: {len(span)} listed instructions, loops={len(backward)}, model={total} cycles/{total/64:.3f}us")
    found += 1
assert found == 6, found
print("Six guarded foreground sites; conditional instruction model only, not hardware WCET.")
