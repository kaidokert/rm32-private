//! 48 kHz-PWM VARIANT of scope_cl2 (detour: high-speed sampling enabler — see NEXT_ROADMAP.md
//! "sampling wall"). IDENTICAL to scope_cl2 except PWM_HZ = 48_000 (so the valley-triggered
//! ADC frame rate is 48 kHz, not 20 kHz) and CAPTURE_MIN_HZ raised to keep the DMA buffer the
//! same size in 32 KB SRAM. Purpose: keep ≥6 ADC frames/sector up toward ~1300 elec Hz (at
//! 20 kHz scan that floor is hit by ~800 Hz). Watch DBG_TIM7 cyc — the TIM7 ISR now fires
//! every 20.8 µs (2.4× more often), so this also stress-tests whether the ISR budget holds.
//! CAVEAT: the valley scan must finish inside the half-ON window (duty*10.42 us at 48 kHz).
//! BEMF channels run at Cycles_12_5 here (2x the 20 kHz tool, for cleaner reads): the 3rd BEMF
//! aperture ends ~1.47 us after TRGO, so BEMF is valid down to ~15 % actual duty (≈22 % amp =
//! AMP_START). The two current channels sit later in the scan and are valid above ~23 % duty.
//! Below ~15 % duty the BEMF reads drift out of the ON window and are NOT trustworthy. Fine
//! for the high-duty/high-speed regime this variant targets.
//!
//! CLOSED-LOOP STAGE 2 / PATH B (BOUNDED STEERING) scope. The validated
//! `rinz::cl::ClLoop` controller (detector → ZC-to-ZC period PI filter → commutate
//! ~30° after the ZC) drives commutation, alpha-blended with the open-loop schedule:
//!   target = ol_period + alpha·clamp(loop_target − ol_period, ±CL_SLEW_FRAC·ol_period)
//! `alpha` (keys 0/1/2/3 = 0.0/0.2/0.5/1.0; 0 = instant fallback; also zeroed by
//! w/q/watchdog) ranges 0..1:
//!   alpha=0  → commutate exactly on the open-loop schedule = the governor / instant
//!             fallback (spin up here, then raise alpha);
//!   alpha=1  → the loop may nudge commutation within ±CL_SLEW_FRAC of the open-loop
//!             period. The frequency stays GOVERNED by ol_period, so it cannot run away.
//! The capture/stream path is intact and the debug line carries `alpha=`/`period_est=`
//! plus the per-commutation `cl` log, so the loop stays observable while it steers
//! (compare period_est vs the offline oracle = the D2/G3 anti-skunk check). The
//! controller is the IDENTICAL code validated on the host motor model (tests/cl_sim.rs).
//!
//! Sampling (unchanged from scope1): ONE scan per PWM period at the valley (CNT=0).
//! In center-aligned PWM mode 1 the ON pulse is centered at the valley, so a
//! TRGO at CNT=0 lands in the exact middle of the ON window for every duty:
//!   HIGH  phase: high-side ON  → divider reads ~Vbus
//!   LOW   phase: low-side ON   → divider reads ~0
//!   FLOAT phase: both FETs OFF → divider reads neutral + BEMF
//! Mid-ON sampling budget at 48 kHz: the period is 20.8 us, so the half-ON window is
//! duty*20.8/2 us. The 5-ch valley scan's last aperture ends ~2.24 us after TRGO at
//! Cycles_6_5 — so the full scan is valid only above ~21% actual duty (vs ~5% at 20 kHz).
//!
//! TIM1 CC4 (CCR4=1, PWM mode 1) pulses OC4REF at the valley; CR2.MMS routes
//! OC4REF to TRGO straight into ADC2 EXTSEL — no TIM3 needed. 12-bit samples,
//! 48 kHz frame rate (1 frame = ch17/PA4, ch5/PC4, ch14/PB11 BEMF voltages +
//! ch16/OPAMP2 phase-B current + ch18/OPAMP3 phase-C current).
//!
//! Each dump's `debug:` line also carries `vbus_mv=` (clean, PA0/ADC1 ×10.39) and
//! `iu_ma=` (phase-U current, OPAMP1 PGA gain-16, peak of 8 samples ≈ bus current;
//! single-phase proxy, the lock-catch shows as a sharp jump).
//!
//! Commands:
//!   f / v   electrical frequency  +10 / -10 Hz
//!   g / b   electrical frequency  +1 / -1 Hz
//!   a / z   amplitude              +1.0 / -1.0 %
//!   + / -   amplitude              +0.1 / -0.1 %
//!   ] / [   duty trim              +1 / -1 raw CCR count
//!   w       kill
//!   d       capture 2 electrical revs from phase zero, then dump 12-bit hex
//!   c       same capture, Ascii85-packed binary (cdump:, ~2x faster than hex)
//!   l / k   start / stop continuous live streaming (binary, like 'c'; other cmds work)
//!   p       toggle the command watchdog (default off): armed -> ISR kills the motor
//!           after 10 s of no serial activity (a hung host can't cook a stalled rotor)
//!   q       reset to defaults and run

#![no_std]
#![no_main]

use core::fmt::Write;

use cortex_m::peripheral::NVIC;
use cortex_m_rt::entry;
use embedded_io::{Read, ReadReady};
use portable_atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering};
use rtt_target::rprintln;

use rinz::cl::ClLoop;
use rinz::hal;
use rinz::hal::adc::{
    Adc, AdcClaim, AdcCommonExt, DMA as AdcDmaStatus, Instance,
    config::{
        ClockMode, Continuous, Dma as AdcDma, ExternalTrigger12, Resolution, SampleTime, Sequence,
        TriggerMode,
    },
};
use rinz::hal::dma::{
    PeripheralToMemory, TransferExt, channel::DMAExt, config::DmaConfig, traits::TargetAddress,
};
use rinz::hal::opamp::Gain;
use rinz::hal::prelude::*;
use rinz::hal::pwm::PwmAdvExt;
use rinz::hal::pwr::{PwrExt, VoltageScale};
use rinz::hal::rcc::{PllConfig, PllMDiv, PllNMul, PllRDiv, PllSrc};
use rinz::hal::serial::FullConfig;
use rinz::hal::time::{ExtU32, Hertz, RateExtU32};
use rinz::hal::{rcc, stm32};
use rinz::idle_loop::IdleLoop;

const PWM_HZ: u32 = 48_000; // 48k variant (scope_cl2 is 20_000). Valley ADC frame rate follows.
const DRIVE_HZ: u32 = PWM_HZ;
const LOGICAL_SECTORS: u32 = 72; // DRIVE_A/B/C table length (12 logical per physical sector)

// amp is in 0.1% units of the drive scale; six-step duty = amp * 2/3.
// 1425 (142.5%) maps to 95% actual PWM duty — the design ceiling.
const AMP_CAP: u32 = 1425;
// 48k variant: start at 22.0 % amp (≈15 % actual duty) -- the floor where the BEMF scan
// (now 12.5-cyc) still finishes inside the half-ON window. Below this the BEMF reads drift
// out of the ON window and are not trustworthy (see header). scope_cl2 (20 kHz) starts at 11 %.
const AMP_START: u32 = 220; // 22.0 % (≈15 % actual duty)
const FREQ_MIN: u32 = 1;
const FREQ_MAX: u32 = 1200;
const FREQ_START: u32 = 60;
// Command-watchdog timeout: 10 s at the TIM7 drive rate (long enough never to
// false-trip between captures, short enough to limit damage on a host hang).
const WATCHDOG_TIMEOUT_TICKS: u32 = 10 * DRIVE_HZ;

// TIM1_TRGO-triggered ADC2 scan of the three BEMF phases:
// ch17/PA4, ch5/PC4, ch14/PB11 — one 3-channel scan per PWM period at the valley.
// ADC clock = synchronous HCLK/4 = 42.5 MHz (proven on this board, always present
// — async-from-SYSCLK doesn't deliver a live kernel clock). 48k variant: BEMF channels
// at the 12.5-cycle sample time = 12.5 + 12.5 = 25 cyc = 588 ns/ch; the 3rd BEMF channel's
// sampling aperture ends 62.5 cyc = 1471 ns after TRGO, inside the half-ON window down to
// ~15% duty (period 20.8 us). 12-bit resolution: bench BEMF swings are a few tens of mV at
// the pin, far below 8-bit LSB.
// 5 channels: 3 BEMF voltages (ch17/ch5/ch14) + 2 phase currents (OPAMP2 ch16 =
// phase-B shunt, OPAMP3 ch18 = phase-C shunt), all in one ADC2 valley scan so
// current and voltage are frame-aligned. Phase-A current is OPAMP1->ADC1 (Phase 2).
const ADC_CHANNELS: usize = 5;
const ADC_SAMPLES_PER_PWM: u32 = 1;
const ADC_FRAME_HZ: u32 = PWM_HZ * ADC_SAMPLES_PER_PWM;
// Buffer must hold this many revolutions at the lowest supported electrical freq.
const CAPTURE_REVS: u32 = 2;
// Raised 60 -> 150 for the 48k variant: ADC_FRAME_COUNT scales with ADC_FRAME_HZ, so at
// 48 kHz a 60 Hz floor would need ~1600 frames (~22 KB ADC DMA) — too much of 32 KB SRAM.
// 150 Hz keeps the frame count (~641) and buffer size the same as the 20 kHz/60 Hz tool.
// Below 150 Hz fewer than 2 revs are captured (fine — this variant targets the high-speed end).
const CAPTURE_MIN_HZ: u32 = 150;
// +1 to round up the partial frame; total buffer fits comfortably in 32 KB SRAM.
const ADC_FRAME_COUNT: usize = (ADC_FRAME_HZ * CAPTURE_REVS / CAPTURE_MIN_HZ + 1) as usize;
const ADC_BUF_LEN: usize = ADC_FRAME_COUNT * ADC_CHANNELS;
// ADC1 ring: phase-A current (OPAMP1 ch13) + VBUS (PA0 ch1), 2 values per frame,
// co-triggered with the ADC2 scan so ADC1 frame i aligns with ADC2 frame i.
const ADC1_CHANNELS: usize = 2;
const ADC1_BUF_LEN: usize = ADC_FRAME_COUNT * ADC1_CHANNELS;

struct AdcDma12<ADC: Instance>(Adc<ADC, AdcDmaStatus>);

impl<ADC: Instance> AdcDma12<ADC> {
    fn start_conversion(&mut self) {
        self.0.start_conversion();
    }
}

unsafe impl<ADC: Instance> TargetAddress<PeripheralToMemory> for AdcDma12<ADC> {
    #[inline(always)]
    fn address(&self) -> u32 {
        <Adc<ADC, AdcDmaStatus> as TargetAddress<PeripheralToMemory>>::address(&self.0)
    }

    type MemSize = u16;

    const REQUEST_LINE: Option<u8> =
        <Adc<ADC, AdcDmaStatus> as TargetAddress<PeripheralToMemory>>::REQUEST_LINE;
}

