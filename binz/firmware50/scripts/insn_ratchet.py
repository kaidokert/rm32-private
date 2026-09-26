"""Instruction-class ratchet for the ISR roots AND the foreground hot path.

Why this exists (E303). E291 fixed a real overflow in `BusDepth::observe` by
scaling both sides of a cross-product by 1/5, written inline as
`rhs * (self.fracs[i] / 5)`. `fracs` is a struct field, so the compiler could
not fold it: on thumbv6m it lowered to `bl __aeabi_uidiv` **inside** the
four-iteration loop, and `scan_pass` calls `observe` twice per scan -- eight
soft divisions per ADC scan at ~9.9 kHz, in the foreground that hosts every bus
judgement. Measured: `loop_iters_closed` fell 48% and the worst foreground gap
grew 49%.

**Nothing in the tree could see it.**

* `isr_diff.py` compares the four ISR roots and says so in its own docstring:
  thread mode is invisible to it. `BusDepth::observe` is thread mode.
* `isr_audit.py` and `WCET_ESTIMATES.md` cover ISR roots only.
* I read the disassembly twice, stopped at `movs r1, #5` without following the
  `bl` two instructions later, and reported the fix as verified.

So: count instruction CLASSES per symbol and refuse when they move. A division
appearing anywhere in a hot path becomes a build failure rather than something
a reader has to notice. Per [[feedback-instrument-must-fail-loudly]] the check
cannot pass by landing on the expected answer, because it asserts a baseline it
did not compute from the image under test.

Classes counted, and why each:

* `div`    -- `__aeabi_uidiv`/`idiv`/`uidivmod`/`idivmod`. No hardware divide on
              M0+, so each is a call of tens of cycles. The E303 class.
* `mul`    -- `muls` and `__aeabi_lmul`. Cheap but not free, and a widening
              multiply where a narrow one was is a silent cost
              ([[reference-m0-optlevel-s-softdiv]]).
* `irq`    -- `cpsid`/`cpsie`. Critical sections: a new one in a root or in the
              scan path changes the preemption structure, which is what the
              atomic stop/arm work exists to protect.
* `excl`   -- `ldrex`/`strex`. Should never appear on this part.
* `helper` -- any `bl` to an `__aeabi_*` or `mem*` symbol.
* `insns`  -- total, as a coarse drift signal with a little slack.

Usage:
    python scripts/insn_ratchet.py --elf path/to.elf
    python scripts/insn_ratchet.py --elf path/to.elf --bless
"""
from __future__ import annotations

import argparse
import json
import pathlib
import re
import subprocess

BASELINE = pathlib.Path(__file__).parent.parent / "captures" / "insn_baseline.json"

# The four motor ISR roots, plus the foreground symbols `isr_diff.py`
# structurally cannot see. The scan path is here because that is exactly where a
# per-scan cost landed with no existing gate noticing.
# The four ISR roots are reached through their VECTOR symbols, not their Rust
# names: LTO inlines `roots::comp_root` and friends into the `#[interrupt]`
# shims, so only `ADC_COMP` / `TIM16` / `TIM6_DAC_LPTIM1` / `DMA1_CHANNEL1`
# survive -- the same four `isr_diff.py` uses. Most protection helpers are
# inlined too; `BusDepth::observe` is the one that stays a symbol, and it is the
# one E303 was about.
WATCHED = (
    "ADC_COMP",
    "TIM16",
    "TIM6_DAC_LPTIM1",
    "DMA1_CHANNEL1",
    "protection::BusDepth::observe",
    # **Thread-mode additions (E308).** Both reviews of E305 landed on the same
    # structural point: `spent` is stamped INSIDE the COMP handler
    # (`roots.rs:1255-1257`), so COMP *entry* latency is invisible to
    # `spent_max_us`, `thin_count` and `late_arms` alike -- and entry latency is
    # set by thread-mode work and critical sections. The four ISR roots are
    # byte-identical across every image in this campaign, including the one that
    # has never late-armed, so whatever moves the failure rate is HERE, in the
    # foreground, where `isr_diff.py` says in its own docstring it cannot look.
    #
    # `record_sag_row` is the concrete instance: it takes a critical section
    # 9 840 times a second, which `sagtrace.rs`'s own module comment says is
    # unnecessary, and the measured consequence was `gt150` up 1.4-2.0x -- the
    # signature of delayed entry -- with `spent_max` unchanged at 11.
    "run::states::Ctx::scan_pass",
    "run::states::Ctx::record_sag_row",
    "sagtrace::Trace::push",
    # **E315.** A per-symbol objdump sweep found ten 64-bit division call sites
    # in the image, seven of them inside `Controller::run` -- and every one was
    # invisible to the old `DIV` pattern. They are not on the per-scan path
    # (`zero_from_blocks` has a single caller, the one-shot `averaged_zero`, and
    # `phase_rate` is a `const fn` folded at the rung table), so nothing is
    # wrong today. But "I reasoned it is not hot" is exactly the standard that
    # let E303 ship, so the orchestrator goes under the ratchet instead: it is
    # the symbol a future division would most plausibly land in.
    "run::Controller",
)

