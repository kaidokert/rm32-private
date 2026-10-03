# Cortex-M0 motor-ISR arithmetic audit

2026-09-15 port from the G071 `binz` campaign into main `rm32`.

## Result

The pre-change G071 release ELF contained seven software-division calls reachable
from motor interrupt roots:

- `TIM14`: one `__aeabi_uidiv` (`e_com / 3`).
- `TIM6_DAC_LPTIM1`: throttle mapping, active brake scaling, `e_com / 3`,
  voltage-ramp mapping, running PWM scaling, and proportional-brake scaling.

The current release ELF has **zero forbidden arithmetic helpers reachable by
direct calls** from all four G071 motor roots:

| root | forbidden reachable |
|---|---:|
| `TIM6_DAC_LPTIM1` | 0 |
| `TIM14` | 0 |
| `ADC_COMP` | 0 |
| `DMA1_CHANNEL1` | 0 |

Build from `rm32_stm32/` (the package is intentionally excluded from the
top-level host workspace):

```text
cargo build --release --bin rm32_firmware --no-default-features --features stm32g071
python scripts/audit_linker.py --audit-existing target/thumbv6m-none-eabi/release/rm32_firmware
SHA256 B701A77B59A0D7257D238DBE3F1B30A8D65B2FC8A012287C0A94E25990203650
text=40024 data=1784 bss=1144
```

The explicit second command is intentional: Cargo may reuse a cached ELF
without invoking the linker wrapper. Re-auditing the final path refreshes the
sidecars and records the ELF SHA-256 in JSON, so a stale report cannot be
mistaken for evidence about a different artifact.

The release profile is `opt-level="s"`, thin LTO, one codegen unit. The Cargo
target linker is now an audit wrapper: it performs the real link, disassembles
the resulting ELF, walks the emitted direct-call graph from every supported MCU
motor-vector name, and fails the build if division/remainder, 64/128-bit, or
software-floating helpers are reachable.

The audit still reports 69 helper call sites elsewhere (foreground control,
configuration, sine startup, formatting, and HAL initialization). Those are not
silently blessed; they are simply outside this first hard-real-time gate.

The post-tracking-loss product path is also covered by a sequence-level host
test. It proves that `AllOff` owns the first tick, the receiver command remains
data rather than output authority, ordinary startup begins at `min_startup`,
the existing 10 kHz duty ramp bounds the reclimb, and a newer receiver command
supersedes the retained request through that same ramp. This uses the normal
controller rather than adding a bench-specific recovery writer.

The current-limit ownership is likewise explicit: the continuous PID runs in
main, publishes `current_limit_adjust` atomically, and the 20 kHz ISR applies it
as the final ceiling after command, ramp, duty maximum, and stall boost. A host
regression proves a high command and positive stall boost cannot override the
published ceiling. The E777 bench-only 5% step, session-monotonic ceiling, and
1.5 A nominal constant are intentionally not duplicated here.

## Arithmetic replacement

`rm32::fast_math` contains exact, bounded operations rather than approximate
control changes:

- signed/unsigned divide by 3: 32-bit shift/add/correction;
- PWM-domain divide by 2000: factor 16, reciprocal multiply, two bounded
  corrections, valid through 16,000,000 (supported maximum is 14,164,000);
- voltage-ramp divide by 1400: at most nine subtracts on ramp-update ticks.

Tests exhaust every numerator through the declared `/2000` and `/1400` domains,
the complete motor timing domain for `/3`, every throttle/minimum mapping pair,
and PWM outputs for the F0/G0/G4 timer ranges. The existing core suite also
passes unchanged.

## Scope limits

The link gate follows emitted direct calls. It cannot see indirect calls, and
wide arithmetic that LLVM lowers inline still requires disassembly review. The
generated `.math-audit.S` and `.math-audit.json` sidecars remain next to each
linked artifact so that review is possible instead of inferred from source.