static RUNNING: AtomicBool = AtomicBool::new(false);
static AMPLITUDE: AtomicU32 = AtomicU32::new(AMP_START);
static ELECTRICAL_HZ: AtomicU32 = AtomicU32::new(FREQ_START);
// Signed fine offset added to the computed six-step duty, in raw CCR counts.
// '[' / ']' nudge it by 1 (finer than '+' /'-', which step ~2.8 counts); reset by 'q'.
static DUTY_TRIM: AtomicI32 = AtomicI32::new(0);
// Live-stream toggle: 'l' sets it, 'k'/'w' clear it. The main idle loop services
// one back-to-back dump per pass while set; all other commands stay unchanged.
static STREAMING: AtomicBool = AtomicBool::new(false);
// Opt-in command watchdog ('p' toggles; default OFF). While armed, the TIM7 ISR
// kills the motor if there's been no serial activity (pet) for WATCHDOG_TIMEOUT_TICKS
// -- protects against a hung host leaving the rotor energized (which cooks a stalled
// motor). Petted on every received byte and at each run_capture (so streaming stays
// alive). The sweep arms it at start and disarms in a finally, so a clean exit or
// Ctrl-C disarms but a HANG (finally never runs) leaves it armed -> fires -> safe.
static WATCHDOG_ARMED: AtomicBool = AtomicBool::new(false);
static WATCHDOG_FIRED: AtomicBool = AtomicBool::new(false);
static WATCHDOG_TICKS: AtomicU32 = AtomicU32::new(0);
static IDLE_DUTY: AtomicU32 = AtomicU32::new(0); // CCR for the killed/idle state (half ARR)
static CAPTURE_REQUEST: AtomicBool = AtomicBool::new(false);
static CAPTURE_DONE: AtomicBool = AtomicBool::new(false);
static CAPTURE_FRAMES: AtomicU32 = AtomicU32::new(0);
static CAPTURE_TICKS_TARGET: AtomicU32 = AtomicU32::new(0);
static CAPTURE_BUF_ADDR: AtomicU32 = AtomicU32::new(0);
// The other buffer: where the ISR re-points the DMA the instant the capture window
// closes, so ADC->DMA keeps streaming while main drains the frozen buffer over UART.
static CAPTURE_BUF_ALT: AtomicU32 = AtomicU32::new(0);
// ADC1 (phase-A current + VBUS) capture ring -- same double-buffer flip as the
// ADC2 ring, re-pointed in lockstep by the TIM7 ISR at window open/close.
static CAPTURE_BUF1_ADDR: AtomicU32 = AtomicU32::new(0);
static CAPTURE_BUF1_ALT: AtomicU32 = AtomicU32::new(0);
// false = buffer 0, true = buffer 1. Flipped on every 'd' so a new capture lands
// in the back buffer while the previous one stays intact in the front buffer.
static BUF_SEL: AtomicBool = AtomicBool::new(false);
static DBG_TIM7_TICKS: AtomicU32 = AtomicU32::new(0);
static DBG_DMA_TC: AtomicU32 = AtomicU32::new(0);
static DBG_DMA_HT: AtomicU32 = AtomicU32::new(0);
static DBG_DMA_TE: AtomicU32 = AtomicU32::new(0);
static DBG_CAPTURE_AMP: AtomicU32 = AtomicU32::new(0);
static DBG_CAPTURE_HZ: AtomicU32 = AtomicU32::new(0);
static DBG_SIX_STEP_TICKS: AtomicU32 = AtomicU32::new(0);
// Worst-case TIM7 ISR duration in CPU cycles (DWT.CYCCNT), to settle "are we hitting an
// MCU bottleneck": budget is 170MHz/DRIVE_HZ = 170M/48k = 3541 cyc/tick. Reset each capture.
static DBG_ISR_CYC: AtomicU32 = AtomicU32::new(0);

// ---- Path B: bounded closed-loop steering via the validated cl::ClLoop ----
// alpha in 0.000..1.000 (stored x1000). alpha=0 == pure open-loop (the governor /
// instant fallback); alpha=1 == the loop may nudge commutation +/- CL_SLEW_FRAC of the
// open-loop period. The frequency stays governed by the commanded period, so it cannot
// run away. 'u'/'i' step alpha; 'o' (and 'w'/'q') slam it to 0.
static CL_ALPHA_X1000: AtomicU32 = AtomicU32::new(0); // default OFF (open-loop)
static CL_PERIOD_X100: AtomicU32 = AtomicU32::new(0); // loop period_est x100, for telemetry
// Per-sector ZC smoothing weight x1000 (live-tunable via ','/'.'); sim says ~0.6 cuts
// commutation jitter ~40% without lagging. Applied to the loop every frame.
static CL_ZC_BETA_X1000: AtomicU32 = AtomicU32::new(600);
// Commutation timing ADVANCE, sector-fraction x1000 (live-tunable via '<'/'>'). 0 = 30 deg
// after ZC (baseline); larger commutates earlier to offset winding-L current lag at speed.
// 0.45 sector = 27 deg advance max. Testing whether this lifts the ~700 Hz six-step ceiling.
static CL_ADVANCE_X1000: AtomicU32 = AtomicU32::new(0);
// Speed-proportional advance schedule (1=on, toggle '/'): effective advance ramps 0 -> the
// CL_ADVANCE_X1000 cap linearly from ADV_SCHED_HZ0 over ADV_SCHED_RAMP_HZ Hz. Keeps the low end
// un-advanced (clean catch) and applies full advance only near the six-step ceiling.
static CL_ADV_SCHED: AtomicU32 = AtomicU32::new(0);
const ADV_SCHED_HZ0: u32 = 500; // advance stays 0 below this commanded Hz
const ADV_SCHED_RAMP_HZ: u32 = 400; // Hz above HZ0 over which advance ramps 0 -> cap
// Predictive coast (1=on): on a missed ZC, schedule from per-sector memory vs open-loop
// timeout. Targets the residual ~4% per-miss commutation clips. Live A/B via 'y'.
static CL_PREDICT_COAST: AtomicU32 = AtomicU32::new(1);
// Loop detector source: 0 = linfit (extrapolates a crossing even out-of-window -> Phase-3
// jitter), 1 = sign-change (clean in-window crossing only, else coast). Toggle with 'j'.
static CL_USE_SIGNCHANGE: AtomicU32 = AtomicU32::new(0);
// Streaming harmonic mode: 0 = off, 1 = push only (per-tick accumulate), 2 = full (push +
// per-commutation crossing). Splits the harmonic's ISR cost into per-tick vs per-commutation.
// 0 falls back to the cheap per-sector detector. ';' cycles 2 -> 1 -> 0 -> 2.
static CL_HARM_EN: AtomicU32 = AtomicU32::new(2);
// Stage-2 experiment: drive commutation from the harmonic crossing (1) vs the per-sector
// sign-change (0). Authority stays bounded by on_frame_blend (slew clamp + alpha). Toggle "'".
static CL_HARM_DRIVE: AtomicU32 = AtomicU32::new(0);
// Predictive-coast engage gate x100 (lock_fast threshold). Default 50 = 0.50. Sign-change
// ceilings lock_fast at ~0.33, so 0.50 never engages predict -> lower with 'e', raise 'r'.
static CL_PREDICT_GATE_X100: AtomicU32 = AtomicU32::new(50);
// Full-sensorless drive: 0 = governed (on_frame_blend), 1 = drive when locked (on_frame, the
// PLL sets the rate). Toggle with 'o'. CL_DRIVING_FLAG mirrors the ISR's live mode for telemetry.
static CL_DRIVE: AtomicU32 = AtomicU32::new(0);
static CL_DRIVING_FLAG: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);
// Lock-quality IIRs (x1000) read from the loop into telemetry: hit fraction + ZC jitter.
static CL_LOCK_FAST_X1000: AtomicU32 = AtomicU32::new(0);
static CL_LOCK_SLOW_X1000: AtomicU32 = AtomicU32::new(0);
static CL_JIT_FAST_X1000: AtomicU32 = AtomicU32::new(0); // ticks x1000
static CL_JIT_SLOW_X1000: AtomicU32 = AtomicU32::new(0);
// Long-running glitch monitor: cumulative counters mirrored from the loop each frame,
// emitted as a periodic `glitch:` line while MONITOR is set ('i' toggles, 'x' resets).
// Catches the rare seconds-apart events a single 2-rev capture can't.
static CL_GLITCH_COMM: AtomicU32 = AtomicU32::new(0);
static CL_GLITCH_COAST: AtomicU32 = AtomicU32::new(0);
static CL_GLITCH_BURSTS: AtomicU32 = AtomicU32::new(0);
static CL_GLITCH_BIGRES: AtomicU32 = AtomicU32::new(0);
static CL_GLITCH_MAXRUN: AtomicU32 = AtomicU32::new(0);
static CL_GLITCH_SINCE: AtomicU32 = AtomicU32::new(0);
// Histograms: coast-run lengths (1..8+) and |ZC residual| buckets, emitted as a `hist:`
// line so the rare-event distribution shape shows (deep bursts vs harmless singles).
static CL_RUN_HIST: [AtomicU32; 8] = [const { AtomicU32::new(0) }; 8];
static CL_RESID_HIST: [AtomicU32; 6] = [const { AtomicU32::new(0) }; 6];
static CL_GLITCH_RESET: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);
static MONITOR: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);
// Stall-kill: when enabled (default), the ISR kills the motor the moment the cl loop
// reports a stall (lost a lock it held). 'h' toggles; STALL_FIRED is the one-shot event
// (main loop reports it); CL_STALL_RESET re-arms the detector on a re-spin ('q').
static STALL_KILL_EN: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(true);
static STALL_FIRED: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);
static CL_STALL_RESET: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);
const CL_STALL_RUN: u32 = 10; // consecutive coasts that count as a stall (~1.6 elec revs)
// Phase-2 fault telemetry: an always-on watcher emits a classified `FAULT:` line on each
// fault EDGE over UART (no 'i' monitor needed), so control failures are caught the moment
// they happen. A coast run reaching this length (but below CL_STALL_RUN) is a momentary
// lock wobble worth flagging; above the normal per-sector-wave coast (~3) so it doesn't flood.
const CL_FAULT_RUN_MIN: u32 = 5;
// CPU-starvation guard: emit FAULT: CPU_HIGH when the worst-case TIM7 ISR exceeds this % of
// its per-tick budget (170MHz/DRIVE_HZ) -- so ISR overrun surfaces as a labelled fault in any
// mode, never mistaken for a control desync. Hysteresis re-arm at -10%; DBG_ISR_CYC (fetch_max)
// is cleared by each capture, so the guard re-arms naturally between captures.
const CL_CPU_HIGH_PCT: u32 = 88;
// Full-sensorless DRIVE (Step 3): hand from the governor to on_frame (PLL drives the rate) only
// once solidly locked, and revert below the drop threshold (hysteresis on lock_slow). The period
// estimate is hard-clamped to a safe speed band (ticks/sector) so a detector glitch can't run the
// rate away -- on top of on_frame's already-gated PLL. PMIN = max speed, PMAX = min speed.
// Measured: sign-change lock_slow caps ~0.29 at 380 Hz (settled), so 0.40 was unreachable.
// Set the handoff to the achievable lock and test whether drive HOLDS off sparse (~30%) detection
// -- the gated PLL + clamp_period are the protection. Revisit if drive can't hold the rate.
const CL_DRIVE_HANDOFF_LOCK: f32 = 0.25;
const CL_DRIVE_DROP_LOCK: f32 = 0.12;
const CL_DRIVE_PMIN: f32 = DRIVE_HZ as f32 / (1400.0 * 6.0); // ~1400 Hz elec ceiling
const CL_DRIVE_PMAX: f32 = DRIVE_HZ as f32 / (80.0 * 6.0); //   ~80 Hz elec floor
const CL_SLEW_FRAC: f32 = 0.15; // max nudge as a fraction of the open-loop period at alpha=1
const CL_KP: f32 = 0.3; // period PI gains (validated in the host sim)
const CL_KI: f32 = 0.2;
const CL_GATE_FRAC: f32 = 0.5; // reject period measurements deviating > 50%
const CL_LOOP_COAST: f32 = 1.0; // dead-reckon at period_est when a ZC is missed

// ---- OBSERVE-ONLY closed-loop ZC detector (Stage 1; steers nothing) ----
// Physical six-step sector = STEP / STEPS_PER_PHYS_SECTOR (24), 6 per electrical rev.
// SIX_HIGH/SIX_LOW give the driven channels per physical sector (channel idx == phase
// idx: 0=A/ch17, 1=B/ch5, 2=C/ch14); the third is the floating phase. These match
// scope_common.SIX_STEP_HIGH/LOW (verified against the DRIVE_A/B/C tables).
const CL_BLANK: u32 = 4; // demag skip frames before the line fit (demag tail corrupts it)
const CL_MAXCOMM: usize = 24; // per-capture commutation-log capacity (2 revs = 12)
// Per-commutation log filled DURING a capture window, dumped before the frame dump.
// Packed u32: byte2=phys(0..5), byte1=zc_pct(0..100, 255=no in-window ZC), byte0=
// sector period in ticks (omega proxy, saturated at 255). Read by main for the dump.
static CL_CAP: [AtomicU32; CL_MAXCOMM] = [const { AtomicU32::new(0) }; CL_MAXCOMM];
// Parallel to CL_CAP: the RAW signed linfit ZC% (the value the loop actually commutates
// on), un-clamped so out-of-window extrapolations (-30..130, gate-bounded) survive. zcb
// above is the clamped in-window subset; lf is the full linfit point. 9999 = none.
static CL_CAP_LF: [AtomicI32; CL_MAXCOMM] = [const { AtomicI32::new(9999) }; CL_MAXCOMM];
// Capture-buffer frame index at each commutation = the ACTUAL (nudged) sector boundary.
// Lets the host place sectors where the loop really commutated instead of on a uniform
// grid -- so in closed loop the firmware and python linfit windows coincide.
static CL_CAP_BND: [AtomicU32; CL_MAXCOMM] = [const { AtomicU32::new(0) }; CL_MAXCOMM];
// Per-commutation streaming-harmonic crossing (% of window, signed; 9999 = no fit/crossing).
// OBSERVE-ONLY -- the harmonic does not steer yet; this is for comparing its coverage to the
// sign-change (zcb) and the offline oracle on real hardware before letting it drive.
static CL_CAP_HARM: [AtomicI32; CL_MAXCOMM] = [const { AtomicI32::new(9999) }; CL_MAXCOMM];
static CL_CAP_N: AtomicU32 = AtomicU32::new(0);