# A thin match is a silent pass: the first version of this script found 1 of 8
# symbols and cheerfully blessed a one-symbol baseline. Refuse instead.
MIN_SYMBOLS = 5

# Symbols that exist only in some build configurations. Their ABSENCE is not a
# failure -- `record_sag_row` and `Trace::push` are compiled out of a production
# build, and flagging that would make the ratchet refuse every normal image.
# Their hazard-class DRIFT is still a failure wherever they do appear.
#
# This is the honest form of a cross-configuration ratchet: without it, blessing
# from a recorder build refuses production and blessing from production refuses
# the recorder build, so the check would be switched off within a day.
OPTIONAL = frozenset({
    "run::states::Ctx::record_sag_row",
    "sagtrace::Trace::push",
})

CLASSES = ("insns", "div", "mul", "irq", "excl", "helper")

# **This toolchain emits several names for the same hazard**, and an enumeration
# of the ones I happened to have seen has now failed TWICE:
#
#  1. the first version matched only `__aeabi_*idiv*`, while this toolchain
#     calls `compiler_builtins::int::specialized_div_rem::u32_div_rem` -- so the
#     check could not see the very defect it was written for (E303);
#  2. the second version still matched only the 32-bit forms, so every 64-bit
#     division (`__aeabi_ldivmod`, `__aeabi_uldivmod`, `__divmoddi4`,
#     `__udivmoddi4`) was invisible -- and this image holds ten such call sites
#     (E315). A 64-bit divide is the most expensive arithmetic on this part, so
#     the detector was blind in precisely its worst case.
#
# So an allowlist of known-bad names cannot be the mechanism. `BUILTIN` matches
# ANY call into the compiler support library and `classify_builtin` must place
# each one in a named class or the run FAILS. A new helper name is then a loud
# refusal, not a silent zero ([[feedback-instrument-must-fail-loudly]]).
DIV = re.compile(
    r"__aeabi_u?l?idiv(mod)?"          # __aeabi_idiv/uidiv/idivmod/uidivmod
    r"|__aeabi_u?ldivmod"              # 64-bit __aeabi_ldivmod/__aeabi_uldivmod
    r"|u(32|64|128)_div_rem"           # compiler_builtins specialized_div_rem
    r"|__u?divmod(si|di|ti)4"          # __divmoddi4/__udivmoddi4
    r"|__u?div(si|di|ti)3"             # __udivsi3/__divsi3/__udivdi3
    r"|__u?mod(si|di|ti)3"             # the modulus siblings
    r"|__aeabi_[fd]div"                # soft float/double divide
)
MUL = re.compile(r"\bmuls?\b|__aeabi_lmul|__mul(di|ti)3|__aeabi_[fd]mul")
IRQ = re.compile(r"\bcps(id|ie)\b")
EXCL = re.compile(r"\b(ldrex|strex)\b")
HELPER = re.compile(
    r"\bbl\b.*(__aeabi_|memcpy|memset|memmove|memclr|compiler_builtins)")
BODY = re.compile(r"^\s+[0-9a-f]+:\s")

# Any branch-with-link into the compiler support library, whatever its name.
# **The `0x` is optional and that is not cosmetic.** `arm-none-eabi-objdump`
# prints `bl\t800874a <__aeabi_uidiv>` while `llvm-objdump` prints
# `bl\t0x800874a <__aeabi_uidiv>`; the first version of this pattern required
# the prefix, so against the objdump this script actually invokes it matched
# NOTHING and both absolute invariants silently passed on every image. Caught by
# running the falsification tests rather than by reading the output, which is
# the whole point of [[feedback-instrument-must-fail-loudly]].
BUILTIN = re.compile(
    r"\bbl\b\s+(?:0x)?[0-9a-f]+\s+<([^>]*(?:__aeabi_|__udiv|__div|__mod|__mul"
    r"|__ashl|__lshr|__ashr|_div_rem|compiler_builtins|memcpy|memset|memmove"
    r"|memclr)[^>]*)>")

# Classes a support-library call may legitimately fall into. Anything not placed
# here is an UNCLASSIFIED builtin and refuses the image.
MEM = re.compile(r"memcpy|memset|memmove|memclr|compiler_builtins::mem::")
SHIFT = re.compile(r"__aeabi_(llsl|llsr|lasr)|__ashldi3|__lshrdi3|__ashrdi3")
PANIC = re.compile(r"panic")


