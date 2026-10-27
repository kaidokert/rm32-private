#!/usr/bin/env bash
# Build each named board's production image (no bench features) and print
# one size line per board: board tuning text data bss.
#   scripts/board_sizes.sh boards/flycolor_g071.yaml [boards/... ...]
# The tuning column is read from the yaml, so run it before and after a
# tuning change to compare.
set -euo pipefail
cd "$(dirname "$0")/../rm32_stm32"
for y in "$@"; do
    mcu=$(awk '/^mcu:/{print $2}' "$y")
    tuning=$(awk '/^tuning:/{print $2}' "$y")
    case "$mcu" in
        stm32g071|stm32f051) target=thumbv6m-none-eabi ;;
        stm32l431|stm32g431) target=thumbv7em-none-eabihf ;;
        *) echo "board_sizes: $y: unknown mcu '$mcu'" >&2; exit 1 ;;
    esac
    BOARD=$y cargo build -q --release --target "$target" --no-default-features --features "$mcu" >&2
    size=$(arm-none-eabi-size "target/$target/release/rm32_firmware" | awk 'NR==2{print $1, $2, $3}')
    [ -n "$size" ] || { echo "board_sizes: $y: no size" >&2; exit 1; }
    echo "$(basename "$y" .yaml) $tuning $size"
done