/// Per-phase output-stage mode per logical sector (one array each for A/B/C).
/// Each physical six-step state is repeated 12 times across the 72 sectors.
/// F = Forward (high), R = Reverse (low), O = Floating (BEMF sense).
const F: Drive = Drive::Forward;
const R: Drive = Drive::Reverse;
const O: Drive = Drive::Floating;
const DRIVE_A: [Drive; LOGICAL_SECTORS as usize] = [
    /*0*/ F, F, F, F, F, F, F, F, F, F, F, F, /*1*/ F, F, F, F, F, F, F, F, F, F, F, F,
    /*2*/ O, O, O, O, O, O, O, O, O, O, O, O, /*3*/ R, R, R, R, R, R, R, R, R, R, R, R,
    /*4*/ R, R, R, R, R, R, R, R, R, R, R, R, /*5*/ O, O, O, O, O, O, O, O, O, O, O, O,
];
const DRIVE_B: [Drive; LOGICAL_SECTORS as usize] = [
    /*0*/ R, R, R, R, R, R, R, R, R, R, R, R, /*1*/ O, O, O, O, O, O, O, O, O, O, O, O,
    /*2*/ F, F, F, F, F, F, F, F, F, F, F, F, /*3*/ F, F, F, F, F, F, F, F, F, F, F, F,
    /*4*/ O, O, O, O, O, O, O, O, O, O, O, O, /*5*/ R, R, R, R, R, R, R, R, R, R, R, R,
];
const DRIVE_C: [Drive; LOGICAL_SECTORS as usize] = [
    /*0*/ O, O, O, O, O, O, O, O, O, O, O, O, /*1*/ R, R, R, R, R, R, R, R, R, R, R, R,
    /*2*/ R, R, R, R, R, R, R, R, R, R, R, R, /*3*/ O, O, O, O, O, O, O, O, O, O, O, O,
    /*4*/ F, F, F, F, F, F, F, F, F, F, F, F, /*5*/ F, F, F, F, F, F, F, F, F, F, F, F,
];

/// Output-stage mode for one phase half-bridge.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Drive {
    /// High-side PWM active — terminal driven toward Vbus at `duty`.
    Forward,
    /// Low-side held on — terminal pulled to GND (CCR=0).
    Reverse,
    /// Both FETs off — terminal floats so BEMF can be sensed.
    Floating,
}

/// Set MODER for all six motor pins. AF=0b10, OUTPUT=0b01.
/// Also resets ODR to 0 for any floated pins via BSRR.
unsafe fn set_phase_modes(float_a: bool, float_b: bool, float_c: bool) {
    const AF: u32 = 0b10;
    const OUT: u32 = 0b01;

    let ga = unsafe { &*stm32::GPIOA::ptr() };
    let gb = unsafe { &*stm32::GPIOB::ptr() };
    let gc = unsafe { &*stm32::GPIOC::ptr() };

    let (ma8, ma9, ma10, ma12) = (
        if float_a { OUT } else { AF },
        if float_b { OUT } else { AF },
        if float_c { OUT } else { AF },
        if float_b { OUT } else { AF },
    );
    ga.moder().modify(|r, w| unsafe {
        w.bits(
            r.bits() & !(3 << 16 | 3 << 18 | 3 << 20 | 3 << 24)
                | ma8 << 16
                | ma9 << 18
                | ma10 << 20
                | ma12 << 24,
        )
    });

    let mb15 = if float_c { OUT } else { AF };
    gb.moder()
        .modify(|r, w| unsafe { w.bits((r.bits() & !(3 << 30)) | mb15 << 30) });

    let mc13 = if float_a { OUT } else { AF };
    gc.moder()
        .modify(|r, w| unsafe { w.bits((r.bits() & !(3 << 26)) | mc13 << 26) });

    let mut ba = 0u32;
    let mut bb = 0u32;
    let mut bc = 0u32;
    if float_a {
        ba |= 1 << (16 + 8);
        bc |= 1 << (16 + 13);
    }
    if float_b {
        ba |= 1 << (16 + 9) | 1 << (16 + 12);
    }
    if float_c {
        ba |= 1 << (16 + 10);
        bb |= 1 << (16 + 15);
    }
    if ba != 0 {
        ga.bsrr().write(|w| unsafe { w.bits(ba) });
    }
    if bb != 0 {
        gb.bsrr().write(|w| unsafe { w.bits(bb) });
    }
    if bc != 0 {
        gc.bsrr().write(|w| unsafe { w.bits(bc) });
    }
}

fn restore_all_af() {
    unsafe { set_phase_modes(false, false, false) }
}

/// De-energize the motor: all phases driven to the idle half duty -> no inter-phase
/// voltage -> no current. The single kill path, shared by the 'w' command and the
/// watchdog ISR -- raw registers only (no HAL handles), so it's callable from the ISR.
fn kill_motor() {
    restore_all_af();
    let t1 = unsafe { &*stm32::TIM1::ptr() };
    let half = IDLE_DUTY.load(Ordering::Relaxed);
    t1.ccr1().write(|w| unsafe { w.ccr().bits(half) });
    t1.ccr2().write(|w| unsafe { w.ccr().bits(half) });
    t1.ccr3().write(|w| unsafe { w.ccr().bits(half) });
    RUNNING.store(false, Ordering::Relaxed);
    STREAMING.store(false, Ordering::Relaxed);
    CL_ALPHA_X1000.store(0, Ordering::Relaxed); // revert to open-loop on any kill
}

/// Route TIM1_TRGO directly to ADC2 — one trigger per PWM period at the valley.
///
/// CCR4=1 with PWM mode 1 in center-aligned mode: OC4REF is HIGH only while
/// CNT < 1. The LOW→HIGH transition occurs when the downcount reaches CNT=0,
/// i.e. at the valley — the exact center of every phase's ON window. The ADC
/// scan therefore always starts mid-ON, with half the ON window of margin.
fn configure_adc_valley_trgo() {
    let t1 = unsafe { &*stm32::TIM1::ptr() };
    t1.ccr4().write(|w| unsafe { w.ccr().bits(1) });
    // CCMR2: OC4 PWM mode 1. CR2.MMS=0b111: OC4REF → TRGO.
    t1.ccmr2_output()
        .modify(|r, w| unsafe { w.bits((r.bits() & !(0x7000 | 0x300)) | (0b110u32 << 12)) });
    t1.cr2()
        .modify(|r, w| unsafe { w.bits((r.bits() & !0x70) | (0b111u32 << 4)) });
}

fn two_rev_drive_tick_count(electrical_hz: u32) -> u32 {
    let hz = electrical_hz.max(1);
    (DRIVE_HZ * CAPTURE_REVS).div_ceil(hz).max(1)
}

fn two_rev_adc_frame_count(electrical_hz: u32) -> u32 {
    let hz = electrical_hz.max(1);
    let frames = (ADC_FRAME_HZ * CAPTURE_REVS).div_ceil(hz);
    frames.min(ADC_FRAME_COUNT as u32).max(1)
}

unsafe fn restart_capture_dma(buf_addr: u32) {
    if buf_addr == 0 {
        return;
    }

    let dma = unsafe { &*stm32::DMA1::ptr() };
    let ch = dma.ch1();
    let cr = ch.cr().read().bits();

    ch.cr().write(|w| unsafe { w.bits(cr & !1) });
    dma.ifcr().write(|w| unsafe { w.bits(0x0f) });
    ch.mar().write(|w| unsafe { w.bits(buf_addr) });
    ch.ndtr().write(|w| unsafe { w.bits(ADC_BUF_LEN as u32) });
    ch.cr().write(|w| unsafe { w.bits(cr | 1) });
}

/// Re-point the ADC1 capture ring (DMA1 ch2) the same way restart_capture_dma does
/// for the ADC2 ring (ch1). Called in lockstep so the two rings stay frame-aligned.
unsafe fn restart_capture_dma1(buf_addr: u32) {
    if buf_addr == 0 {
        return;
    }

    let adc1 = unsafe { &*stm32::ADC1::ptr() };
    let dma = unsafe { &*stm32::DMA1::ptr() };
    let ch = dma.ch2();

    // Stop ADC1, re-point the DMA, then re-arm -- so the next TRGO starts a FRESH
    // scan from rank 1 (ch13) into slot 0. Without this, re-pointing the DMA while a
    // scan is in flight offsets the [I_A, VBUS] pair by one (the channels swap) on
    // ~2% of captures, timing-dependent and NOT eliminated by a short sample time
    // alone. ADSTP makes slot 0 = I_A deterministically. (ADC2's all-fast scan
    // happens to align without this; ADC1's longer scan does not.)
    adc1.cr().modify(|_, w| w.adstp().set_bit());
    while adc1.cr().read().adstart().bit_is_set() {}

    let cr = ch.cr().read().bits();
    ch.cr().write(|w| unsafe { w.bits(cr & !1) });
    dma.ifcr().write(|w| unsafe { w.bits(0xf0) }); // clear ch2 flags (bits 4-7)
    ch.mar().write(|w| unsafe { w.bits(buf_addr) });
    ch.ndtr().write(|w| unsafe { w.bits(ADC1_BUF_LEN as u32) });
    ch.cr().write(|w| unsafe { w.bits(cr | 1) });

    adc1.cr().modify(|_, w| w.adstart().set_bit()); // re-arm for the next TRGO
}

/// Read the most-recently-completed ADC2 frame straight from the live DMA buffer
/// (DMA1 ch1 MAR + NDTR), for the observe-only real-time detector. Steers nothing.
fn read_latest_adc2_frame() -> [u16; ADC_CHANNELS] {
    let dma = unsafe { &*stm32::DMA1::ptr() };
    let ch = dma.ch1();
    let base = ch.mar().read().bits() as *const u16;
    let ndtr = ch.ndtr().read().ndt().bits() as usize;
    let done = ADC_BUF_LEN.saturating_sub(ndtr) / ADC_CHANNELS;
    let idx = done.saturating_sub(1); // latest COMPLETE frame
    let mut f = [0u16; ADC_CHANNELS];
    for (c, fc) in f.iter_mut().enumerate() {
        *fc = unsafe { core::ptr::read_volatile(base.add(idx * ADC_CHANNELS + c)) };
    }
    f
}

/// Logical-sector commutation: two phases driven, one floating.
unsafe fn set_six_step(sector: u8, duty: u32) {
    let s = (sector as usize) % LOGICAL_SECTORS as usize;
    let (da, db, dc) = (DRIVE_A[s], DRIVE_B[s], DRIVE_C[s]);
    let t1 = unsafe { &*stm32::TIM1::ptr() };

    // Forward = high-side PWM at `duty`; Reverse/Floating leave CCR at 0.
    let ccr = |d: Drive| if matches!(d, Drive::Forward) { duty } else { 0 };
    t1.ccr1().write(|w| unsafe { w.ccr().bits(ccr(da)) });
    t1.ccr2().write(|w| unsafe { w.ccr().bits(ccr(db)) });
    t1.ccr3().write(|w| unsafe { w.ccr().bits(ccr(dc)) });

    let floating = |d: Drive| matches!(d, Drive::Floating);
    unsafe { set_phase_modes(floating(da), floating(db), floating(dc)) };
}

struct BoardInit {
    clocks: hal::rcc::Clocks,
    rcc: hal::rcc::Rcc,
}

fn board_init(dp_rcc: stm32::RCC, dp_pwr: stm32::PWR) -> BoardInit {
    let pwr = dp_pwr
        .constrain()
        .vos(VoltageScale::Range1 { enable_boost: true })
        .freeze();
    let pll_cfg = PllConfig {
        mux: PllSrc::HSE(Hertz::MHz(8)),
        m: PllMDiv::DIV_2,
        n: PllNMul::MUL_85,
        r: Some(PllRDiv::DIV_2),
        p: None,
        q: None,
    };
    let rcc = dp_rcc.freeze(rcc::Config::pll().pll_cfg(pll_cfg).boost(true), pwr);
    BoardInit {
        clocks: rcc.clocks,
        rcc,
    }
}

