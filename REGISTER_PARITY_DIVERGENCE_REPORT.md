# Register-parity divergence report — rm32 L431 vs AM32 L431

## Principle

**rm32 is intended to be a bit-for-bit register-level port of the AM32 C firmware.** That means the post-init state of every memory-mapped peripheral register touched by either firmware on the *same hardware (Vimdrones L431, PA2 signal pin)* should be **byte-identical** between the two. Where they differ, either:

- rm32 is intentionally doing something C doesn't (e.g. the `debuguart` feature stealing USART1+PB6 for a plaintext debug log — fine, documented), or
- it's a port bug (common; cause of every runtime issue we've debugged this past week).

**"Same outcome via a different register path" is also a port bug**, not an acceptable variation. Same SYSCLK from a different PLL decomposition produces different jitter spectra. Same 20 kHz tick from a different PSC/ARR produces different `CNT` readback scaling. Same channel enable from a different write order produces different transient states observed by any peripheral that races against the init. The reference C is the spec. Match it exactly.

The default position is *bit-for-bit parity*. Divergences are guilty until proven innocent.

## Verification methodology

`E:/m/robot/esc/scripts/dump_l431_regs.py` reads ~35 peripheral registers via `probe-rs read` without halting the core. Run once with each firmware flashed and `diff -u` the outputs. Procedure:

```
# flash AM32 + bootloader, reset, wait ~6 s
python scripts/dump_l431_regs.py > am32_regs.txt

# flash rm32, reset, wait ~6 s
python scripts/dump_l431_regs.py > rm32_regs.txt

diff -u am32_regs.txt rm32_regs.txt
```

Both firmwares ran at idle (motor disarmed, BF sending bidir DSHOT) at dump time.

## Divergences found (2026-05-14)

Decoded from the `diff` of `am32_regs.txt` vs `rm32_regs.txt`. SYSCLK is identical at 80 MHz despite different PLL paths (cosmetic). The differences below are not cosmetic.

| Register | AM32 | rm32 | Decode |
|---|---|---|---|
| **GPIOA.PUPDR** (bits 4..5, PA2) | `01` (pull-up) | `00` (no pull) | rm32's signal pin **floats** when BF goes high-Z during bidir-response slot → captures comparator noise → 50% CRC fail rate |
| **GPIOA.OSPEEDR** (bits 4..5, PA2) | `10` (medium) | `00` (low) | rm32's TX slew rate is too slow for clean bidir telemetry response edges |
| **GPIOA.AFRL** (bits 28..31, PA7) | `1` (AF1) | `0` (AF0=disabled) | AM32 routes PA7 to alternate function; rm32 doesn't. (PA7 is BEMF comparator pin — could affect motor commutation timing.) |
| **RCC.APB1ENR1** bit 5 (TIM7EN) | `1` | `0` | AM32 enables TIM7, rm32 doesn't. AM32 may use TIM7 for telemetry-response timing. |
| **TIM15.PSC** | `0x01` (40 MHz tick) | `0x0D` (5.71 MHz detection mode) | AM32 uses fixed PSC=1 always. rm32 has *dynamic* PSC that switches between detection (13) and post-detection (1 or 3) values — invites mid-frame timing glitches. |
| **TIM15.SR** CC1OF flag | clear | **set** | rm32 has **capture overflow** — captures being dropped. AM32 doesn't. |
| **TIM15.CCR1** | `0x0000` | **`0x9AE2`** | rm32 has unread capture pending. DMA not keeping up. AM32's CCR1 is empty. |
| **DMA1.ISR** | `0x00` | **`0x07` (TC+HT+GIF)** | rm32 not clearing DMA flags after each cycle. AM32 does. |
| **DMA1.CNDTR5** | `0x20` (32 = full) | `0x1C` (28 remaining) | rm32 mid-burst at dump time; AM32 was idle. |
| GPIOA.MODER bits 4..5 (PA2) | `10` ✓ | `10` ✓ | both: alternate function. Match. |
| TIM15.CCMR1 | 0x41 ✓ | 0x41 ✓ | match |
| TIM15.CCER | 0x0B ✓ | 0x0B ✓ | match (both-edge capture, enabled) |
| TIM15.ARR | 0xFFFF ✓ | 0xFFFF ✓ | match |
| RCC.PLLCFGR | 0x01001412 | 0x01000A02 | different PLL config, same SYSCLK (`HSI16/M*N/R` = 80 MHz both ways). Cosmetic. |
| RCC.AHB1ENR / AHB2ENR / AHB3ENR / APB1ENR2 / APB2ENR | match | match | bus enables OK |
| FLASH.ACR | 0x604 ✓ | 0x604 ✓ | match |
| SCB.VTOR | 0x08001000 ✓ | 0x08001000 ✓ | both apps loaded correctly |