def classify_builtin(name: str) -> "str | None":
    """Name the hazard class of a support-library call, or None if unknown."""
    for cls, pat in (("div", DIV), ("mul", MUL), ("mem", MEM),
                     ("shift", SHIFT), ("panic", PANIC)):
        if pat.search(name):
            return cls
    return None


# **The strongest invariant in this file, and the only binary one.** The four
# motor ISR roots must call NO support-library routine at all: not a division,
# not a 64-bit multiply, not a memcpy. Measured true for every image in this
# campaign, and far harder to drift past than a class count, because there is no
# threshold to argue about. `advance_of`'s doc comment in `src/commutation.rs`
# records that `__aeabi_lmul` was refused at link time once already (E058); this
# makes that refusal general and automatic.
ISR_ROOTS = ("ADC_COMP", "TIM16", "TIM6_DAC_LPTIM1", "DMA1_CHANNEL1")


def text_symbols(elf: pathlib.Path) -> list[tuple[int, str]]:
    out = subprocess.run(["arm-none-eabi-nm", "-C", str(elf)],
                         capture_output=True, text=True, check=False).stdout
    got: list[tuple[int, str]] = []
    for line in out.splitlines():
        parts = line.split(" ", 2)
        if len(parts) == 3 and parts[1] in ("t", "T"):
            try:
                got.append((int(parts[0], 16), parts[2].strip()))
            except ValueError:
                pass
    return sorted(got)