/// Handle one serial command byte (drive is set from atomics in the ISR; the kill
/// goes through the shared kill_motor()).
fn handle_command<TX>(cmd: u8, tx: &mut TX)
where
    TX: Write,
{
    match cmd {
        b'f' => {
            let hz = (ELECTRICAL_HZ.load(Ordering::Relaxed) + 10).min(FREQ_MAX);
            ELECTRICAL_HZ.store(hz, Ordering::Relaxed);
            writeln!(tx, "freq={}Hz\r", hz).ok();
        }
        b'v' => {
            let hz = ELECTRICAL_HZ
                .load(Ordering::Relaxed)
                .saturating_sub(10)
                .max(FREQ_MIN);
            ELECTRICAL_HZ.store(hz, Ordering::Relaxed);
            writeln!(tx, "freq={}Hz\r", hz).ok();
        }
        b'a' => {
            let amp = (AMPLITUDE.load(Ordering::Relaxed) + 10).min(AMP_CAP);
            AMPLITUDE.store(amp, Ordering::Relaxed);
            writeln!(tx, "amp={}.{}%\r", amp / 10, amp % 10).ok();
        }
        b'z' => {
            let amp = AMPLITUDE.load(Ordering::Relaxed).saturating_sub(10);
            AMPLITUDE.store(amp, Ordering::Relaxed);
            writeln!(tx, "amp={}.{}%\r", amp / 10, amp % 10).ok();
        }
        b'+' => {
            let amp = (AMPLITUDE.load(Ordering::Relaxed) + 1).min(AMP_CAP);
            AMPLITUDE.store(amp, Ordering::Relaxed);
            writeln!(tx, "amp={}.{}%\r", amp / 10, amp % 10).ok();
        }
        b'-' => {
            let amp = AMPLITUDE.load(Ordering::Relaxed).saturating_sub(1);
            AMPLITUDE.store(amp, Ordering::Relaxed);
            writeln!(tx, "amp={}.{}%\r", amp / 10, amp % 10).ok();
        }
        b'g' => {
            let hz = (ELECTRICAL_HZ.load(Ordering::Relaxed) + 1).min(FREQ_MAX);
            ELECTRICAL_HZ.store(hz, Ordering::Relaxed);
            writeln!(tx, "freq={}Hz\r", hz).ok();
        }
        b'b' => {
            let hz = ELECTRICAL_HZ
                .load(Ordering::Relaxed)
                .saturating_sub(1)
                .max(FREQ_MIN);
            ELECTRICAL_HZ.store(hz, Ordering::Relaxed);
            writeln!(tx, "freq={}Hz\r", hz).ok();
        }
        b']' => {
            let trim = (DUTY_TRIM.load(Ordering::Relaxed) + 1).clamp(-5000, 5000);
            DUTY_TRIM.store(trim, Ordering::Relaxed);
            writeln!(tx, "trim={}\r", trim).ok();
        }
        b'[' => {
            let trim = (DUTY_TRIM.load(Ordering::Relaxed) - 1).clamp(-5000, 5000);
            DUTY_TRIM.store(trim, Ordering::Relaxed);
            writeln!(tx, "trim={}\r", trim).ok();
        }
        b'w' => {
            kill_motor();
            writeln!(tx, "kill\r").ok();
        }
        b'q' => {
            restore_all_af();
            ELECTRICAL_HZ.store(FREQ_START, Ordering::Relaxed);
            AMPLITUDE.store(AMP_START, Ordering::Relaxed);
            DUTY_TRIM.store(0, Ordering::Relaxed);
            CL_ALPHA_X1000.store(0, Ordering::Relaxed); // reset to open-loop
            STALL_FIRED.store(false, Ordering::Relaxed); // re-arm the stall detector
            CL_STALL_RESET.store(true, Ordering::Relaxed);
            RUNNING.store(true, Ordering::Relaxed);
            writeln!(
                tx,
                "reset: freq={}Hz amp={}.{}%\r",
                FREQ_START,
                AMP_START / 10,
                AMP_START % 10
            )
            .ok();
        }
        _ => {}
    }
}

