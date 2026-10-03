"""Cargo linker wrapper that rejects soft arithmetic reachable from motor IRQs."""

import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys


IRQ_ROOTS = (
    "TIM6_DAC_LPTIM1", "TIM6_DAC", "TIM6_DACUNDER",
    "TIM14", "TIM1_UP_TIM16",
    "ADC_COMP", "COMP1_2_3", "ADC1_2",
    "DMA1_CHANNEL1", "DMA1_CH1", "DMA1_CH4_5_6_7_DMA2_CH3_4_5",
)


def forbidden(symbol: str) -> bool:
    patterns = (
        r"__(?:u?div|u?mod|mul|ashl|ashr|lshr|add|sub|neg|cmp|ucmp|multi|divmod).*ti[234]$",
        r"__aeabi_.*div|__(?:u?div|u?mod|divmod|udivmod)(?:si|di|ti)[234]$",
        r"__aeabi_(?:lmul|llsl|llsr|lasr|lcmp|ulcmp)|__(?:mul|ashl|ashr|lshr|add|sub|neg|cmp|ucmp)di[234]$",
        r"__aeabi_(?:[df]|[ui]2[df]|[lu]+2[df])|__(?:add|sub|mul|div|extend|trunc|fix|float|eq|ne|lt|le|gt|ge|unord).*(?:sf|df|tf)",
    )
    return any(re.search(pattern, symbol) for pattern in patterns)


def scan(disassembly: str):
    caller = None
    calls = []
    edges = {}
    for line in disassembly.splitlines():
        header = re.match(r"^([0-9a-fA-F]+) <(.+)>:$", line)
        if header:
            caller = header[2]
            continue
        insn = re.match(
            r"^\s*([0-9a-fA-F]+):\s+(?:[0-9a-fA-F]{4,8}\s+)+"
            r"([a-z][a-z0-9.]*)\s+(.+)$",
            line,
        )
        if not insn or insn[2] not in ("bl", "blx", "b", "b.w", "b.n"):
            continue
        target_match = re.search(r"<([^>]+)>", insn[3])
        if not target_match:
            continue
        target = re.sub(r"\+0x[0-9a-fA-F]+$", "", target_match[1])
        if caller is not None and caller != target:
            edges.setdefault(caller, set()).add(target)
        if caller is not None and caller != target and forbidden(target):
            calls.append({"address": "0x" + insn[1], "caller": caller, "target": target})
    return calls, edges


def reachable(calls, edges, root):
    seen = {root}
    pending = [root]
    while pending:
        caller = pending.pop()
        for target in edges.get(caller, ()):
            if target not in seen:
                seen.add(target)
                pending.append(target)
    return [call for call in calls if call["caller"] in seen]


def expanded_args(args):
    """Expand rustc/LLD response files enough to locate the linked output."""
    result = []
    for arg in args:
        if not arg.startswith("@"):
            result.append(arg)
            continue
        content = Path(arg[1:]).read_text(encoding="utf-8")
        # rustc emits quoted Windows paths and unquoted switches. Preserve
        # backslashes verbatim; shlex's POSIX escaping would corrupt them.
        result.extend(match[0] or match[1] for match in re.findall(r'"([^"]*)"|(\S+)', content))
    return result


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def audit_output(output: Path) -> int:
    """Audit one already-linked ELF and refresh its adjacent sidecars."""
    command = ["arm-none-eabi-objdump", "-d", "-S", "-C", str(output)]
    disassembly = subprocess.check_output(command, text=True, encoding="utf-8", errors="replace")
    calls, edges = scan(disassembly)
    violations = []
    root_counts = {}
    for root in IRQ_ROOTS:
        found = reachable(calls, edges, root)
        if root in edges or found:
            root_counts[root] = len(found)
        violations.extend((root, call) for call in found)

    report = {
        "elf": str(output),
        "sha256": sha256_file(output),
        "command": command,
        "roots": root_counts,
        "helper_calls_total": len(calls),
        "violations": [{"root": root, **call} for root, call in violations],
        "direct_call_edges": sum(len(targets) for targets in edges.values()),
        "limitations": ["direct calls only", "inline wide arithmetic requires manual review"],
    }
    Path(str(output) + ".math-audit.S").write_text(disassembly, encoding="utf-8")
    Path(str(output) + ".math-audit.json").write_text(
        json.dumps(report, indent=2) + "\n", encoding="utf-8"
    )
    for root, count in root_counts.items():
        print(f"M0 ISR math audit: {root} forbidden_reachable={count}", file=sys.stderr)
    if violations:
        for root, call in violations:
            print(
                f"M0 ISR math violation: {root}: {call['caller']} -> {call['target']}",
                file=sys.stderr,
            )
        return 1
    return 0


def main() -> int:
    args = sys.argv[1:]
    if len(args) == 2 and args[0] == "--audit-existing":
        output = Path(args[1])
        if not output.is_file():
            raise RuntimeError(f"ELF does not exist; arithmetic audit refused: {output}")
        return audit_output(output)
    rustc = os.environ.get("RUSTC", "rustc")
    sysroot = Path(subprocess.check_output([rustc, "--print", "sysroot"], text=True).strip())
    version = subprocess.check_output([rustc, "-vV"], text=True)
    host = next(line.split(": ", 1)[1] for line in version.splitlines() if line.startswith("host: "))
    linker = sysroot / "lib" / "rustlib" / host / "bin" / (
        "rust-lld.exe" if os.name == "nt" else "rust-lld"
    )
    linked = subprocess.run([str(linker), *args])
    if linked.returncode:
        return linked.returncode
    audit_args = expanded_args(args)
    if "-o" not in audit_args:
        raise RuntimeError(
            f"linked output not identified; arithmetic audit refused; argv={audit_args!r}"
        )

    output = Path(audit_args[audit_args.index("-o") + 1])
    return audit_output(output)


if __name__ == "__main__":
    sys.exit(main())
