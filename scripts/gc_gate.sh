#!/usr/bin/env bash
# Generic-core refactor gate (docs/refactor_execution_plan.md S0.6).
#
#   scripts/gc_gate.sh <step> [<previous-step>]
#
# One command, one PASS/FAIL. Every piece runs under scripts/run_capped.py
# (hard 120 s cap, process tree killed), so nothing can hang the loop.
# Artifacts land in captures/gc/<step>/:
#   gate.log            everything, in order
#   l431.elf binz.elf   the PSU bench images (BENCH_SUPPLY=psu) — flash these
#   isr_l431/ isr_binz/ normalized ISR call trees (scripts/isr_diff.py)
#   sizes.txt           flash/RAM for the four public builds + both bench images
#   wcet_binz.txt       binz_wcet.py on the binz image
# With <previous-step>, the ISR dumps are diffed against captures/gc/<prev>/
# (reported, not failed: a CODE step is expected to differ, a HOST step must
# not — the step's class decides, see plan section 4).
#
# M0 link audit: runs inside every thumbv6m link (rm32_stm32/.cargo linker
# wrapper), so the G071/F051 builds below fail on soft arithmetic in a motor
# IRQ. C harness: ../am32_reqcheck/build/Release/am32_harness.exe; known
# C-vs-Rust differences in tests/blackbox/c_known_diffs.txt.
set -u
cd "$(dirname "$0")/.."
step=${1:?usage: gc_gate.sh <step> [<previous-step>]}
prev=${2:-}
out=captures/gc/$step
mkdir -p "$out"
log=$out/gate.log
: > "$log"
cap="python scripts/run_capped.py --secs 120 --"
fail=0
C_HARNESS=../am32_reqcheck/build/Release/am32_harness.exe

run() {  # run <name> <command...>
    local name=$1; shift
    printf '== %s\n' "$name" >> "$log"
    if "$@" >> "$log" 2>&1; then
        printf 'PASS  %s\n' "$name"
    else
        printf 'FAIL  %s (see %s)\n' "$name" "$log"
        fail=1
    fi
}

xbuild() {  # xbuild <feature> <target> [extra features]
    # From INSIDE rm32_stm32: cargo reads .cargo/config.toml from the cwd,
    # not from --manifest-path. Built from the repo root the image links
    # with no -Tlink.x (an empty ELF) and without the M0 audit linker.
    (cd rm32_stm32 && python ../scripts/run_capped.py --secs 120 -- cargo build --release \
        --bin rm32_firmware --no-default-features --features "$1${3:+,$3}" --target "$2")
}

check_elf() {  # a linked image has a vector table and code, not just .bss
    local text
    text=$(arm-none-eabi-size "$1" | awk 'NR==2{print $1}')
    if [ "${text:-0}" -lt 4096 ]; then
        printf 'FAIL  %s is not a linked image (text=%s)\n' "$1" "$text"
        fail=1
    fi
}

c_vectors() {
    $cap cmake --build ../am32_reqcheck/build --config Release --target am32_harness || return 1
    local got
    got=$(AM32_HARNESS=$C_HARNESS $cap python -m pytest tests/blackbox/test_vectors.py -q \
          -p no:cacheprovider -rf 2>&1 | tee -a "$log" \
          | sed -n 's/^FAILED tests.blackbox.test_vectors.py::test_vector\[\([^]]*\)\].*/\1/p' | sort)
    local known
    known=$(sed 's/#.*//' tests/blackbox/c_known_diffs.txt | awk 'NF{print $1}' | sort)
    local new fixed
    new=$(comm -23 <(echo "$got") <(echo "$known") | grep . || true)
    fixed=$(comm -13 <(echo "$got") <(echo "$known") | grep . || true)
    [ -n "$fixed" ] && echo "C harness now PASSES (remove from c_known_diffs.txt): $fixed"
    if [ -n "$new" ]; then echo "NEW C-vs-Rust difference: $new"; return 1; fi
    echo "C harness: $(echo "$got" | grep -c .) failures, all known"
}