#[entry]
fn main() -> ! {
    rinz::panic::ensure_rtt();

    let mut cp = cortex_m::Peripherals::take().unwrap();
    cp.DCB.enable_trace(); // DEMCR.TRCENA -- gate DWT
    cp.DWT.enable_cycle_counter(); // free-running CYCCNT for ISR-duration measurement
    let dp = stm32::Peripherals::take().unwrap();
    let BoardInit { clocks, mut rcc } = board_init(dp.RCC, dp.PWR);

    rprintln!(
        "scope: sys_clk={} apb1={}",
        clocks.sys_clk.raw(),
        clocks.apb1_clk.raw()
    );

    let gpioa = dp.GPIOA.split(&mut rcc);
    let gpiob = dp.GPIOB.split(&mut rcc);
    let gpioc = dp.GPIOC.split(&mut rcc);

    let usart = dp
        .USART2
        .usart(
            gpiob.pb3.into_alternate(),
            gpiob.pb4.into_alternate(),
            FullConfig::default().baudrate(115_200u32.bps()),
            &mut rcc,
        )
        .unwrap();
    let (mut tx, mut rx) = usart.split();

    writeln!(
        tx,
        "scope_cl2_48k ready (48 kHz PWM; Path B CLOSED-LOOP; alpha=0=open-loop)  f/v=hz g/b=hz a/z=amp +/-=amp 0/1/2/3=alpha 0/.2/.5/1.0 n/m=alpha+/-.05 ,/.=zc_beta+/-.05 <,>=advance-/+ /=adv_sched y=predict_coast e/r=predict_gate-/+ j=detector(lf/sc) ;=harm(full/push/off) '=harm_drive o=drive(sensorless) i=monitor x=glitch_reset h=stall_kill (0=fallback) w=kill d/c=dump l/k=stream p=wd q=reset\r"
    )
    .ok();

    let (_ctrl, (c1, c2, c3)) = dp
        .TIM1
        .pwm_advanced(
            (
                gpioa.pa8.into_alternate::<6>(),
                gpioa.pa9.into_alternate::<6>(),
                gpioa.pa10.into_alternate::<6>(),
            ),
            &mut rcc,
        )
        .frequency(PWM_HZ.Hz())
        .with_deadtime(100u32.nanos())
        .center_aligned()
        .finalize();

    let mut c1 = c1.into_complementary(gpioc.pc13.into_alternate::<4>());
    let mut c2 = c2.into_complementary(gpioa.pa12.into_alternate::<6>());
    let mut c3 = c3.into_complementary(gpiob.pb15.into_alternate::<4>());

    let half = c1.max_duty_cycle() as u32 / 2;
    IDLE_DUTY.store(half, Ordering::Relaxed); // watchdog_kill drives all phases here
    c1.enable();
    c2.enable();
    c3.enable();
    let _ = c1.set_duty_cycle(half as u16);
    let _ = c2.set_duty_cycle(half as u16);
    let _ = c3.set_duty_cycle(half as u16);
    configure_adc_valley_trgo();

    // PB5 low enables the BEMF attenuation network. Without it, the ADC input is
    // effectively only clamp-limited through the phase-side series resistor.
    let mut _gpio_bemf = gpiob.pb5.into_push_pull_output();
    _gpio_bemf.set_low();

    // ADC2 ch17/ch5/ch14 (PA4/PC4/PB11) — TIM1_TRGO-triggered circular DMA scan.
    writeln!(tx, "dbg: pwm ok, adc setup\r").ok();
    let pa4 = gpioa.pa4.into_analog();
    let pc4 = gpioc.pc4.into_analog();
    let pb11 = gpiob.pb11.into_analog();
    let dma_channels = dp.DMA1.split(&rcc);
    let dma_config = DmaConfig::default()
        .transfer_complete_interrupt(true)
        .half_transfer_interrupt(true)
        .transfer_error_interrupt(true)
        .circular_buffer(true)
        .memory_increment(true);

    let mut delay = cp.SYST.delay(&clocks);
    // Synchronous HCLK/4 = 42.5 MHz: derived directly from AHB so the kernel clock is
    // always live (calibration won't hang), and within the ADC's 60 MHz max. The
    // async-from-SYSCLK path does not deliver a running kernel clock on this board.
    let adc_clock = ClockMode::AdcHclkDiv4;
    writeln!(tx, "dbg: claim common\r").ok();
    let adc12_common = dp.ADC12_COMMON.claim(adc_clock, &mut rcc);

    // ADC1: VBUS (PA0/IN1) and phase-U current via OPAMP1 PGA gain-16 (PA1). The
    // board has no bus shunt -- iu is a single-phase-U proxy (read as peak-of-8
    // below, so it catches the driven-sector current ~= bus current). VBUS is the
    // clean one. Same wiring/scaling as examples/motor_tester.rs.
    writeln!(tx, "dbg: opamp1 + claim adc1\r").ok();
    let pa0_vbus = gpioa.pa0.into_analog();
    let pa1_isns = gpioa.pa1.into_analog();
    let pa7_isns = gpioa.pa7.into_analog();
    let pb0_isns = gpiob.pb0.into_analog();
    let (opamp1, opamp2, opamp3, ..) = dp.OPAMP.split(&mut rcc);
    let opamp1_pga = opamp1.pga(pa1_isns, Gain::Gain16);
    // Phase-B/C shunt currents via OPAMP2 (PA7 -> ADC2 ch16) and OPAMP3 (PB0 ->
    // ADC2 ch18), same internal PGA gain-16 as OPAMP1 (board OP_OUT pins are N/C,
    // so internal PGA -- no external feedback network). Their InternalOutput is
    // added to the ADC2 valley scan below, so they land in the capture buffer
    // frame-aligned with the three BEMF voltages.
    let opamp2_pga = opamp2.pga(pa7_isns, Gain::Gain16);
    let opamp3_pga = opamp3.pga(pb0_isns, Gain::Gain16);
    // ADC1: phase-A current (OPAMP1 ch13) + VBUS (PA0 ch1), both at 47.5cyc.
    // VBUS MUST stay short: a long sample (640.5cyc ~= 15us) stretches the 2-channel
    // ADC1 scan to ~1/3 of the 50us PWM period, so the ISR's DMA re-point can land
    // mid-scan and offset the [I_A, VBUS] pair by one -> the two channels swap
    // intermittently. 47.5cyc keeps the scan ~1.5us, so the restart lands in the idle
    // gap (like ADC2's all-fast scan, which never offsets). VBUS settles fine at
    // 47.5cyc thanks to the divider's filter cap (C70 ~0.1uF). TIM1_TRGO-triggered
    // like ADC2; circular DMA on ch2 set up below, co-triggered so frames align. No
    // boot zero-current loop: bias is derived per-capture from the buffer.
    let mut adc1 = adc12_common.claim(dp.ADC1, &mut delay);
    adc1.set_resolution(Resolution::Twelve);
    adc1.set_continuous(Continuous::Single);
    adc1.set_external_trigger((TriggerMode::RisingEdge, ExternalTrigger12::Tim_1_trgo));
    adc1.reset_sequence();
    adc1.configure_channel(&opamp1_pga, Sequence::One, SampleTime::Cycles_6_5);
    adc1.configure_channel(&pa0_vbus, Sequence::Two, SampleTime::Cycles_47_5);

    writeln!(tx, "dbg: claim adc2 (vreg+calib)\r").ok();
    let mut adc = adc12_common.claim(dp.ADC2, &mut delay);

    writeln!(tx, "dbg: configure channel\r").ok();
    adc.set_resolution(Resolution::Twelve);
    adc.set_continuous(Continuous::Single);
    adc.set_external_trigger((TriggerMode::RisingEdge, ExternalTrigger12::Tim_1_trgo));
    adc.reset_sequence();
    // 48k variant: BEMF channels at Cycles_12_5 (was 6.5) -- 2x sampling aperture for cleaner
    // reads. The 3rd BEMF aperture now ends ~1.47 us after the valley trigger, so BEMF stays
    // in-window down to ~15% duty (period is 20.8 us at 48 kHz). Currents stay at 6.5 (they
    // sit later in the scan and are secondary; valid above ~23% duty).
    adc.configure_channel(&pa4, Sequence::One, SampleTime::Cycles_12_5);
    adc.configure_channel(&pc4, Sequence::Two, SampleTime::Cycles_12_5);
    adc.configure_channel(&pb11, Sequence::Three, SampleTime::Cycles_12_5);
    adc.configure_channel(&opamp2_pga, Sequence::Four, SampleTime::Cycles_6_5);
    adc.configure_channel(&opamp3_pga, Sequence::Five, SampleTime::Cycles_6_5);

    writeln!(tx, "dbg: dma transfer ({} samples)\r", ADC_BUF_LEN).ok();
    let adc_buffer0 = cortex_m::singleton!(BUF0: [u16; ADC_BUF_LEN] = [0; ADC_BUF_LEN]).unwrap();
    let adc_buffer1 = cortex_m::singleton!(BUF1: [u16; ADC_BUF_LEN] = [0; ADC_BUF_LEN]).unwrap();
    // Raw pointers to both capture buffers for the 'd' dump. Buffer 0 is moved into the
    // DMA transfer below; the ISR re-points the DMA (via CAPTURE_BUF_ADDR) at the
    // selected buffer on each capture. We only read (volatile) after pausing the DMA.
    let buf_ptr0: *const u16 = adc_buffer0.as_ptr();
    let buf_ptr1: *const u16 = adc_buffer1.as_ptr();
    CAPTURE_BUF_ADDR.store(buf_ptr0 as u32, Ordering::Relaxed);
    let mut adc_transfer = dma_channels.ch1.into_circ_peripheral_to_memory_transfer(
        AdcDma12(adc.enable_dma(AdcDma::Continuous)),
        &mut adc_buffer0[..],
        dma_config,
    );
    writeln!(tx, "dbg: adc start\r").ok();
    adc_transfer.start(|adc| adc.start_conversion());
    writeln!(tx, "dbg: adc ok\r").ok();

    // ADC1 ring (phase-A current + VBUS) on DMA ch2, circular, NO DMA IRQ -- the
    // TIM7 ISR flips it via restart_capture_dma1 in lockstep with the ADC2 ring.
    let adc1_buffer0 =
        cortex_m::singleton!(A1BUF0: [u16; ADC1_BUF_LEN] = [0; ADC1_BUF_LEN]).unwrap();
    let adc1_buffer1 =
        cortex_m::singleton!(A1BUF1: [u16; ADC1_BUF_LEN] = [0; ADC1_BUF_LEN]).unwrap();
    let buf1_ptr0: *const u16 = adc1_buffer0.as_ptr();
    let buf1_ptr1: *const u16 = adc1_buffer1.as_ptr();
    CAPTURE_BUF1_ADDR.store(buf1_ptr0 as u32, Ordering::Relaxed);
    let dma_config_a1 = DmaConfig::default()
        .circular_buffer(true)
        .memory_increment(true);
    let mut adc1_transfer = dma_channels.ch2.into_circ_peripheral_to_memory_transfer(
        AdcDma12(adc1.enable_dma(AdcDma::Continuous)),
        &mut adc1_buffer0[..],
        dma_config_a1,
    );
    adc1_transfer.start(|adc| adc.start_conversion());

    rinz::tim7_drive::init(dp.TIM7, DRIVE_HZ, &clocks);

    // CPU headroom profiler (minz idle-loop pattern): calibrate the idle spin rate BEFORE the
    // app ISRs (TIM7/DMA1) are unmasked, so the baseline is the full-idle rate. Monotonic u64
    // wall-clock from the wrapping DWT cycle counter via two Cells (single-threaded main ctx).
    let mono_hi = core::cell::Cell::new(0u32);
    let mono_last = core::cell::Cell::new(0u32);
    let now64 = || {
        let c = cortex_m::peripheral::DWT::cycle_count();
        if c < mono_last.get() {
            mono_hi.set(mono_hi.get().wrapping_add(1));
        }
        mono_last.set(c);
        ((mono_hi.get() as u64) << 32) | (c as u64)
    };
    let mut idle = IdleLoop::new();
    idle.calibrate(170_000_000, &now64); // 1 s window == the glitch/latch cadence below

    unsafe { NVIC::unmask(stm32::Interrupt::DMA1_CH1) };
    unsafe { NVIC::unmask(stm32::Interrupt::TIM7) };

    RUNNING.store(true, Ordering::Relaxed);
    writeln!(
        tx,
        "reset: freq={}Hz amp={}.{}%\r",
        FREQ_START,
        AMP_START / 10,
        AMP_START % 10
    )
    .ok();

    // VBUS + phase-A current for each dump's debug line now come from the frozen
    // ADC1 capture buffer (power_from_buffer), not ad-hoc convert()s -- ADC1 is a
    // DMA scanner now. Computed in run_capture after the window closes.

    // Glitch-monitor emit cadence: ~1 s of the free-running DWT cycle counter (170 MHz).
    const MON_PERIOD_CYC: u32 = 170_000_000;
    let mut last_mon = cortex_m::peripheral::DWT::cycle_count();
    let mut stall_announced = false;
    let mut last_maxrun = 0u32; // for the always-on fault watcher (new coast-run highs)
    let mut prev_monitoring = false; // edge-detect 'i' enable to start cpu_busy on a full window
    let mut cpu_warned = false; // hysteresis for the FAULT: CPU_HIGH edge

    loop {
        // -------- Phase-2 always-on fault watcher (no 'i' monitor needed) --------
        // Emit ONE classified `FAULT:` line per fault EDGE with the context needed to tell
        // the precise mode apart: STALL (sustained lock loss, motor killed) vs COAST_BURST (a
        // momentary >=CL_FAULT_RUN_MIN-coast wobble that recovered). Throttled by construction:
        // STALL fires once per latch, COAST_BURST only on a NEW maxrun high (monotonic).
        // Only a CLOSED-LOOP fault is real: at alpha=0 the drive is the open-loop governor
        // (it doesn't depend on the ZC), so the stall detector's spin-up false-fires aren't
        // faults. Gate the emit on alpha>0; still track state so engaging doesn't re-fire stale.
        let alpha_on = CL_ALPHA_X1000.load(Ordering::Relaxed) > 0;
        let fired = STALL_FIRED.load(Ordering::Relaxed);
        let maxrun = CL_GLITCH_MAXRUN.load(Ordering::Relaxed);
        let fault_mode = if !alpha_on {
            None
        } else if fired && !stall_announced {
            Some("STALL")
        } else if !fired && maxrun > last_maxrun && maxrun >= CL_FAULT_RUN_MIN {
            Some("COAST_BURST")
        } else {
            None
        };
        last_maxrun = maxrun;
        stall_announced = fired;
        if let Some(mode) = fault_mode {
            writeln!(
                tx,
                "FAULT: {} comm={} coast={} burst={} bigres={} maxrun={} lockS={} jitS={} period={} hz={} amp={} alpha={} beta={} det={}\r",
                mode,
                CL_GLITCH_COMM.load(Ordering::Relaxed),
                CL_GLITCH_COAST.load(Ordering::Relaxed),
                CL_GLITCH_BURSTS.load(Ordering::Relaxed),
                CL_GLITCH_BIGRES.load(Ordering::Relaxed),
                maxrun,
                CL_LOCK_SLOW_X1000.load(Ordering::Relaxed),
                CL_JIT_SLOW_X1000.load(Ordering::Relaxed),
                CL_PERIOD_X100.load(Ordering::Relaxed),
                ELECTRICAL_HZ.load(Ordering::Relaxed),
                AMPLITUDE.load(Ordering::Relaxed),
                CL_ALPHA_X1000.load(Ordering::Relaxed),
                CL_ZC_BETA_X1000.load(Ordering::Relaxed),
                if CL_USE_SIGNCHANGE.load(Ordering::Relaxed) != 0 { "sc" } else { "lf" },
            )
            .ok();
        }

        // CPU-starvation guard (always-on, any mode): worst-case TIM7 ISR vs its tick budget.
        let isr_util = DBG_ISR_CYC.load(Ordering::Relaxed) * 100 / (170_000_000 / DRIVE_HZ).max(1);
        if isr_util >= CL_CPU_HIGH_PCT && !cpu_warned {
            cpu_warned = true;
            writeln!(
                tx,
                "FAULT: CPU_HIGH isr_util={} budget_cyc={} hz={} alpha={} det={}\r",
                isr_util,
                170_000_000 / DRIVE_HZ,
                ELECTRICAL_HZ.load(Ordering::Relaxed),
                CL_ALPHA_X1000.load(Ordering::Relaxed),
                if CL_USE_SIGNCHANGE.load(Ordering::Relaxed) != 0 {
                    "sc"
                } else {
                    "lf"
                },
            )
            .ok();
        } else if isr_util + 10 < CL_CPU_HIGH_PCT {
            cpu_warned = false;
        }

        // Service the live stream: one back-to-back dump per idle pass while the
        // STREAMING flag is set. 'l' sets it, 'k' (or 'w') clears it; everything
        // else is the normal command set, processed unchanged below.
        let streaming = STREAMING.load(Ordering::Relaxed);
        if streaming {
            // Binary (Ascii85) dumps: streaming is throughput-bound, so the ~2x
            // smaller payload roughly doubles the live refresh rate.
            run_capture(&mut tx, buf_ptr0, buf_ptr1, buf1_ptr0, buf1_ptr1, true);
        }

        // Long-running glitch monitor: while MONITOR is set (and not streaming), emit one
        // self-describing `glitch:` line per ~1 s without needing a capture. This is the
        // honest way to catch the rare, seconds-apart events: run a window, compare counts.
        let monitoring = MONITOR.load(Ordering::Relaxed);
        if monitoring && !prev_monitoring {
            // 'i' just enabled: start the cpu_busy window fresh so the first reading isn't a
            // partial-window artifact (the inflated 100/76/.. the user saw on each re-enable).
            last_mon = cortex_m::peripheral::DWT::cycle_count();
            idle.latch();
        }
        prev_monitoring = monitoring;
        if monitoring && !streaming {
            let now = cortex_m::peripheral::DWT::cycle_count();
            if now.wrapping_sub(last_mon) >= MON_PERIOD_CYC {
                last_mon = now;
                writeln!(
                    tx,
                    "glitch: comm={} coast={} burst={} resid={} maxrun={} since={} lockS={} mode={} det={} alpha={} beta={} pc={} cpu_busy={} isr_util={}\r",
                    CL_GLITCH_COMM.load(Ordering::Relaxed),
                    CL_GLITCH_COAST.load(Ordering::Relaxed),
                    CL_GLITCH_BURSTS.load(Ordering::Relaxed),
                    CL_GLITCH_BIGRES.load(Ordering::Relaxed),
                    CL_GLITCH_MAXRUN.load(Ordering::Relaxed),
                    CL_GLITCH_SINCE.load(Ordering::Relaxed),
                    CL_LOCK_SLOW_X1000.load(Ordering::Relaxed),
                    if CL_DRIVING_FLAG.load(Ordering::Relaxed) { "drive" } else { "gov" },
                    if CL_USE_SIGNCHANGE.load(Ordering::Relaxed) != 0 { "sc" } else { "lf" },
                    CL_ALPHA_X1000.load(Ordering::Relaxed),
                    CL_ZC_BETA_X1000.load(Ordering::Relaxed),
                    CL_PREDICT_COAST.load(Ordering::Relaxed),
                    // total CPU busy% (idle-loop: 100 - idle spin vs calibration) and the TIM7
                    // ISR's share of its own 48 kHz budget (cyc / (170MHz/DRIVE_HZ)).
                    idle.busy_percentage(idle.latch()),
                    DBG_ISR_CYC.load(Ordering::Relaxed) * 100 / (170_000_000 / DRIVE_HZ).max(1),
                )
                .ok();
                let r = |i: usize| CL_RUN_HIST[i].load(Ordering::Relaxed);
                let x = |i: usize| CL_RESID_HIST[i].load(Ordering::Relaxed);
                writeln!(
                    tx,
                    "hist: runlen={},{},{},{},{},{},{},{} resid={},{},{},{},{},{}\r",
                    r(0),
                    r(1),
                    r(2),
                    r(3),
                    r(4),
                    r(5),
                    r(6),
                    r(7),
                    x(0),
                    x(1),
                    x(2),
                    x(3),
                    x(4),
                    x(5),
                )
                .ok();
            }
        }

        // Fetch the next command key. Block for one only when idle, so streaming
        // stays gap-free; while streaming or monitoring, just take what's pending.
        let key = if streaming || monitoring {
            let mut buf = [0u8; 1];
            if rx.read_ready().unwrap_or(false) && rx.read(&mut buf).is_ok() {
                Some(buf[0])
            } else {
                None
            }
        } else {
            let mut buf = [0u8; 1];
            if rx.read(&mut buf).is_ok() {
                Some(buf[0])
            } else {
                None
            }
        };

        if let Some(k) = key {
            WATCHDOG_TICKS.store(0, Ordering::Relaxed); // pet: any serial activity
            if WATCHDOG_FIRED.swap(false, Ordering::Relaxed) {
                writeln!(tx, "watchdog: FIRED -- motor was killed (host stall)\r").ok();
            }
            match k {
                b'p' => {
                    let armed = !WATCHDOG_ARMED.load(Ordering::Relaxed);
                    WATCHDOG_ARMED.store(armed, Ordering::Relaxed);
                    WATCHDOG_TICKS.store(0, Ordering::Relaxed);
                    if armed {
                        writeln!(
                            tx,
                            "watchdog: ARMED ({}s)\r",
                            WATCHDOG_TIMEOUT_TICKS / DRIVE_HZ
                        )
                        .ok();
                    } else {
                        writeln!(tx, "watchdog: disarmed\r").ok();
                    }
                }
                b'l' => {
                    STREAMING.store(true, Ordering::Relaxed);
                    writeln!(tx, "stream: start (k stops)\r").ok();
                }
                b'k' => {
                    STREAMING.store(false, Ordering::Relaxed);
                    writeln!(tx, "stream: stop\r").ok();
                }
                // glitch monitor: 'i' toggles the periodic `glitch:` line, 'x' zeroes counters
                b'i' => {
                    let on = !MONITOR.load(Ordering::Relaxed);
                    MONITOR.store(on, Ordering::Relaxed);
                    writeln!(tx, "monitor: {}\r", if on { "on" } else { "off" }).ok();
                }
                b'x' => {
                    CL_GLITCH_RESET.store(true, Ordering::Relaxed);
                    writeln!(tx, "glitch: reset\r").ok();
                }
                // toggle stall-kill (kill the motor on a detected stall)
                b'h' => {
                    let on = !STALL_KILL_EN.load(Ordering::Relaxed);
                    STALL_KILL_EN.store(on, Ordering::Relaxed);
                    writeln!(tx, "stall_kill: {}\r", if on { "on" } else { "off" }).ok();
                }
                b'd' => {
                    writeln!(tx, "capture: wait zero, 2 electrical revs\r").ok();
                    run_capture(&mut tx, buf_ptr0, buf_ptr1, buf1_ptr0, buf1_ptr1, false);
                    // Reject re-triggers: drain keys that landed in the RDR during
                    // the long dump (read_ready keeps this non-blocking).
                    let mut drain = [0u8; 1];
                    while rx.read_ready().unwrap_or(false) && rx.read(&mut drain).is_ok() {}
                }
                b'c' => {
                    // Same capture as 'd', but Ascii85-packed binary (~2x faster).
                    writeln!(tx, "capture: wait zero, 2 electrical revs (binary)\r").ok();
                    run_capture(&mut tx, buf_ptr0, buf_ptr1, buf1_ptr0, buf1_ptr1, true);
                    let mut drain = [0u8; 1];
                    while rx.read_ready().unwrap_or(false) && rx.read(&mut drain).is_ok() {}
                }
                b'w' => {
                    // Kill also leaves stream mode (a streaming run_capture would
                    // otherwise re-assert RUNNING on the next pass).
                    STREAMING.store(false, Ordering::Relaxed);
                    handle_command(b'w', &mut tx);
                }
                // ---- Path B closed-loop authority (alpha) as direct levels ----
                // 0/1/2/3 = alpha 0.0/0.2/0.5/1.0. These pass straight through
                // scope_live_ui (u/i/o collide with its replay/loop bindings). '0' is
                // the instant open-loop fallback.
                b'0' | b'1' | b'2' | b'3' => {
                    let a = match k {
                        b'1' => 200,
                        b'2' => 500,
                        b'3' => 1000,
                        _ => 0,
                    };
                    CL_ALPHA_X1000.store(a, Ordering::Relaxed);
                    writeln!(
                        tx,
                        "alpha={}.{:02}{}\r",
                        a / 1000,
                        (a % 1000) / 10,
                        if a == 0 { " (open-loop fallback)" } else { "" }
                    )
                    .ok();
                }
                // fine alpha steps (relative +/-0.05) for gradual ramps -- used by
                // cl_alpha_tune over raw serial (no scope_live_ui conflict).
                b'n' | b'm' => {
                    let cur = CL_ALPHA_X1000.load(Ordering::Relaxed) as i32;
                    let a = (cur + if k == b'm' { 50 } else { -50 }).clamp(0, 1000) as u32;
                    CL_ALPHA_X1000.store(a, Ordering::Relaxed);
                    writeln!(tx, "alpha={}.{:02}\r", a / 1000, (a % 1000) / 10).ok();
                }
                // per-sector ZC smoothing weight (','=down '.'=up, +/-0.05)
                b',' | b'.' => {
                    let cur = CL_ZC_BETA_X1000.load(Ordering::Relaxed) as i32;
                    let v = (cur + if k == b'.' { 50 } else { -50 }).clamp(0, 950) as u32;
                    CL_ZC_BETA_X1000.store(v, Ordering::Relaxed);
                    writeln!(tx, "zc_beta={}.{:02}\r", v / 1000, (v % 1000) / 10).ok();
                }
                // commutation timing advance -/+ (sector fraction x1000, step 0.025 = 1.5 deg)
                b'<' | b'>' => {
                    let cur = CL_ADVANCE_X1000.load(Ordering::Relaxed) as i32;
                    let v = (cur + if k == b'>' { 25 } else { -25 }).clamp(0, 450) as u32;
                    CL_ADVANCE_X1000.store(v, Ordering::Relaxed);
                    writeln!(
                        tx,
                        "advance={}.{:03} ({} deg)\r",
                        v / 1000,
                        v % 1000,
                        v * 60 / 1000
                    )
                    .ok();
                }
                // toggle speed-proportional advance schedule (ramp 0->cap near the ceiling)
                b'/' => {
                    let v = (CL_ADV_SCHED.load(Ordering::Relaxed) == 0) as u32;
                    CL_ADV_SCHED.store(v, Ordering::Relaxed);
                    writeln!(tx, "adv_sched={}\r", v).ok();
                }
                // toggle predictive coast (per-sector-memory schedule on a missed ZC)
                b'y' => {
                    let v = (CL_PREDICT_COAST.load(Ordering::Relaxed) == 0) as u32;
                    CL_PREDICT_COAST.store(v, Ordering::Relaxed);
                    writeln!(tx, "predict_coast={}\r", v).ok();
                }
                // toggle the loop's detector source: linfit (fabricates out-of-window) <-> sign-change
                b'j' => {
                    let v = (CL_USE_SIGNCHANGE.load(Ordering::Relaxed) == 0) as u32;
                    CL_USE_SIGNCHANGE.store(v, Ordering::Relaxed);
                    writeln!(
                        tx,
                        "detector={}\r",
                        if v != 0 { "signchange" } else { "linfit" }
                    )
                    .ok();
                }
                // harmonic mode cycle 2->1->0->2 (full / push-only / off) to split its ISR cost
                b';' => {
                    let cur = CL_HARM_EN.load(Ordering::Relaxed);
                    let v = if cur == 0 { 2 } else { cur - 1 };
                    CL_HARM_EN.store(v, Ordering::Relaxed);
                    let name = match v {
                        2 => "full",
                        1 => "push",
                        _ => "off",
                    };
                    writeln!(tx, "harm_en={}\r", name).ok();
                }
                // Stage-2: harmonic drives commutation (vs sign-change). Bounded by alpha/slew.
                b'\'' => {
                    let v = (CL_HARM_DRIVE.load(Ordering::Relaxed) == 0) as u32;
                    CL_HARM_DRIVE.store(v, Ordering::Relaxed);
                    writeln!(tx, "harm_drive={}\r", if v != 0 { "on" } else { "off" }).ok();
                }
                // full-sensorless drive: governor <-> on_frame (PLL drives the rate when locked)
                b'o' => {
                    let v = (CL_DRIVE.load(Ordering::Relaxed) == 0) as u32;
                    CL_DRIVE.store(v, Ordering::Relaxed);
                    writeln!(tx, "drive={}\r", if v != 0 { "on" } else { "off" }).ok();
                }
                // predictive-coast engage gate (lock_fast threshold) -0.05 / +0.05
                b'e' | b'r' => {
                    let cur = CL_PREDICT_GATE_X100.load(Ordering::Relaxed) as i32;
                    let v = (cur + if k == b'r' { 5 } else { -5 }).clamp(0, 100) as u32;
                    CL_PREDICT_GATE_X100.store(v, Ordering::Relaxed);
                    writeln!(tx, "predict_gate={}.{:02}\r", v / 100, v % 100).ok();
                }
                other => handle_command(other, &mut tx),
            }
        }

        // CPU headroom: while monitoring (and not streaming, which is its own busy work), spin
        // the idle counter for a ~250 us slice. Active work + ISR steal both reduce it vs the
        // boot calibration -> busy_percentage. Only meaningful when monitoring spins the loop
        // (the idle path otherwise blocks on rx.read, so there is no slack to count).
        if monitoring && !streaming {
            idle.run_until(now64() + 42_500, &now64); // 42_500 cyc / 170 MHz = 250 us
        }
    }
}

