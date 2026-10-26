#!/usr/bin/env bash
# L431 battery-guard bench image for a gc tag at the current 48 kHz fixed
# carrier, for bisecting battery-only behaviour across the batch.
#
#   scripts/gc_l431_k48_image.sh <tag> <outdir>
#
# Tags before fc3818c ran the L431 at 24 kHz variable; for those the two
# carrier lines fc3818c added to neutron_l431.yaml are appended (yaml only;
# build.rs already reads them), so every bisect point runs the same carrier.
# Writes <outdir>/l431.elf and prints its SHA-256.
set -eu
tag=${1:?tag}; out=$(realpath -m "${2:?outdir}")
root=$(cd "$(dirname "$0")/.." && pwd)
wt=$(mktemp -d)/wt
git -C "$root" worktree add -q --detach "$wt" "$tag"
trap 'git -C "$root" worktree remove --force "$wt"' EXIT
mkdir -p "$out"
cd "$wt/rm32_stm32"
if ! grep -q '^bench_pwm_khz:' boards/neutron_l431.yaml; then
    printf 'bench_pwm_khz: 48\nbench_fixed_pwm: true\n' >> boards/neutron_l431.yaml
    echo "carrier lines appended ($tag predates fc3818c)"
fi
grep -q '^bench_pwm_khz: 48' boards/neutron_l431.yaml || { echo "FAIL $tag carrier is not 48 kHz"; exit 1; }
export BENCH_SUPPLY=battery CARGO_TARGET_DIR="$root/rm32_stm32/target/battery"
python "$root/scripts/run_capped.py" --secs 300 -- cargo build -q --release --bin rm32_firmware \
    --no-default-features --features stm32l431,benchuart --target thumbv7em-none-eabihf
cp "$CARGO_TARGET_DIR/thumbv7em-none-eabihf/release/rm32_firmware" "$out/l431.elf"
sha256sum "$out/l431.elf"