def count(elf: pathlib.Path) -> dict[str, dict[str, int]]:
    syms = text_symbols(elf)
    addrs = sorted(a for a, _ in syms)
    result: dict[str, dict[str, int]] = {}
    for want in WATCHED:
        hit = next(((a, n) for a, n in syms if want in n), None)
        if hit is None:
            continue
        addr, _name = hit
        nxt = next((a for a in addrs if a > addr), addr + 512)
        end = min(nxt, addr + 4096)
        out = subprocess.run(
            ["arm-none-eabi-objdump", "-d",
             f"--start-address=0x{addr:08x}", f"--stop-address=0x{end:08x}",
             str(elf)],
            capture_output=True, text=True, check=False).stdout
        body = [l for l in out.splitlines() if BODY.match(l)]
        calls = [m.group(1) for l in body for m in [BUILTIN.search(l)] if m]
        result[want] = {
            "insns": len(body),
            "div": sum(1 for l in body if DIV.search(l)),
            "mul": sum(1 for l in body if MUL.search(l)),
            "irq": sum(1 for l in body if IRQ.search(l)),
            "excl": sum(1 for l in body if EXCL.search(l)),
            "helper": sum(1 for l in body if HELPER.search(l)),
        }
        # Absolute invariants, deliberately NOT baselined: a baseline would let
        # an unclassified builtin or an ISR-root helper call be blessed once and
        # then never noticed again, which is the failure this file exists for.
        result[want]["_unknown"] = sorted(
            {c for c in calls if classify_builtin(c) is None})
        result[want]["_builtins"] = sorted(set(calls))
    return result


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--elf", required=True)
    ap.add_argument("--config", default="default",
                    help="the FEATURE CONFIGURATION this ELF was built with, "
                         "naming its baseline bucket. ADC_COMP is 758 at "
                         "default features and 742 with advance-ref,"
                         "deep-filter, so one flat baseline reports every "
                         "feature build as an 18-instruction hazard drift; a "
                         "gate that always fires is not a gate. An unknown "
                         "bucket is REFUSED, never compared against another "
                         "configuration's numbers.")
    ap.add_argument("--bless", action="store_true",
                    help="write current counts as the baseline; only with a "
                         "stated reason in the notebook")
    ap.add_argument("--insn-slack", type=int, default=8,
                    help="allowed total-instruction drift per symbol "
                         "(default 8); the hazard classes get NO slack")
    args = ap.parse_args()

    elf = pathlib.Path(args.elf)
    if not elf.exists():
        print(f"no such ELF: {elf}")
        return 2
    now = count(elf)
    if len(now) < MIN_SYMBOLS:
        print(f"REFUSED: found {len(now)} of {len(WATCHED)} watched symbols "
              f"(need >= {MIN_SYMBOLS}).")
        print("  Either this is the wrong ELF, or a symbol was renamed or "
              "inlined and the ratchet would be silently measuring almost "
              "nothing.")
        print("  Found: " + ", ".join(sorted(now)))
        return 2

    # The two absolute invariants, checked BEFORE the baseline comparison and
    # before --bless, so neither can be blessed away.
    absolute: list[str] = []
    for k, v in sorted(now.items()):
        for name in v["_unknown"]:
            absolute.append(
                f"{k}: UNCLASSIFIED support-library call `{name}` -- add it to "
                f"a class in classify_builtin() and say what it costs")
        if any(k == r or k.startswith(r) for r in ISR_ROOTS) and v["_builtins"]:
            absolute.append(
                f"{k}: ISR root calls the support library: "
                f"{', '.join(v['_builtins'])}")
    if absolute:
        print(f"REFUSED ({len(absolute)}) -- absolute invariant, not ratcheted:")
        for a in absolute:
            print(f"  {a}")
        return 2

    # Only the ratcheted counts are persisted; `_unknown`/`_builtins` are
    # re-derived from every image so they cannot go stale in a baseline.
    ratcheted = {k: {c: v[c] for c in CLASSES} for k, v in now.items()}

    # **The baseline is keyed by feature configuration** (E346). It used to be a
    # flat symbol map blessed from one default-features build, so every
    # feature-built image -- including every image this campaign actually runs --
    # reported ADC_COMP 760 -> 742 as a hazard-class failure. That is how the
    # withdrawn qualification came to be recorded with `--insn-slack 30`: the
    # gate was tripping on the CONFIGURATION, so it got widened instead of fixed,
    # and a widened gate then covered whatever real drift existed.
    all_base: dict = {}
    if BASELINE.exists():
        all_base = json.loads(BASELINE.read_text(encoding="utf-8"))
        # Migrate a pre-E346 flat baseline (symbol -> counts) into the default
        # bucket rather than silently reading it as a bucket map.
        if all_base and not all(isinstance(v, dict) and "symbols" in v
                                for v in all_base.values()):
            all_base = {"default": {"symbols": all_base, "elf": "(pre-E346)"}}

    if args.bless or args.config not in all_base:
        if not args.bless and all_base:
            # Deny by default: never fall back to another configuration's
            # numbers, which is exactly the comparison that was broken.
            print(f"REFUSED: no baseline for configuration '{args.config}'.")
            print(f"  Known configurations: {', '.join(sorted(all_base))}")
            print("  Counts are configuration-specific, so comparing across "
                  "them is meaningless. Build this configuration, check the "
                  "counts by eye, and re-run with --bless once the notebook "
                  "says why.")
            return 2
        all_base[args.config] = {"symbols": ratcheted, "elf": elf.name}
        BASELINE.parent.mkdir(parents=True, exist_ok=True)
        BASELINE.write_text(json.dumps(all_base, indent=1, sort_keys=True)
                            + "\n", encoding="utf-8")
        verb = "blessed" if args.bless else "created"
        print(f"{verb} baseline for '{args.config}' from {elf.name}: "
              f"{len(ratcheted)} symbols")
        for k, v in sorted(ratcheted.items()):
            print(f"  {k:<44} {v}")
        return 0

    base = all_base[args.config]["symbols"]
    print(f"configuration '{args.config}' (baseline from "
          f"{all_base[args.config].get('elf', '?')})")
    fails: list[str] = []
    print(f"{'symbol':<44}{'insns':>12}{'div':>7}{'mul':>7}"
          f"{'irq':>7}{'excl':>7}{'helper':>8}")
    for k in sorted(set(base) | set(ratcheted)):
        b, n = base.get(k), ratcheted.get(k)
        if n is None:
            if k in OPTIONAL:
                print(f"{k:<44}{'(absent, optional)':>12}")
            else:
                fails.append(f"{k}: in the baseline, ABSENT from this ELF")
            continue
        if b is None:
            fails.append(f"{k}: in this ELF but NOT the baseline (bless to accept)")
            print(f"{k:<44}{'(new)':>12}")
            continue
        cells = []
        for c in CLASSES:
            d = n[c] - b[c]
            if c == "insns":
                if abs(d) > args.insn_slack:
                    fails.append(f"{k}: insns {b[c]} -> {n[c]} ({d:+d}), "
                                 f"beyond +-{args.insn_slack}")
            elif d != 0:
                # Hazard classes ratchet at zero drift in BOTH directions: a
                # count that fell is also a changed hot path and wants a reason.
                fails.append(f"{k}: {c} {b[c]} -> {n[c]} ({d:+d})")
            cells.append(f"{n[c]}" + ("" if d == 0 else f" ({d:+d})"))
        print(f"{k:<44}{cells[0]:>12}{cells[1]:>7}{cells[2]:>7}"
              f"{cells[3]:>7}{cells[4]:>7}{cells[5]:>8}")

    if fails:
        print(f"\nRATCHET FAILED ({len(fails)}):")
        for f in fails:
            print(f"  {f}")
        print("\nA hazard-class change is a build failure, not a note. If it is "
              "intended,\nsay why in the notebook and re-run with --bless.")
        return 1
    print("\nratchet OK: no hazard-class drift in any watched symbol")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