/// VBUS (mV) and phase-A current proxy (mA) from a frozen ADC1 ring (2 vals/frame:
/// [I_A ch13, VBUS ch1], 12-bit). VBUS = mean (counts -> mV at 3.3V ref, x10.39
/// divider). iu = mean-rectified swing of I_A about its window mean, /48 = gain16 *
/// shunt -> mA (magnitude proxy, jumps at lock-catch; not a calibrated bus amp).
fn power_from_buffer(ptr1: *const u16, frames: usize) -> (u32, u32) {
    if frames == 0 {
        return (0, 0);
    }
    let read = |f: usize, ch: usize| -> u32 {
        unsafe { core::ptr::read_volatile(ptr1.add(f * ADC1_CHANNELS + ch)) as u32 }
    };
    let mut isum = 0u32;
    let mut vsum = 0u32;
    for f in 0..frames {
        isum += read(f, 0);
        vsum += read(f, 1);
    }
    let n = frames as u32;
    let imean = isum / n;
    let vbus_mv = (vsum / n) * 3300 / 4095 * 1039 / 100;
    let mut iacc = 0u32;
    for f in 0..frames {
        iacc += (read(f, 0) as i32 - imean as i32).unsigned_abs();
    }
    let iu_ma = (iacc / n) * 3300 / 4095 * 1000 / 48;
    (vbus_mv, iu_ma)
}