**Highest-confidence bug**: PA2 PUPDR. Without the pull-up, the line floats during BF's bidir-response listening slot; the floating line picks up noise that the comparator interprets as DSHOT edges, the DMA captures them, the decoder fails CRC. This is the most direct cause of the ~50% CRC failure rate we observed at the bench.

## Full audit results (2026-05-14, 6 parallel subagents)

Six agents investigated peripheral groups. Findings consolidated and ranked.

### CRITICAL — likely root cause of observed runtime bugs

| # | Register / behavior | AM32 (C) | rm32 (Rust) | Symptom this likely caused |
|---|---|---|---|---|
| C1 | **TIM15.PSC for input capture** | Dynamic 0..13, set per detected protocol (PSC=0 for DSHOT600 full 80 MHz, 1 for DSHOT300, etc.) | Hardcoded **13** in `mcu_l431/input_capture.rs:125` (`80/6`), never re-tuned for DSHOT600. | Coarse edge-timing for DSHOT600 (~42 ns per tick vs 12.5 ns on C). At DSHOT600 bit pulse widths of ~0.6 µs, that's only 14 ticks — barely above the heuristic threshold. Plausible cause of intermittent decode + the observed 50% CRC fail rate at bidir. |
| C2 | **PA2 OSPEEDR (DSHOT signal pin)** | `HIGH` speed | unset (defaults to **LOW**) | When rm32 acts as bidir TX, slew rate is too slow for clean GCR-encoded telemetry response edges → BF rejects telemetry → handshake fails. |
| C3 | **PA2 PUPDR (DSHOT signal pin)** | hardware dump shows **01 (pull-up)** | hardware dump shows **00 (no pull)** | When BF goes high-Z during bidir telemetry slot, line floats → comparator triggers on noise → captured as garbage "frames" with wrong timing. Direct cause of the 50% CRC fail rate. (Agent A's source read shows C sets PUPDR=NO at init, but the *runtime* register has pull-up — `setInputPullUp()` is called later. rm32 either never calls it or sets a different pull state.) |
| C4 | **DSHOT TX NDTR (transmit length)** | C uses **`23 + buffer_padding`** where `buffer_padding ∈ {0, 7, 14}` per DSHOT600/300/150. For DSHOT150 → CNDTR=**37**. | Rust uses **fixed 31** in `capture_generic.rs::send_dshot_dma`. | DSHOT150 bidir telemetry response is **truncated** — BF receives a short / malformed GCR frame, link unhealthy. |
| C5 | **IWDG timeout** | PSC = `LL_IWDG_PRESCALER_16` (/16 from LSI ~32 kHz), RLR=4000 → **2.0 s** timeout. | `WDG_PRESCALER=2` (/4 divider), RLR=4000 → **0.5 s** timeout — **4× faster reset**. | rm32 chip resets sooner under stall / heavy load. If the chip ever fails to feed the watchdog inside 500 ms (including during init startup or bidir handshake stalls), it resets — visible to user as "random reboot." We've had the watchdog disabled for bench debug all session, so we've been masking this. |

### HIGH — confirmed divergences, not yet linked to a specific symptom

| # | Register / behavior | AM32 (C) | rm32 (Rust) | Impact |
|---|---|---|---|---|
| H1 | **PA7 init** | configured (AF1 = TIM1_CH1N complementary PWM output) | **not initialized at all** in Rust `init.rs` | One of the three complementary PWM outputs is in default GPIO state. Could cause shoot-through risk or motor phase imbalance. Verify via scope. |
| H2 | **PA8/PA9/PA10 OSPEEDR + OTYPER** | C sets OSPEEDR explicit + OTYPER conditional (push-pull or open-drain per build flag) | Rust only sets MODER + AFRH, leaves OSPEEDR/OTYPER at reset defaults | Slow slew rate on motor PWM = soft FET switching = wasted power + heat at high PWM frequencies. |
| H3 | **PB6 OTYPER (USART1 telemetry TX)** | C: **push-pull** | Rust: **open-drain** | Intentional design choice in Rust for single-wire half-duplex robustness. Marginal: BF with internal pull-up tolerates it, but slew rate is slower than push-pull. Not necessarily a bug; flag for awareness. |
| H4 | **Servo half-transfer (HT) interrupt** | C handles DMA HT to switch capture polarity mid-burst for servo PWM | Rust **has no HT handler** | Servo PWM path is incomplete in rm32 (matches an earlier session's "servo polarity sometimes wrong" observation). |
| H5 | **TIM7 clock enable** | enabled (APB1ENR1 bit 5), TIM7 initialized (PSC=79, ARR=65535) | not enabled | Agent D confirms TIM7 is dead code in AM32 — it's initialized but never armed. So this divergence is **cosmetic** for now, but worth removing in AM32 too if confirmed unused. |
| H6 | **Backup domain write enable** | C enables `PWR.CR1.DBP` (clears backup-domain write protection) so LSE / RTC config in `RCC.BDCR` is writable | Rust doesn't | RTC and LSE are unreachable from rm32 firmware. Probably fine for ESC use but worth noting. |

### REVERSE DIVERGENCE — Rust may be MORE correct than C

| # | Register | AM32 (C) | rm32 (Rust) | Note |
|---|---|---|---|---|
| R1 | **COMP2 INMESEL** (BEMF comparator negative-input subselect) | C leaves at reset value → defaults to **DAC1 output** | Rust explicitly sets to **external BEMF pins** via board YAML | Agent C's analysis: AM32 may be comparing motor BEMF against the DAC output (a fixed reference, ~0.6 V) instead of the actual BEMF phase voltage. If true, this would explain AM32's BEMF detection working "by chance" rather than design. Worth **physical verification** (probe BEMF pin vs comparator output) before concluding either direction. |

### HIGH — "same outcome via different path" divergences

**These are NOT cosmetic.** "Same SYSCLK from different PLL config" or "same 20 kHz from different PSC/ARR" both produce real side effects: different jitter, different CNT readback scaling, different power profile, different default-state assumptions in downstream code. Operating policy is **bit-for-bit identical register state** unless rm32 is intentionally doing something C doesn't (like the `debuguart` feature stealing USART1+PB6).

| # | Register / behavior | AM32 (C) | rm32 (Rust) | Side effect |
|---|---|---|---|---|
| H7 | **PLLCFGR M/N decomposition** | M=2, N=20, R=2 → VCO input 8 MHz, VCO output 160 MHz | M=1, N=10, R=2 → VCO input 16 MHz, VCO output 160 MHz | Different VCO input frequency → different PLL lock time. Different N multiplication ratio (×20 vs ×10) → different phase-noise multiplication of HSI reference. For sub-µs DSHOT timing measurement, this matters. |
| H8 | **TIM6.PSC / TIM6.ARR decomposition** | PSC=79, ARR=50 → counter ticks at 1 MHz, fires every 50 µs (20 kHz) | PSC=0, ARR=3999 → counter ticks at 80 MHz, fires every 50 µs (20 kHz) | Same ISR rate but **`TIM6.CNT` reads at totally different scales**. Any code that reads CNT for intra-period interpolation gets a different number. Also: prescaler-counter wrap behavior differs (PSC=79 cycles every 80 SYSCLK, PSC=0 never wraps). |
| H9 | **NVIC priority initialization** | C sets explicit priorities at init (`TIM6_DAC=3`, `TIM1_UP_TIM16=0`, `COMP=0`, `DMA1_CH5=1`, etc.) | rm32 unmasks at default priority (0 for all), then `adjust_irq_priorities()` rebalances dynamically based on motor speed | Even ignoring the dynamic adjustment: **at boot, before the first dynamic-adjust call**, rm32 has all IRQs at priority 0 (all equal, no preemption). AM32 has structured priority ordering from cycle 1. Boot-time ISR interactions could resolve differently. |
| H10 | **USART1 clock source** | C explicit `RCC->CCIPR` write selecting PCLK2 | rm32 trusts reset default (which is also PCLK2 on L4) | Defensively-set vs trusting-defaults. Reset value is correct today, but if a peripheral init reorder or other library write touched CCIPR between reset and our use, the rm32 path would miss it. |
| H11 | **ADC.CFGR.CONT** | C explicit `=0` (single conversion mode) | rm32 trusts reset default | Same shape as H10. |

## Sequenced fix plan

Roughly in order of expected bug-resolution impact, low effort first:

1. **C3 — PA2 pull-up**: in `mcu_l431/input_capture.rs` `init_l431()`, enable PA2 internal pull-up at GPIO init. One line. Likely fixes the 50% bidir CRC fail rate.
2. **C2 — PA2 OSPEEDR=HIGH**: same file. One line. Required for clean bidir TX edges.
3. **C5 — IWDG prescaler**: change `chip.rs:18 WDG_PRESCALER` from `2` to `4` (encoding for `/16` divider) to match AM32's 2 s timeout. Until then keep the watchdog disabled for bench debug.
4. **H7 — PLL decomposition**: change rm32 to use PLLM=2, PLLN=20 instead of M=1, N=10. Need to replace the stm32l4xx-hal `.cfgr.sysclk(80.MHz()).freeze()` with explicit RCC register writes that match AM32's `LL_RCC_PLL_ConfigDomain_SYS()` call exactly. Pre/post-`freeze()` register-write hook, or fork the HAL.
5. **H8 — TIM6 PSC/ARR**: change `mcu_l431/init.rs` TIM6 init to `PSC=79, ARR=50` matching AM32. Audit any code reading `TIM6.CNT` since the readback scale changes.
6. **H9 — NVIC priorities**: replace per-IRQ `NVIC::unmask()` calls in `mcu_l431/init.rs` with explicit `NVIC::set_priority()` then `NVIC::unmask()` for each, matching AM32's `LL_NVIC_SetPriority()` values exactly (TIM6_DAC=3, TIM1_UP_TIM16=0, COMP=0, DMA1_CH5=1, DMA1_CH4=2, etc. — exact values per agent D's table).
7. **H2 — Motor PWM pin OSPEEDR + OTYPER**: explicit settings in `mcu_l431/init.rs` for PA8/9/10 + PB13/14/15. Mirror AM32's per-pin OSPEEDR/OTYPER bits exactly.
8. **H1 — PA7 init**: add PA7 alternate-function init in `mcu_l431/init.rs`. Required for the third complementary PWM channel.
9. **C4 — DSHOT TX NDTR**: in `capture_generic.rs::send_dshot_dma`, replace fixed `23 + buffer_size/4` with protocol-dependent value. Needs the detected DSHOT rate plumbed in (we already track per-protocol prescaler from `d7231ac`; reuse that).
10. **C1 — TIM15.PSC dynamic**: refactor `mcu_l431/input_capture.rs` so the input prescaler is updated per detected protocol like AM32 does, not hard-coded 13. Set values: PSC=0 for DSHOT600, PSC=1 for DSHOT300, PSC=3 for DSHOT150.
11. **H10 — USART1 CCIPR explicit**: defensive write even though reset default is correct.
12. **H11 — ADC CONT explicit**: defensive write even though reset default is correct.
13. **R1 — COMP2 INMESEL audit**: scope-verify what AM32 actually does vs what the source reads. If AM32 really is comparing against DAC1 by accident, this is an AM32 bug; if it's configured elsewhere, document where. The reverse-divergence is the only place rm32 might be *more* correct than AM32, and we should resolve which is right before flipping rm32's setting to match C.
14. **H4 — Servo HT handler**: add servo-only polarity-switch path. Lower priority since DSHOT300/600 are the current bench protocols.
15. **H5, H6**: optional cleanup once C1-C5 + H1-H2 + H7-H9 are landed and bench-validated.

After each landed step, re-run the register-dump diff and confirm the targeted register(s) now match AM32. Move on to next step only after that.

## Stretch idea: register-parity assertion binary

Worth doing once the immediate fixes have validated the methodology: a separate firmware target that calls every rm32 init function, then *halts in an empty main loop after asserting expected post-init register values*. The expected values come from an AM32-firmware register dump (already captured: `am32_regs.txt`). Any divergence panics with a descriptive message naming the diverging register. Run via probe-rs run, watch the panic / silence, fix one register at a time. Once the binary stays silent in main loop, *every register touched by init matches AM32's reference state*.

This is more thorough than the agent audit because it covers writes that happen at runtime (not just init) too, and it future-proofs against new divergences slipping in. Cost: ~one day to build the assertion harness + value table.

## Stretch idea: register-parity assertion binary

Worth doing later: a separate firmware target that calls every rm32 init function, then *halts in an empty main loop after asserting expected post-init register values*. The expected values are computed from the AM32 register dump (above-style). Any divergence panics with a descriptive message naming the diverging register. Run it via probe-rs run, watch the panic / silence, fix one register at a time. Once the binary stays silent in main loop, *every register touched by init matches AM32's reference state*.

This is more thorough than the agent audit because it covers writes that happen at runtime (after init) too. It also future-proofs against new divergences. But it's a separate work item that requires the audit to complete first (so we know the expected values).

## Severity & priority

The bench failures we've spent multiple sessions chasing — false-sync detection gap, stall-rate bug, ghost-state `with_isr_state` no-op, EDT-armed throttle gate, missing desync_check transfer, **and now the bidir CRC fail rate** — are all instances of the same root pattern: **the Rust port doesn't faithfully reflect the C reference, and we keep discovering specific divergences one at a time, the hard way.**

A systematic register-level audit eliminates the entire class of "register configured wrong" bugs at once. It's worth doing now before more bench debug cycles compound the cost.