run "host tests (cargo test --workspace)" $cap cargo test -q --workspace
run "clippy workspace" $cap cargo clippy -q --workspace -- -D warnings
run "rust harness build" $cap cargo build -q --release -p rm32-harness --bin rm32_harness
run "blackbox vectors (Rust harness)" $cap python -m pytest tests/blackbox/test_vectors.py -q -p no:cacheprovider
run "blackbox vectors (C harness vs known diffs)" c_vectors

# Public builds (what the pre-commit hook gates), sizes recorded after each:
# all four share one target dir per triple, so measure before the next build.
: > "$out/sizes.txt"
for fam in "stm32g071 thumbv6m-none-eabi" "stm32f051 thumbv6m-none-eabi" \
           "stm32l431 thumbv7em-none-eabihf" "stm32g431 thumbv7em-none-eabihf"; do
    set -- $fam
    run "build $1 (public)" xbuild "$1" "$2"
    check_elf "rm32_stm32/target/$2/release/rm32_firmware"
    { printf '%s public: ' "$1"; arm-none-eabi-size "rm32_stm32/target/$2/release/rm32_firmware" | tail -1; } >> "$out/sizes.txt"
    # Keep each production image: its ISR roots are what a CODE step that
    # removes bench hooks must show only removals on.
    cp "rm32_stm32/target/$2/release/rm32_firmware" "$out/pub_$1.elf"
    run "ISR dump $1 (public)" $cap python rm32_stm32/scripts/isr_diff.py dump --elf "$out/pub_$1.elf" --out "$out/isr_pub_$1"
done

# PSU bench images — the binaries every bench run of this step flies.
export BENCH_SUPPLY=psu
run "build L431 bench image (psu)" xbuild stm32l431 thumbv7em-none-eabihf benchuart
cp rm32_stm32/target/thumbv7em-none-eabihf/release/rm32_firmware "$out/l431.elf"
check_elf "$out/l431.elf"
BOARD=boards/binz_drv8304h_g071.yaml run "build binz bench image (psu)" xbuild stm32g071 thumbv6m-none-eabi benchuart
cp rm32_stm32/target/thumbv6m-none-eabi/release/rm32_firmware "$out/binz.elf"
check_elf "$out/binz.elf"
unset BENCH_SUPPLY
for b in l431 binz; do
    { printf '%s bench: ' "$b"; arm-none-eabi-size "$out/$b.elf" | tail -1; } >> "$out/sizes.txt"
done

run "binz WCET" sh -c "$cap python rm32_stm32/scripts/binz_wcet.py --elf $out/binz.elf > $out/wcet_binz.txt"
run "ISR dump L431" $cap python rm32_stm32/scripts/isr_diff.py dump --elf "$out/l431.elf" --out "$out/isr_l431"
run "ISR dump binz" $cap python rm32_stm32/scripts/isr_diff.py dump --elf "$out/binz.elf" --out "$out/isr_binz"
sha256sum "$out"/l431.elf "$out"/binz.elf | sed "s#$out/##" > "$out/elf.sha256"

if [ -n "$prev" ]; then
    for b in l431 binz pub_stm32g071 pub_stm32f051 pub_stm32l431 pub_stm32g431; do
        if [ ! -d "captures/gc/$prev/isr_$b" ]; then
            echo "ISR diff $b vs $prev ($step): no baseline dump in $prev"
            continue
        fi
        printf 'ISR diff %s vs %s (%s): ' "$b" "$prev" "$step"
        python rm32_stm32/scripts/isr_diff.py diff --summary "captures/gc/$prev/isr_$b" "$out/isr_$b" \
            | tee "$out/isr_diff_$b.txt" | tail -1
        # CODE steps: every ISR-root change must be a removal (HOST steps:
        # the diff above is already identical, so this passes trivially).
        printf '    removals: '
        python rm32_stm32/scripts/isr_diff.py removals "captures/gc/$prev/isr_$b" "$out/isr_$b" \
            | tee "$out/isr_removals_$b.txt" | tail -1
    done
fi

echo "---"
cat "$out/sizes.txt"
cat "$out/elf.sha256"
if [ $fail -eq 0 ]; then echo "GATE PASS ($step)"; else echo "GATE FAIL ($step)"; fi
exit $fail