/// Run one capture+dump cycle: arm the back/front flip, wait for the phase-aligned
/// window to close, then emit the debug line, register snapshot and hex frame dump.
/// Shared by the single-shot `d` command and the continuous `l` stream. Both the
/// ADC2 ring (buf_ptr*) and the co-triggered ADC1 ring (buf1_ptr*) are flipped in
/// lockstep; VBUS/iu_mA come from the frozen ADC1 ring.
fn run_capture<TX: Write>(
    tx: &mut TX,
    buf_ptr0: *const u16,
    buf_ptr1: *const u16,
    buf1_ptr0: *const u16,
    buf1_ptr1: *const u16,
    binary: bool,
) {
    WATCHDOG_TICKS.store(0, Ordering::Relaxed); // pet: keeps the watchdog fed while streaming
    // Flip to the other buffer; the ISR aims the DMA at CAPTURE_BUF_ADDR for this
    // capture, then flips it onto CAPTURE_BUF_ALT when the window closes.
    let sel = !BUF_SEL.load(Ordering::Relaxed);
    BUF_SEL.store(sel, Ordering::Relaxed);
    let cap_ptr = if sel { buf_ptr1 } else { buf_ptr0 };
    let alt_ptr = if sel { buf_ptr0 } else { buf_ptr1 };
    let cap_ptr1 = if sel { buf1_ptr1 } else { buf1_ptr0 };
    let alt_ptr1 = if sel { buf1_ptr0 } else { buf1_ptr1 };
    CAPTURE_BUF_ADDR.store(cap_ptr as u32, Ordering::Relaxed);
    CAPTURE_BUF_ALT.store(alt_ptr as u32, Ordering::Relaxed);
    CAPTURE_BUF1_ADDR.store(cap_ptr1 as u32, Ordering::Relaxed);
    CAPTURE_BUF1_ALT.store(alt_ptr1 as u32, Ordering::Relaxed);

    CAPTURE_DONE.store(false, Ordering::Relaxed);
    CAPTURE_FRAMES.store(0, Ordering::Relaxed);
    CAPTURE_TICKS_TARGET.store(0, Ordering::Relaxed);
    CAPTURE_REQUEST.store(true, Ordering::Relaxed);
    RUNNING.store(true, Ordering::Relaxed);

    while !CAPTURE_DONE.load(Ordering::Relaxed) {
        cortex_m::asm::nop();
    }

    let frames = CAPTURE_FRAMES.load(Ordering::Relaxed) as usize;
    let (vbus_mv, iu_ma) = power_from_buffer(cap_ptr1, frames);
    writeln!(
        tx,
        "debug: hz={} amp={} trim={} vbus_mv={} iu_ma={} alpha={} zc_beta={} predict={} det={} pgate={} mode={} period_est={} lock_fast={} lock_slow={} jit_fast={} jit_slow={} isr_cyc={} tim7={} six_ticks={} dma_tc={} dma_ht={} dma_te={}\r",
        DBG_CAPTURE_HZ.load(Ordering::Relaxed),
        DBG_CAPTURE_AMP.load(Ordering::Relaxed),
        DUTY_TRIM.load(Ordering::Relaxed),
        vbus_mv,
        iu_ma,
        CL_ALPHA_X1000.load(Ordering::Relaxed),
        CL_ZC_BETA_X1000.load(Ordering::Relaxed),
        CL_PREDICT_COAST.load(Ordering::Relaxed),
        if CL_USE_SIGNCHANGE.load(Ordering::Relaxed) != 0 { "sc" } else { "lf" },
        CL_PREDICT_GATE_X100.load(Ordering::Relaxed),
        if CL_DRIVING_FLAG.load(Ordering::Relaxed) { "drive" } else { "gov" },
        CL_PERIOD_X100.load(Ordering::Relaxed),
        CL_LOCK_FAST_X1000.load(Ordering::Relaxed),
        CL_LOCK_SLOW_X1000.load(Ordering::Relaxed),
        CL_JIT_FAST_X1000.load(Ordering::Relaxed),
        CL_JIT_SLOW_X1000.load(Ordering::Relaxed),
        DBG_ISR_CYC.load(Ordering::Relaxed),
        DBG_TIM7_TICKS.load(Ordering::Relaxed),
        DBG_SIX_STEP_TICKS.load(Ordering::Relaxed),
        DBG_DMA_TC.load(Ordering::Relaxed),
        DBG_DMA_HT.load(Ordering::Relaxed),
        DBG_DMA_TE.load(Ordering::Relaxed)
    )
    .ok();
    dump_debug_registers(tx);
    dump_cl_log(tx); // observe-only per-commutation ZC log (before the frame dump)
    // No motor kill, no DMA pause: the DMA is already streaming into the alt buffer
    // (flipped by the ISR), so dump the frozen buffer in place while capture and
    // commutation keep running. `binary` picks Ascii85-packed (c) vs hex (d).
    if binary {
        dump_buffer_b85(tx, cap_ptr, cap_ptr1, frames);
    } else {
        dump_buffer(tx, cap_ptr, cap_ptr1, frames);
    }
}

/// Dump the (frozen) capture as 12-bit hex (4-digit). One line is one frame:
/// ch17/ch5/ch14 (BEMF A/B/C voltages), ch16/ch18 (phase-B/C currents) from the
/// ADC2 ring, then ch13/ch1 (phase-A current, VBUS) from the ADC1 ring -- frame i
/// aligns across both since they share the TIM1_TRGO valley trigger.
fn dump_buffer<TX: Write>(tx: &mut TX, ptr: *const u16, ptr1: *const u16, frames: usize) {
    writeln!(
        tx,
        "dump7: {} frames x 7 channels (ch17 ch5 ch14 ch16 ch18 ch13 ch1, 12-bit ADC, {} Hz)\r",
        frames, ADC_FRAME_HZ
    )
    .ok();
    for frame in 0..frames {
        for ch in 0..ADC_CHANNELS {
            let v = unsafe { core::ptr::read_volatile(ptr.add(frame * ADC_CHANNELS + ch)) };
            write!(tx, "{:04x} ", v).ok();
        }
        for ch in 0..ADC1_CHANNELS {
            let v = unsafe { core::ptr::read_volatile(ptr1.add(frame * ADC1_CHANNELS + ch)) };
            write!(tx, "{:04x}", v).ok();
            if ch + 1 < ADC1_CHANNELS {
                write!(tx, " ").ok();
            }
        }
        writeln!(tx, "\r").ok();
    }
    writeln!(tx, "\rend\r").ok();
}

/// Encode one 4-byte group (big-endian) as standard Ascii85: 5 base-85 digits, each
/// +33 ('!'..='u'). For the final short group of `valid` bytes (1..=3, rest zero-
/// padded) only `valid + 1` chars are emitted. Wraps lines at ~80 chars. Decodes
/// with Python `base64.a85decode` (which ignores the newlines).
fn emit_a85_group<TX: Write>(tx: &mut TX, group: &[u8; 4], valid: usize, col: &mut usize) {
    let num = ((group[0] as u32) << 24)
        | ((group[1] as u32) << 16)
        | ((group[2] as u32) << 8)
        | (group[3] as u32);
    let mut digits = [0u8; 5];
    let mut v = num;
    for d in digits.iter_mut().rev() {
        *d = (v % 85) as u8 + 33;
        v /= 85;
    }
    let emit = if valid == 4 { 5 } else { valid + 1 };
    for &d in digits.iter().take(emit) {
        write!(tx, "{}", d as char).ok();
        *col += 1;
        if *col >= 80 {
            writeln!(tx, "\r").ok();
            *col = 0;
        }
    }
}

/// Binary counterpart to dump_buffer: the same 7 channels/frame as raw u16
/// little-endian samples, Ascii85-packed (~1.25x vs 2x for hex -> ~2x faster). The
/// `cdump:` header mirrors dump7 so the host auto-detects channels/scale; only the
/// payload codec differs (b85 vs hex).
fn dump_buffer_b85<TX: Write>(tx: &mut TX, ptr: *const u16, ptr1: *const u16, frames: usize) {
    writeln!(
        tx,
        "cdump: {} frames x 7 channels (ch17 ch5 ch14 ch16 ch18 ch13 ch1, 12-bit, b85, {} Hz)\r",
        frames, ADC_FRAME_HZ
    )
    .ok();
    let mut group = [0u8; 4];
    let mut gi = 0usize;
    let mut col = 0usize;
    let mut push = |tx: &mut TX, byte: u8| {
        group[gi] = byte;
        gi += 1;
        if gi == 4 {
            emit_a85_group(tx, &group, 4, &mut col);
            gi = 0;
            group = [0u8; 4];
        }
    };
    for frame in 0..frames {
        for ch in 0..ADC_CHANNELS {
            let v = unsafe { core::ptr::read_volatile(ptr.add(frame * ADC_CHANNELS + ch)) };
            push(tx, v as u8);
            push(tx, (v >> 8) as u8);
        }
        for ch in 0..ADC1_CHANNELS {
            let v = unsafe { core::ptr::read_volatile(ptr1.add(frame * ADC1_CHANNELS + ch)) };
            push(tx, v as u8);
            push(tx, (v >> 8) as u8);
        }
    }
    if gi > 0 {
        emit_a85_group(tx, &group, gi, &mut col); // final short group (zero-padded)
    }
    if col > 0 {
        writeln!(tx, "\r").ok();
    }
    writeln!(tx, "end\r").ok();
}

/// Dump the observe-only per-commutation ZC log for the captured window. Emitted
/// BEFORE the `dump7:` header so the existing host parser ignores it (it scans only
/// `debug:`/`regs:` pre-header); the closed-loop host tool reads the `cl ...` lines.
/// Each line: physical sector, real-time ZC position (% of the 60-deg float window;
/// -1 = no in-window crossing), and the sector period in ticks. The OFFLINE oracle is
/// computed host-side on the SAME dumped frames; their agreement is the Stage-1 gate.
fn dump_cl_log<TX: Write>(tx: &mut TX) {
    let n = (CL_CAP_N.load(Ordering::Relaxed) as usize).min(CL_MAXCOMM);
    writeln!(
        tx,
        "cl: {} commutations CLOSED-LOOP (i phys zc_pct coasted lf=raw_linfit_pct harm=harmonic_pct bnd=cap_frame)\r",
        n
    )
    .ok();
    for i in 0..n {
        let v = CL_CAP[i].load(Ordering::Relaxed);
        let phys = (v >> 16) & 0xff;
        let zcb = (v >> 8) & 0xff;
        let coast = v & 0xff;
        let zc: i32 = if zcb == 255 { -1 } else { zcb as i32 };
        let lf = CL_CAP_LF[i].load(Ordering::Relaxed); // raw signed linfit %, 9999 = none
        let bnd = CL_CAP_BND[i].load(Ordering::Relaxed); // capture-frame of this commutation
        let harm = CL_CAP_HARM[i].load(Ordering::Relaxed); // observe-only harmonic %, 9999 = none
        writeln!(
            tx,
            "cl i={} phys={} zc={} coast={} lf={} harm={} bnd={}\r",
            i, phys, zc, coast, lf, harm, bnd
        )
        .ok();
    }
}

fn dump_debug_registers<TX: Write>(tx: &mut TX) {
    let t1 = unsafe { &*stm32::TIM1::ptr() };
    let adc2 = unsafe { &*stm32::ADC2::ptr() };
    let dma = unsafe { &*stm32::DMA1::ptr() };
    let ch = dma.ch1();

    writeln!(
        tx,
        "regs: t1_cr2={:08x} t1_arr={:04x} t1_ccr4={:04x}\r",
        t1.cr2().read().bits(),
        t1.arr().read().arr().bits(),
        t1.ccr4().read().ccr().bits(),
    )
    .ok();
    writeln!(
        tx,
        "regs: adc2_cfgr={:08x} adc2_isr={:08x} dma_isr={:08x} ch1_cr={:08x} ch1_ndtr={:04x}\r",
        adc2.cfgr().read().bits(),
        adc2.isr().read().bits(),
        dma.isr().read().bits(),
        ch.cr().read().bits(),
        ch.ndtr().read().ndt().bits()
    )
    .ok();
}

#[allow(non_snake_case)]
#[unsafe(no_mangle)]
extern "C" fn DMA1_CH1() {
    let dma = unsafe { &*stm32::DMA1::ptr() };
    let flags = dma.isr().read().bits() & 0x0f;
    dma.ifcr().write(|w| unsafe { w.bits(0x0f) });

    if flags & (1 << 1) != 0 {
        DBG_DMA_TC.fetch_add(1, Ordering::Relaxed);
    }
    if flags & (1 << 2) != 0 {
        DBG_DMA_HT.fetch_add(1, Ordering::Relaxed);
    }
    if flags & (1 << 3) != 0 {
        DBG_DMA_TE.fetch_add(1, Ordering::Relaxed);
    }
}

