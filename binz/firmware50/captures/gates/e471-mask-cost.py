"""ELF-specific over-count, not hardware WCET or permission to power.

Sum every instruction in the reviewed mask-address span, including mutually
exclusive paths. Add eleven extra copies of the 12-read persistence loop and
the complete straight-line shutdown callee. Flash penalty: two cycles per
instruction, two per PC-relative data load, two per branch target. Peripheral
and bus arbitration stalls, exception entry and pending guard service are NOT
bounded here. Addresses apply ONLY to C2F903A0, checked below.
"""
import hashlib
import importlib.util
import pathlib
import subprocess

ROOT = pathlib.Path(__file__).resolve().parents[2]
ELF = ROOT / "captures/elf/C2F903A0.e470-diode-atomic-arm.elf"
assert hashlib.sha256(ELF.read_bytes()).hexdigest().upper() == (
    "C2F903A045128B7D760DEF14504C3031B76977BE9678F69872C6E02CC14C48FB")
spec = importlib.util.spec_from_file_location("cycles", ROOT / "scripts/isr_cycles.py")
cycles = importlib.util.module_from_spec(spec)
spec.loader.exec_module(cycles)
sysroot = subprocess.check_output(["rustc", "--print", "sysroot"], text=True).strip()
objdump = pathlib.Path(sysroot) / "lib/rustlib/x86_64-pc-windows-msvc/bin/llvm-objdump.exe"
functions = cycles.load(str(objdump), str(ELF))
span = [x for x in functions["ADC_COMP"] if 0x80008b4 <= x[0] <= 0x8000ae4]
loop = [x for x in span if 0x80008d8 <= x[0] <= 0x80008f0]
assert len(loop) == 13  # Twelve on success; also charge the refusal branch.
assert span[0][1] == "cpsid" and span[-1][1] == "msr"
callee_name = next(n for n in functions if "16stop_expired_arm" in n)
callee = [x for x in functions[callee_name] if x[0] <= 0x8009176]
assert callee[-1][1] in ("bx", "pop"), callee[-1]
assert sum(m == "bl" for _, m, _ in span) == 1
assert not any(m == "bl" for _, m, _ in callee)
callee_addresses = {a for a, _, _ in callee}
def is_branch(mnemonic):
    return mnemonic in ("b", "b.n", "b.w") or mnemonic in {
        "b" + condition for condition in cycles.COND
    }

for a, mnemonic, operands in callee:
    if is_branch(mnemonic):
        target = cycles.target(operands)
        assert target in callee_addresses and target > a, "shutdown cycle/escape"

# Require all paths from CPSID to restore to stay in this span. Prove the
# reviewed persistence back-edge is the only cycle, not merely the only
# backward branch. Missing/indirect/out-of-span paths fail closed.
by_address = {row[0]: row for row in span}
addresses = list(by_address)
successor = {}
pending = [addresses[0]]
while pending:
    address = pending.pop()
    if address in successor:
        continue
    _, mnemonic, operands = by_address[address]
    assert not mnemonic.startswith("."), (address, "executable literal pool")
    edges = []
    if address != addresses[-1]:
        assert mnemonic != "bx" and not (mnemonic == "pop" and "pc" in operands)
        branch = is_branch(mnemonic)
        if branch:
            edges.append(cycles.target(operands))
        if mnemonic not in ("b", "b.n", "b.w"):
            edges.append(addresses[addresses.index(address) + 1])
    assert all(edge in by_address for edge in edges), (hex(address), edges)
    successor[address] = edges
    pending.extend(edges)
nodes = list(successor)
index = {a: i for i, a in enumerate(nodes)}
graph = [[index[b] for b in successor[a]] for a in nodes]
back = {(index[0x80008f0], index[0x80008d8])}
cycles._topo_order(len(nodes), graph, back)
print(f"mask CFG: {len(nodes)} reachable instructions; sole cycle is persistence")

def charge(row):
    _, mnemonic, operands = row
    if mnemonic.startswith("."):
        return 0  # Literal-pool data: not executable on the reviewed CFG.
    result = cycles.cost(mnemonic, operands) + 2
    if mnemonic.startswith("ldr") and "[pc" in operands:
        result += 2
    if mnemonic.startswith("b"):
        result += 4  # Overcharge each branch as taken + target-fetch penalty.
    return result

body = sum(map(charge, span))
repeats = 11 * sum(map(charge, loop))
shutdown = sum(map(charge, callee))
total = body + repeats + shutdown
print(f"span instructions/data rows={len(span)} cost={body}")
print(f"11 extra persistence iterations={repeats}; shutdown={shutdown}")
print(f"reviewed-span over-count={total} cycles = {total / 64:.3f} us")
print("Conditional instruction/fetch model only; NOT guard latency or hardware WCET.")
