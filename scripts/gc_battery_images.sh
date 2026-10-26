#!/usr/bin/env bash
# Battery-guard bench images (BENCH_SUPPLY=battery) for a gc tag, for the
# RISKY-step battery batch (plan section 6 step 8).
#
#   scripts/gc_battery_images.sh <tag> <outdir>
#
# Builds <tag> in a throwaway worktree, writes <outdir>/{l431,binz}.elf and
# prints their SHA-256. Same commands as gc_gate.sh's PSU images, only the
# supply differs (binz: BINZ_BATTERY 8 A / 15 A surge / 17.5 % sag; L431:
# battery limits).
set -eu
tag=${1:?tag}; out=$(realpath -m "${2:?outdir}")
root=$(cd "$(dirname "$0")/.." && pwd)
wt=$(mktemp -d)/wt
git -C "$root" worktree add -q --detach "$wt" "$tag"
trap 'git -C "$root" worktree remove --force "$wt"' EXIT
mkdir -p "$out"
cd "$wt/rm32_stm32"
export BENCH_SUPPLY=battery CARGO_TARGET_DIR="$root/rm32_stm32/target/battery"
python "$root/scripts/run_capped.py" --secs 300 -- cargo build -q --release --bin rm32_firmware \
    --no-default-features --features stm32l431,benchuart --target thumbv7em-none-eabihf
cp "$CARGO_TARGET_DIR/thumbv7em-none-eabihf/release/rm32_firmware" "$out/l431.elf"
BOARD=boards/binz_drv8304h_g071.yaml python "$root/scripts/run_capped.py" --secs 300 -- cargo build -q \
    --release --bin rm32_firmware --no-default-features --features stm32g071,benchuart \
    --target thumbv6m-none-eabi
cp "$CARGO_TARGET_DIR/thumbv6m-none-eabi/release/rm32_firmware" "$out/binz.elf"
for f in "$out"/l431.elf "$out"/binz.elf; do
    text=$(arm-none-eabi-size "$f" | awk 'NR==2{print $1}')
    [ "$text" -ge 4096 ] || { echo "FAIL $f not a linked image"; exit 1; }
done
sha256sum "$out"/l431.elf "$out"/binz.elf
