"""Falsification tests for `insn_ratchet.py`'s two absolute invariants.

Why this file exists (E315). The ratchet's hazard allowlist had by then failed
three times, each time the same way: it printed a clean, plausible table while
being structurally unable to see the thing it was written to catch.

 1. E303 -- `DIV` matched only `__aeabi_*idiv*`, but this toolchain emits
    `compiler_builtins::int::specialized_div_rem::u32_div_rem`.
 2. E315 -- `DIV` still matched only the 32-bit forms, so all ten 64-bit
    division call sites in the image were invisible. A 64-bit divide is the
    most expensive arithmetic on this part, so the detector was blind in
    precisely its worst case.
 3. E315 -- the replacement `BUILTIN` pattern required an `0x` address prefix
    that `llvm-objdump` prints and `arm-none-eabi-objdump` (the one the script
    actually invokes) does not. Both new invariants matched nothing and passed
    on every image.

Defect 3 is the instructive one: it was introduced *while fixing* defect 2, by
the same author, in the same hour, and reading the output could not reveal it,
because a check that matches nothing looks exactly like a check that passes.
Only deliberately breaking the image tells the two apart
([[feedback-instrument-must-fail-loudly]]).

* TEST A declares a symbol that genuinely calls the support library to be an
  ISR root, and requires the zero-builtins invariant to refuse.
* TEST B disables one classifier and requires the unclassified-builtin
  invariant to refuse.
* TEST C requires every spelling of a division helper to classify as `div`.

Run it whenever `insn_ratchet.py` changes:

    python scripts/insn_ratchet_selftest.py
"""
from __future__ import annotations

import contextlib
import glob
import importlib.util
import io
import re
import sys

SPEC = importlib.util.spec_from_file_location("ir", "scripts/insn_ratchet.py")


def load():
    """A fresh module each time, so one test's monkeypatch cannot leak."""
    mod = importlib.util.module_from_spec(SPEC)
    SPEC.loader.exec_module(mod)
    return mod


def run(mod, elf: str) -> tuple[int, str]:
    sys.argv = ["insn_ratchet", "--elf", elf]
    buf = io.StringIO()
    with contextlib.redirect_stdout(buf):
        rc = mod.main()
    return rc, buf.getvalue()


def main() -> int:
    elfs = sorted(glob.glob("captures/elf/*.elf"))
    if not elfs:
        print("REFUSED: no ELF under captures/elf/ to test against.")
        return 2
    elf = elfs[-1]
    print("self-test against " + elf)
    print()
    failed: list[str] = []

    # A carrier for the invariants: a symbol that really does call a helper.
    # If this is empty the tests below would pass vacuously, so check it first.
    probe = load()
    counts = probe.count(__import__("pathlib").Path(elf))
    carriers = [k for k, v in counts.items() if v["_builtins"]]
    if not carriers:
        print("REFUSED: no watched symbol in this ELF calls the support "
              "library, so TEST A and TEST B would pass vacuously.")
        print("  Either BUILTIN matches nothing (the E315 defect) or this is "
              "the wrong ELF.")
        return 2
    carrier = carriers[0]
    print(f"carrier symbol (calls helpers): {carrier} "
          f"-> {counts[carrier]['_builtins']}")
    print()

    # TEST A -- the ISR-root zero-builtins invariant must refuse.
    mod = load()
    mod.ISR_ROOTS = mod.ISR_ROOTS + (carrier,)
    rc, out = run(mod, elf)
    ok = rc == 2 and "ISR root calls" in out
    if not ok:
        failed.append("A")
    print("TEST A  ISR root calls the support library:",
          "FIRED" if ok else "DID NOT FIRE")

    # TEST B -- the unclassified-builtin invariant must refuse.
    mod = load()
    mod.DIV = re.compile(r"\bZZZ_NO_MATCH_ZZZ\b")
    mod.MEM = re.compile(r"\bZZZ_NO_MATCH_ZZZ\b")
    rc, out = run(mod, elf)
    ok = rc == 2 and "UNCLASSIFIED" in out
    if not ok:
        failed.append("B")
    print("TEST B  unclassified support-library call:",
          "FIRED" if ok else "DID NOT FIRE")
    print()

    # TEST C -- every division spelling must classify as `div`.
    mod = load()
    spellings = (
        "__aeabi_uidiv", "__aeabi_idiv", "__aeabi_idivmod", "__aeabi_uidivmod",
        "__aeabi_ldivmod", "__aeabi_uldivmod", "__divmoddi4", "__udivmoddi4",
        "__udivsi3", "__divsi3", "__umodsi3",
        "compiler_builtins::int::specialized_div_rem::u32_div_rem",
        "compiler_builtins::int::specialized_div_rem::u64_div_rem",
    )
    for name in spellings:
        got = mod.classify_builtin(name)
        if got != "div":
            failed.append(f"C:{name}")
        short = name.rsplit("::", 1)[-1]
        print(f"TEST C  {short:22s} -> {got}"
              + ("" if got == "div" else "   <-- NOT SEEN AS A DIVISION"))

    if failed:
        print()
        print(f"SELF-TEST FAILED ({len(failed)}): " + ", ".join(failed))
        print("An invariant that cannot fire is worse than no invariant, "
              "because it reports success.")
        return 1
    print()
    print("self-test OK: both absolute invariants fire, every division "
          "spelling is seen")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