#[allow(non_snake_case)]
#[unsafe(no_mangle)]
extern "C" fn TIM7() {
    rinz::tim7_drive::clear_update_flag();
    DBG_TIM7_TICKS.fetch_add(1, Ordering::Relaxed);

    static mut CL: Option<ClLoop> = None;
    static mut CL_DRIVING: bool = false; // full-sensorless drive engaged (hysteretic, lock-gated)
    static mut CL_LAST_ZC: u32 = 255;
    static mut CL_LAST_LF: i32 = 9999; // raw signed linfit ZC% this sector (9999 = none)
    static mut CAPTURE_WAIT_ZERO: bool = false;
    static mut CAPTURE_ACTIVE: bool = false;
    static mut CAPTURE_TICKS: u32 = 0;

    if !RUNNING.load(Ordering::Relaxed) {
        return;
    }

    // Command watchdog: while armed (and not already fired), count up; if no serial
    // pet for the timeout, kill the motor here in the ISR -- independent of the main
    // loop, so a hung host can't keep a stalled rotor energized.
    if WATCHDOG_ARMED.load(Ordering::Relaxed) && !WATCHDOG_FIRED.load(Ordering::Relaxed) {
        let t = WATCHDOG_TICKS.fetch_add(1, Ordering::Relaxed) + 1;
        if t >= WATCHDOG_TIMEOUT_TICKS {
            kill_motor();
            WATCHDOG_FIRED.store(true, Ordering::Relaxed);
            return;
        }
    }

    let electrical_hz = ELECTRICAL_HZ.load(Ordering::Relaxed);
    let amplitude = AMPLITUDE.load(Ordering::Relaxed);

    // Settle the MCU-bottleneck question: worst-case active-path ISR duration in cycles.
    // Budget = 170MHz / DRIVE_HZ = 170M/48k = 3541 cyc. fetch_max keeps the spike; reset at capture.
    let isr_t0 = cortex_m::peripheral::DWT::cycle_count();

    unsafe {
        if CAPTURE_REQUEST.swap(false, Ordering::Relaxed) {
            CAPTURE_WAIT_ZERO = true;
            CAPTURE_ACTIVE = false;
            CAPTURE_TICKS = 0;
        }

        // ---- Path B: the validated cl::ClLoop drives commutation, alpha-blended ----
        // alpha=0 -> commutate exactly on the open-loop schedule (governor/fallback);
        // alpha>0 -> nudge toward the ZC-derived schedule within +/- CL_SLEW_FRAC.
        let ol_period = DRIVE_HZ as f32 / (electrical_hz.max(1) * 6) as f32;
        let alpha = CL_ALPHA_X1000.load(Ordering::Relaxed) as f32 / 1000.0;
        let cl_ptr = &raw mut CL;
        if (*cl_ptr).is_none() {
            *cl_ptr = Some(ClLoop::new(
                ol_period,
                CL_KP,
                CL_KI,
                CL_GATE_FRAC,
                CL_LOOP_COAST,
                CL_BLANK,
            ));
        }
        let cl = (*cl_ptr).as_mut().unwrap();
        cl.set_zc_beta(CL_ZC_BETA_X1000.load(Ordering::Relaxed) as f32 / 1000.0); // live-tunable
        // Commutation advance. CL_ADVANCE_X1000 is the CAP; with the schedule on ('/'), the
        // effective advance ramps 0 -> cap linearly from ADV_SCHED_HZ0 over ADV_SCHED_RAMP_HZ, so
        // the low end stays un-advanced (clean catch) and full advance lands only near the ceiling.
        let adv_cap = CL_ADVANCE_X1000.load(Ordering::Relaxed) as f32 / 1000.0;
        let adv = if CL_ADV_SCHED.load(Ordering::Relaxed) != 0 {
            let above = electrical_hz.saturating_sub(ADV_SCHED_HZ0) as f32;
            (adv_cap * above / ADV_SCHED_RAMP_HZ as f32).min(adv_cap)
        } else {
            adv_cap
        };
        cl.set_advance(adv);
        cl.set_predict_coast(CL_PREDICT_COAST.load(Ordering::Relaxed) != 0);
        cl.set_use_signchange(CL_USE_SIGNCHANGE.load(Ordering::Relaxed) != 0);
        cl.set_harm_mode(CL_HARM_EN.load(Ordering::Relaxed) as u8);
        cl.set_harm_drive(CL_HARM_DRIVE.load(Ordering::Relaxed) != 0);
        cl.set_predict_gate(CL_PREDICT_GATE_X100.load(Ordering::Relaxed) as f32 / 100.0);
        cl.set_stall_run(CL_STALL_RUN);
        if CL_GLITCH_RESET.swap(false, Ordering::Relaxed) {
            cl.reset_glitch();
        }
        if CL_STALL_RESET.swap(false, Ordering::Relaxed) {
            cl.reset_stall(); // re-arm after a re-spin / restart
        }
        // --- Full-sensorless DRIVE handoff (Step 3) -----------------------------------
        // In drive mode on_frame's period PLL sets the COMMUTATION RATE from the BEMF ZC, so
        // rotor speed is an OUTPUT (set by amp/load), free to exceed the governor's ~450 Hz
        // open-loop spin envelope. Governed mode (on_frame_blend) is the startup ramp AND the
        // fallback. Hysteretic, lock_slow-gated so we only ever drive off a genuine lock.
        let want_drive = CL_DRIVE.load(Ordering::Relaxed) != 0;
        let lock = cl.lock_slow();
        CL_DRIVING = want_drive
            && if CL_DRIVING {
                lock > CL_DRIVE_DROP_LOCK // stay until lock drops well below (hysteresis)
            } else {
                lock > CL_DRIVE_HANDOFF_LOCK // enter only once solidly locked
            };
        CL_DRIVING_FLAG.store(CL_DRIVING, Ordering::Relaxed);

        if !CL_DRIVING && alpha <= 0.0 {
            cl.set_period(ol_period); // governed open loop: pin period for a sane later handoff
            // Open loop: the governor spins the motor regardless of the ZC, so a detector
            // "stall" is meaningless -- don't let it kill the motor (no silent kill on 'q').
            cl.reset_stall();
        }
        let arr = (*stm32::TIM1::ptr()).arr().read().arr().bits() as u32;
        let frame = read_latest_adc2_frame();
        let bemf = [frame[0] as i32, frame[1] as i32, frame[2] as i32];
        let step = if CL_DRIVING {
            cl.clamp_period(CL_DRIVE_PMIN, CL_DRIVE_PMAX); // runaway bound, every tick (uniform)
            cl.on_frame(bemf) // PLL drives the rate -- full sensorless, past the governor cap
        } else {
            cl.on_frame_blend(bemf, ol_period, alpha, CL_SLEW_FRAC)
        };
        // While driving, slew the commanded freq toward period_est so (a) the readout follows
        // the rotor and (b) the governor fallback is already at rotor speed -> smooth revert.
        if CL_DRIVING {
            let hz = (DRIVE_HZ as f32 / (6.0 * step.period_est.max(1.0))) as u32;
            ELECTRICAL_HZ.store(hz.clamp(1, 2000), Ordering::Relaxed);
        }
        CL_PERIOD_X100.store((step.period_est * 100.0) as u32, Ordering::Relaxed);
        CL_LOCK_FAST_X1000.store((cl.lock_fast() * 1000.0) as u32, Ordering::Relaxed);
        CL_LOCK_SLOW_X1000.store((cl.lock_slow() * 1000.0) as u32, Ordering::Relaxed);
        CL_JIT_FAST_X1000.store((cl.jit_fast() * 1000.0) as u32, Ordering::Relaxed);
        CL_JIT_SLOW_X1000.store((cl.jit_slow() * 1000.0) as u32, Ordering::Relaxed);
        let (g_comm, g_coast, g_burst, g_resid, g_maxrun, g_since) = cl.glitch_stats();
        CL_GLITCH_COMM.store(g_comm, Ordering::Relaxed);
        CL_GLITCH_COAST.store(g_coast, Ordering::Relaxed);
        CL_GLITCH_BURSTS.store(g_burst, Ordering::Relaxed);
        CL_GLITCH_BIGRES.store(g_resid, Ordering::Relaxed);
        CL_GLITCH_MAXRUN.store(g_maxrun, Ordering::Relaxed);
        CL_GLITCH_SINCE.store(g_since, Ordering::Relaxed);
        let rh = cl.run_hist();
        for (i, v) in rh.iter().enumerate() {
            CL_RUN_HIST[i].store(*v, Ordering::Relaxed);
        }
        let xh = cl.resid_hist();
        for (i, v) in xh.iter().enumerate() {
            CL_RESID_HIST[i].store(*v, Ordering::Relaxed);
        }
        // Stall -> kill (rising edge). The loop lost a lock it held -> stop the motor.
        // kill_motor() drops RUNNING, so this ISR early-returns next pass; the main loop
        // reports STALL_FIRED; a re-spin ('q') re-arms via CL_STALL_RESET.
        if cl.stalled()
            && STALL_KILL_EN.load(Ordering::Relaxed)
            && !STALL_FIRED.swap(true, Ordering::Relaxed)
        {
            kill_motor();
        }
        if let Some(t) = step.zc_ticks {
            CL_LAST_ZC = ((t / ol_period) * 100.0) as u32; // ZC % of window (telemetry)
            CL_LAST_LF = ((t / ol_period) * 100.0) as i32; // raw signed (keeps out-of-window)
        }
        let wrapped_to_zero = step.commutate && step.sector == 0; // electrical-rev start
        if CAPTURE_ACTIVE {
            DBG_SIX_STEP_TICKS.fetch_add(1, Ordering::Relaxed);
        }
        let base = (arr * amplitude * 2 / (1000 * 3)) as i32;
        let duty = (base + DUTY_TRIM.load(Ordering::Relaxed)).clamp(0, arr as i32) as u32;
        set_six_step(step.sector * 12, duty); // drive the physical sector (logical = phys*12)

        let mut capture_started = false;
        if CAPTURE_WAIT_ZERO && wrapped_to_zero {
            let frames = two_rev_adc_frame_count(electrical_hz);
            let ticks = two_rev_drive_tick_count(electrical_hz);
            DBG_TIM7_TICKS.store(0, Ordering::Relaxed);
            DBG_DMA_TC.store(0, Ordering::Relaxed);
            DBG_DMA_HT.store(0, Ordering::Relaxed);
            DBG_DMA_TE.store(0, Ordering::Relaxed);
            DBG_CAPTURE_AMP.store(amplitude, Ordering::Relaxed);
            DBG_CAPTURE_HZ.store(electrical_hz, Ordering::Relaxed);
            DBG_SIX_STEP_TICKS.store(0, Ordering::Relaxed);
            DBG_ISR_CYC.store(0, Ordering::Relaxed); // reset worst-case ISR timer for this capture
            CL_CAP_N.store(0, Ordering::Relaxed); // restart the per-commutation ZC log
            CAPTURE_FRAMES.store(frames, Ordering::Relaxed);
            CAPTURE_TICKS_TARGET.store(ticks, Ordering::Relaxed);
            CAPTURE_TICKS = 0;
            restart_capture_dma(CAPTURE_BUF_ADDR.load(Ordering::Relaxed));
            restart_capture_dma1(CAPTURE_BUF1_ADDR.load(Ordering::Relaxed));
            CAPTURE_WAIT_ZERO = false;
            CAPTURE_ACTIVE = true;
            capture_started = true;
        }

        if CAPTURE_ACTIVE && !capture_started {
            CAPTURE_TICKS += 1;
            if CAPTURE_TICKS >= CAPTURE_TICKS_TARGET.load(Ordering::Relaxed) {
                // Flip the DMA onto the alt buffer so ADC capture continues with NO gap
                // while main slowly dumps the just-frozen buffer over UART (UART is far
                // slower than the ADC->DMA rate). RUNNING stays set: commutation lives
                // here in the ISR, so the motor keeps spinning through the blocking dump.
                restart_capture_dma(CAPTURE_BUF_ALT.load(Ordering::Relaxed));
                restart_capture_dma1(CAPTURE_BUF1_ALT.load(Ordering::Relaxed));
                CAPTURE_ACTIVE = false;
                CAPTURE_DONE.store(true, Ordering::Relaxed);
            }
        }

        // Per-commutation telemetry during a capture: log (ended sector, ZC %, coasted)
        // so the host can compare the loop's behaviour to the offline oracle (D2/G3).
        // The frame dump + this log + the alpha/period_est debug fields are how the loop
        // stays observable while it steers.
        if step.commutate && CAPTURE_ACTIVE {
            let prev = ((step.sector + 5) % 6) as u32; // the sector that just ended
            let n = CL_CAP_N.load(Ordering::Relaxed) as usize;
            if n < CL_MAXCOMM {
                // zc= is the honest in-window SIGN-CHANGE crossing (cl.sc_zc()), captured every
                // commutation independent of what drove it -- the 2/6 measurement. 255 = none.
                let zcb = match cl.sc_zc() {
                    Some(t) => {
                        let p = (t / step.period_est.max(1.0) * 100.0) as i32;
                        if (0..=100).contains(&p) {
                            p as u32
                        } else {
                            255
                        }
                    }
                    None => 255,
                };
                let co = u32::from(step.coasted);
                CL_CAP[n].store((prev << 16) | (zcb << 8) | co, Ordering::Relaxed);
                CL_CAP_LF[n].store(CL_LAST_LF, Ordering::Relaxed);
                CL_CAP_BND[n].store(CAPTURE_TICKS, Ordering::Relaxed); // capture-frame of this commutation
                // Observe-only harmonic crossing for the just-ended sector, as % of window.
                let harm = match cl.harm_zc() {
                    Some(t) => (t / step.period_est.max(1.0) * 100.0) as i32,
                    None => 9999,
                };
                CL_CAP_HARM[n].store(harm, Ordering::Relaxed);
                CL_CAP_N.store((n + 1) as u32, Ordering::Relaxed);
            }
            CL_LAST_ZC = 255; // reset for the next sector
            CL_LAST_LF = 9999;
        }
    }

    let isr_dt = cortex_m::peripheral::DWT::cycle_count().wrapping_sub(isr_t0);
    DBG_ISR_CYC.fetch_max(isr_dt, Ordering::Relaxed);
}
