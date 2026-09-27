"""Exact-image mask CFG check and deliberately over-counted instruction model.
Not hardware WCET: peripheral/flash/bus stalls and IRQ response are not bounded.
"""
import hashlib
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
import isr_audit as audit
import isr_cycles as cycles

elf = ROOT / "captures/elf/0F2697A7.e479-commit-arm.elf"
assert hashlib.sha256(elf.read_bytes()).hexdigest().upper() == "0F2697A75AD55E96266A44099F1BD2D56085C93370A58BA5968DAE7335216CC1"
functions = audit.parse(subprocess.check_output(["arm-none-eabi-objdump", "-d", "-C", str(elf)], text=True))
rows = functions["ADC_COMP"].cfg
by_addr = {r[0]: r for r in rows}
next_addr = {a[0]: b[0] for a, b in zip(rows, rows[1:])}
start, end = 0x8000868, 0x8000AAE
assert by_addr[start][1] == "cpsid" and by_addr[end][1] == "msr"
pending, edges = [start], {}
while pending:
    address = pending.pop()
    if address in edges:
        continue
    _, mnemonic, target, operands = by_addr[address]
    assert not audit.opaque_transfer(mnemonic, operands)
    outgoing = []
    if address != end:
        assert mnemonic not in {"bx", "pop"}
        if mnemonic in audit.BRANCHES:
            assert target in by_addr
            outgoing.append(target)
        if mnemonic != "b":
            outgoing.append(next_addr[address])
    edges[address] = outgoing
    pending.extend(outgoing)
nodes = list(edges)
index = {address: i for i, address in enumerate(nodes)}
cycles._topo_order(len(nodes), [[index[x] for x in edges[a]] for a in nodes], set())

def charge(row):
    _, mnemonic, _, operands = row
    cost = cycles.cost(mnemonic, operands) + 2
    if mnemonic.startswith("ldr") and "[pc" in operands:
        cost += 2
    if mnemonic in audit.BRANCHES or mnemonic == "bl":
        cost += 4
    return cost

cost = sum(charge(by_addr[a]) for a in nodes)
for address in nodes:
    _, mnemonic, _, operands = by_addr[address]
    if mnemonic in audit.DIRECT_CALLS:
        name = audit.target_symbol(operands)
        assert name == "firmware50::roots::stop_expired_arm", name
        callee = functions[name]
        assert not callee.has_backward_branch and not callee.indirect and not callee.calls
        cost += sum(map(charge, callee.cfg))
print(f"mask CPSID {start:#x} -> restore {end:#x}: {len(nodes)} reachable instructions, acyclic")
print(f"sum of all alternative paths plus full shutdown callee: {cost} model cycles / {cost/64:.3f} us")
print("Conditional instruction model only, NOT hardware WCET or pending-guard latency.")
