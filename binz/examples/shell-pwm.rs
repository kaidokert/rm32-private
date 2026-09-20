//! Bench shell (see shell.rs) + raw-TIM1 3-phase sine drive campaign.
//! TIM1 generates complementary PWM at 10 kHz with dead-time on the native
//! PA8/PA9/PA10 + PA7/PB0/PB1 pin set. Duty is runtime-selectable from
//! 0.1..10% (default target 6%), sine-modulated at 120 degree offsets. UART is drained
//! every loop pass so `off` stays responsive; a 5 s energized timeout is hard.
//! All wired ADC and comparator feedback is retained at 1 kHz for the final
//! 256 ms of an attempt. Then all gates and ENABLE are cleared before a 2 kHz,
//! 250 ms high-impedance BEMF coast capture. Quiet summary by default;
//! `cap1` arms a one-shot full dump for a host fixture, `cap0` cancels it.
//!
//! Extra commands (on top of shell.rs):
//!   sine       toggle the sine drive (auto-sets en=1). Off = coast (gates low).
//!   sf <hz>    set electrical frequency (default 10 Hz)
//!   run<hz>    1% ALIGN -> 100Hz/7% CATCH -> RAMP -> target HOLD -> COAST
//! TIM1 channels map C/B/A to CH1/CH2/CH3 respectively, preserving the wire map.
//! Run: cargo run --release --example shell-pwm

#![no_std]
#![no_main]

use core::fmt::Write;
use core::panic::PanicInfo;
use cortex_m::peripheral::syst::SystClkSource;
use cortex_m_rt::entry;
use rtt_target::{rprintln, rtt_init_print};
use stm32g0xx_hal::prelude::*;
use stm32g0xx_hal::rcc::{Config, RccExt};
use stm32g0xx_hal::serial::BasicConfig;
use stm32g0xx_hal::stm32;

#[path = "support/bus_foldback.rs"]
mod bus_foldback;
#[cfg(all(feature = "bench-fast-bus-sag", feature = "bench-driver-fault-probe"))]
compile_error!("fast bus-sag stop cannot coexist with the driver-fault probe's average-guard bypass");
#[cfg(all(feature = "bench-running-revisit-off35", feature = "bench-running-revisit-off50"))]
compile_error!("select one running-level-revisit duty cutoff for a causal A/B");
#[cfg(any(
    all(feature = "bench-running-revisit-off35", feature = "bench-running-revisit-off48"),
    all(feature = "bench-running-revisit-off48", feature = "bench-running-revisit-off50")
))]
compile_error!("select one running-level-revisit duty cutoff for a causal A/B");
#[cfg(all(feature = "bench-fast-bus-sag", feature = "bench-current-report-only"))]
compile_error!("fast bus-sag campaign images must retain active current protection");
#[cfg(feature = "bench-capture-filter")]
#[path = "support/capture_filter.rs"]
mod capture_filter;
#[cfg(feature = "bench-pwm-roles")]
#[path = "support/carrier_profile.rs"]
mod carrier_profile;
#[path = "support/com_timer.rs"]
mod com_timer;
#[path = "support/com_timing.rs"]
mod com_timing;
#[path = "support/core_bench.rs"]
mod core_bench;
#[path = "support/core_state.rs"]
mod core_state;
#[cfg(feature = "bench-fast-sag-causal")]
#[path = "support/fast_sag_causal.rs"]
mod fast_sag_causal;
#[cfg(feature = "bench-rate-census")]
#[path = "support/rate_census.rs"]
mod rate_census;
#[cfg(feature = "bench-current-foldback-policy")]
#[path = "support/current_foldback.rs"]
mod current_foldback;
#[path = "support/detector.rs"]
mod detector;
#[cfg(feature = "bench-dma-peer")]
#[path = "support/dma_priority_probe.rs"]
mod dma_priority_probe;
#[cfg(feature = "bench-pwm-roles")]
#[path = "support/duty_envelope.rs"]
mod duty_envelope;
#[path = "support/duty_split.rs"]
mod duty_split;
#[cfg(feature = "bench-filter-latency")]
#[path = "support/filter_latency.rs"]
mod filter_latency;
#[cfg(feature = "bench-filter-source")]
#[path = "support/filtered_irq_check.rs"]
mod filtered_irq_check;
#[cfg(feature = "bench-filter-source")]
#[path = "support/filtered_irq_hw.rs"]
mod filtered_irq_hw;
#[cfg(feature = "bench-filter-source")]
#[path = "support/filtered_irq_source.rs"]
mod filtered_irq_source;
#[cfg(any(
    feature = "bench-bemf-level-revisit",
    feature = "bench-running-level-revisit"
))]
#[path = "support/level_revisit.rs"]
mod level_revisit;
#[cfg(feature = "bench-live-control")]
#[path = "support/live_command.rs"]
mod live_command;
#[cfg(any(feature = "bench-live-duty-check", feature = "bench-live-control"))]
#[path = "support/live_duty.rs"]
mod live_duty;
#[cfg(any(feature = "bench-live-duty-check", feature = "bench-live-control"))]
#[path = "support/live_duty_hw.rs"]
mod live_duty_hw;
#[cfg(feature = "bench-live-control")]
#[path = "support/live_reply.rs"]
mod live_reply;
#[path = "support/observation.rs"]
mod observation;
#[cfg(feature = "bench-pwm-roles")]
#[path = "support/phase_gpio_plan.rs"]
mod phase_gpio_plan;
#[path = "support/phase_direction.rs"]
mod phase_direction;
#[cfg(feature = "bench-pwm-roles")]
#[path = "support/phase_role_live.rs"]
mod phase_role_live;
#[cfg(feature = "bench-pwm-roles")]
#[path = "support/phase_role_sequence.rs"]
mod phase_role_sequence;
#[cfg(feature = "bench-com-peer")]
#[path = "support/priority_probe.rs"]
mod priority_probe;
#[path = "support/rolling_current.rs"]
mod rolling_current;
#[cfg(feature = "bench-revisit-origin")]
#[path = "support/revisit_origin.rs"]
mod revisit_origin;
#[cfg(feature = "bench-reverse-advance22-high")]
#[path = "support/reverse_advance.rs"]
mod reverse_advance;
#[cfg(feature = "bench-reverse-advance24-override")]
#[path = "support/reverse_advance24.rs"]
mod reverse_advance24;
#[cfg(all(
    feature = "bench-seed-timing-reanchor",
    not(feature = "bench-live-control")
))]
#[path = "support/seed_timing_check.rs"]
mod seed_timing_check;
#[path = "support/sixstep.rs"]
mod sixstep;
#[path = "support/snapshot.rs"]
mod snapshot;
#[cfg(all(feature = "bench-filter-source", feature = "bench-filter-observe"))]
compile_error!("TIM2 capture observer must not consume filtered IRQ source flags");
#[cfg(feature = "bench-capture-filter")]
#[path = "support/capture_filter_check.rs"]
mod capture_filter_check;
#[cfg(feature = "bench-comp-paths")]
#[path = "support/comp_path_live.rs"]
mod comp_path_live;
#[cfg(feature = "bench-comp-paths")]
#[path = "support/comp_paths.rs"]
mod comp_paths;
#[cfg(feature = "bench-capture-filter")]
#[path = "support/filter_epoch.rs"]
mod filter_epoch;
#[cfg(feature = "bench-capture-filter")]
#[path = "support/filter_observe.rs"]
mod filter_observe;
#[cfg(feature = "bench-qualification-direct")]
#[path = "support/qualification_counts.rs"]
mod qualification_counts;
#[cfg(feature = "bench-qualification-direct")]
#[path = "support/qualification_direct_live.rs"]
mod qualification_direct_live;
#[cfg(feature = "bench-qualification-event")]
#[path = "support/qualification_event.rs"]
mod qualification_event;
#[cfg(feature = "bench-filter-raw")]
#[path = "support/raw_capture.rs"]
mod raw_capture;
#[cfg(all(feature = "bench-qualification-direct", feature = "bench-comp-paths"))]
compile_error!("Direct qualification probe excludes the rejected Scope collector");
#[cfg(feature = "bench-comp-decisions")]
#[path = "support/comp_decision_tail.rs"]
mod comp_decision_tail;
#[cfg(feature = "bench-qualification-window")]
#[path = "support/qualification_live.rs"]
mod qualification_live;
#[cfg(feature = "bench-qualification-sparse")]
#[path = "support/qualification_sparse.rs"]
mod qualification_sparse;
#[cfg(feature = "bench-qualification-sparse")]
#[path = "support/qualification_sparse_live.rs"]
mod qualification_sparse_live;
#[cfg(feature = "bench-qualification-window")]
#[path = "support/qualification_window.rs"]
mod qualification_window;
#[cfg(all(
    feature = "bench-qualification-sparse",
    feature = "bench-qualification-window"
))]
compile_error!("qualify sparse and dense observers separately");
#[path = "support/accepted_timing.rs"]
mod accepted_timing;
#[path = "support/campaign.rs"]
mod campaign;
#[path = "support/comp_input.rs"]
mod comp_input;
#[cfg(feature = "bench-driven-entry")]
#[path = "support/driven_guard.rs"]
mod driven_guard;
#[cfg(feature = "bench-driven-irq")]
#[path = "support/driven_irq_core.rs"]
mod driven_irq_core;
#[cfg(feature = "bench-driven-irq")]
#[path = "support/driven_irq_live.rs"]
mod driven_irq_live;
#[cfg(feature = "bench-driven-entry")]
#[path = "support/driven_observer.rs"]
mod driven_observer;
#[cfg(feature = "bench-driven-entry")]
#[path = "support/driven_probe.rs"]
mod driven_probe;
#[cfg(feature = "bench-driven-power")]
#[path = "support/driven_run.rs"]
mod driven_run;
#[cfg(feature = "bench-driven-irq")]
#[path = "support/driven_seed.rs"]
mod driven_seed;
#[path = "support/event_watch.rs"]
mod event_watch;
#[path = "support/powered_guard.rs"]
mod powered_guard;
#[cfg(feature = "bench-driven-power")]
#[path = "support/pwm_sample_dma.rs"]
mod pwm_sample_dma;
#[cfg(all(
    feature = "bench-driven-power",
    feature = "bench-dma-feedback",
    not(feature = "bench-driven-dma")
))]
compile_error!("sequential driven/ADC TIM3 reuse requires bench-driven-dma");
#[path = "support/adc_occupancy.rs"]
mod adc_occupancy;
#[cfg(feature = "bench-adc-phase")]
#[path = "support/adc_phase_dma.rs"]
mod adc_phase_dma;
#[cfg(feature = "bench-dma-feedback")]
#[path = "support/adc_stream.rs"]
mod adc_stream;
#[path = "support/adc_trigger_probe.rs"]
mod adc_trigger_probe;
#[cfg(feature = "bench-average-current")]
#[path = "support/average_current_live.rs"]
mod average_current_live;
#[cfg(feature = "bench-current-epoch")]
#[path = "support/calibration_epoch.rs"]
mod calibration_epoch;
#[cfg(feature = "bench-current-epoch")]
#[path = "support/calibration_live.rs"]
mod calibration_live;
#[cfg(feature = "bench-dma-feedback")]
#[path = "support/current_sums.rs"]
mod current_sums;
#[path = "support/dma_snapshot.rs"]
mod dma_snapshot;
#[path = "support/event_stats.rs"]
mod event_stats;
#[path = "support/event_tail.rs"]
mod event_tail;
#[path = "support/event_timeline.rs"]
mod event_timeline;
#[path = "support/flying_acquire.rs"]
mod flying_acquire;
#[path = "support/flying_bench.rs"]
mod flying_bench;
#[path = "support/irq_dispatch.rs"]
mod irq_dispatch;
#[path = "support/normal_restart.rs"]
mod normal_restart;
#[path = "support/powered_timer.rs"]
mod powered_timer;
#[cfg(feature = "bench-current-baseline")]
#[path = "support/prestart_baseline.rs"]
mod prestart_baseline;
#[path = "support/sampled_clock.rs"]
mod sampled_clock;
#[cfg(feature = "bench-dma-feedback")]
#[path = "support/scan_queue.rs"]
mod scan_queue;
#[path = "support/segment_archive.rs"]
mod segment_archive;
#[path = "support/stack_probe.rs"]
mod stack_probe;
#[path = "support/uart_dma.rs"]
mod uart_dma;
#[path = "support/wave_timer.rs"]
mod wave_timer;
#[path = "support/zero_stats.rs"]
mod zero_stats;
#[interrupt]
fn TIM6_DAC_LPTIM1() {
    #[cfg(feature = "bench-cpu-timing")]
    let _cpu = cpu_meter::enter::<1>();
    #[cfg(feature = "bench-cpu-sparse")]
    let _cpu_sparse = cpu_sparse::enter::<0>();
    #[cfg(feature = "bench-driven-power")]
    if driven_run::owns() {
        driven_run::tick();
        return;
    }
    #[cfg(feature = "bench-driven-entry")]
    if driven_probe::owns() {
        driven_probe::tick();
        return;
    }
    if powered_timer::owns() {
        powered_timer::interrupt();
    } else {
        wave_timer::interrupt();
    }
}
#[cfg(feature = "bench-driven-entry")]
#[interrupt]
fn TIM3() {
    #[cfg(feature = "bench-cpu-timing")]
    let _cpu = cpu_meter::enter::<4>();
    #[cfg(feature = "bench-driven-power")]
    if driven_run::owns() {
        driven_run::sector();
        return;
    }
    driven_probe::sector();
}
#[interrupt]
fn TIM7_LPTIM2() {
    #[cfg(feature = "bench-cpu-timing")]
    let _cpu = cpu_meter::enter::<5>();
    core_bench::polling_interrupt();
}

use stm32::interrupt;
#[cfg(feature = "bench-filter-source")]
#[interrupt]
fn TIM2() {
    #[cfg(feature = "bench-com-peer")]
    if priority_probe::comp() {
        return;
    }
    #[cfg(feature = "bench-filter-control")]
    if comp_input::filtered() && core_bench::real_irq_active() {
        #[cfg(feature = "bench-cpu-timing")]
        let _cpu = cpu_meter::enter::<2>();
        core_bench::comp_interrupt();
        return;
    }
    filtered_irq_check::interrupt();
}
#[cfg(feature = "bench-dma-feedback")]
#[interrupt]
fn DMA1_CHANNEL1() {
    #[cfg(feature = "bench-dma-peer")]
    if dma_priority_probe::dma() {
        return;
    }
    #[cfg(feature = "bench-cpu-timing")]
    let _cpu = cpu_meter::enter::<6>();
    #[cfg(feature = "bench-cpu-sparse")]
    let _cpu_sparse = cpu_sparse::enter::<3>();
    adc_stream::interrupt();
}
#[interrupt]
fn ADC_COMP() {
    #[cfg(feature = "bench-dma-peer")]
    if dma_priority_probe::comp() {
        return;
    }
    #[cfg(feature = "bench-filter-control")]
    if comp_input::filtered() {
        // A stale legacy vector must not masquerade as a TIM2 capture IRQ.
        cortex_m::peripheral::NVIC::mask(stm32::Interrupt::ADC_COMP);
        cortex_m::peripheral::NVIC::unpend(stm32::Interrupt::ADC_COMP);
        return;
    }
    #[cfg(feature = "bench-cpu-timing")]
    let _cpu = cpu_meter::enter::<2>();
    #[cfg(feature = "bench-cpu-sparse")]
    let _cpu_sparse = cpu_sparse::enter::<1>();
    #[cfg(feature = "bench-driven-irq")]
    if driven_run::owns() {
        driven_run::comp_irq();
        return;
    }
    if core_bench::real_irq_active() {
        core_bench::comp_interrupt();
    } else {
        comp_input::interrupt();
    }
}
#[interrupt]
fn TIM16() {
    #[cfg(feature = "bench-com-peer")]
    if priority_probe::com() {
        return;
    }
    #[cfg(feature = "bench-cpu-timing")]
    let _cpu = cpu_meter::enter::<3>();
    #[cfg(feature = "bench-cpu-sparse")]
    let _cpu_sparse = cpu_sparse::enter::<2>();
    if core_bench::active() {
        core_bench::interrupt();
    } else {
        com_timer::interrupt();
    }
}

#[path = "support/atomic_check.rs"]
mod atomic_check;
#[cfg(feature = "bench-cpu-timing")]
#[path = "support/cpu_meter.rs"]
mod cpu_meter;
#[cfg(feature = "bench-cpu-sparse")]
#[path = "support/cpu_sparse.rs"]
mod cpu_sparse;
#[cfg(all(feature = "bench-cpu-sparse", feature = "bench-cpu-timing"))]
compile_error!("sparse CPU probe and aggregate CPU meter are mutually exclusive");
#[cfg(all(feature = "bench-cpu-timing", not(feature = "bench-cpu-union")))]
#[path = "support/irq_accounting.rs"]
mod irq_accounting;
#[cfg(feature = "bench-cpu-union")]
#[path = "support/irq_union.rs"]
mod irq_accounting;
#[cfg(feature = "bench-scheduling-tail")]
#[path = "support/scheduling_tail.rs"]
mod scheduling_tail;

// Caller must keep ENABLE low for preflight, or own runtime guards for drive.
// Break before make; comparator mux matches rm32's C,A,B,C,A,B convention.
fn sixstep_apply(step: u8, duty: u32) -> sixstep::Plan {
    gates_off();
    sixstep_write(step, duty)
}
// Timer-owned commutation; clears bridge without cancelling its scheduler.
fn sixstep_write(step: u8, duty: u32) -> sixstep::Plan {
    let period = unsafe { (*stm32::TIM1::ptr()).arr().read().bits() + 1 };
    let p = sixstep::plan_with_period(phase_direction::physical_step(step), duty, period).unwrap();
    bridge_clear();
    let tim = unsafe { &*stm32::TIM1::ptr() };
    unsafe {
        tim.ccmr1_output().write(|w| w.bits(p.ccmr1));
        tim.ccmr2_output().write(|w| w.bits(p.ccmr2));
        tim.ccer().write(|w| w.bits(p.ccer));
        tim.ccr1().write(|w| w.bits(p.ccr[0]));
        tim.ccr2().write(|w| w.bits(p.ccr[1]));
        tim.ccr3().write(|w| w.bits(p.ccr[2]));
        tim.egr().write(|w| w.bits(1));
        let old = core::ptr::read_volatile(COMP2_CSR);
        core::ptr::write_volatile(
            COMP2_CSR,
            (old & !(0xF << 4 | 0x3 << 8)) | ((6 + p.floating as u32) << 4) | (2 << 8),
        );
    }
    pwm_moe(true);
    p
}

const VREFINT_CAL_ADDR: u32 = 0x1FFF_75AA;
const COMP2_CSR: *mut u32 = 0x4001_0204 as *mut u32;
const CARRIER_HZ: u32 = if cfg!(feature = "bench-startup-20k") {
    20_000
} else {
    10_000
};
const CONTROL_HZ: u32 = 1_000;
const PWM_ARR: u32 = 64_000_000 / CARRIER_HZ - 1;
const PWM_PERIOD_US: u32 = 1_000_000 / CARRIER_HZ;
// The reverse-motor diagnostic may need more torque before the coast seed.
// This affects only the shell's forced-waveform/startup limit; the protected
// live-duty envelope remains separately bounded by duty_envelope::MAX.
const MAX_DUTY_TENTHS: u32 = if cfg!(feature = "bench-reverse-startup-15") {
    150
} else {
    100
};
const DEFAULT_DUTY_TENTHS: u32 = 60; // 6.0%, operator-established catch region
// Measured against gate/VSEN phase identity, LAB_REPORT Entry 016.
// Raw diagnostics retain physical ADC numbering; logical drive captures use this.
const CURRENT_ADC: [u8; 3] = [4, 1, 0];
const ALIGN_DUTY_TENTHS: u32 = 10; // 1.0%, proven electrically clean startup amplitude
const CATCH_DUTY_TENTHS: u32 = 70; // bench-proven start at 100 electrical Hz
const ALIGN_TICKS: u32 = 20;
const START_TICKS: u32 = if cfg!(feature = "bench-startup-fast") {
    100
} else {
    980
};
// The autonomous staircase reproduces the successful host path: establish
// rotation at50eHz, then step60..200. Its former100eHz initial jump was not
// host parity and intermittently stalled into the1A average-current guard.
const START_HZ: u32 = if cfg!(feature = "bench-startup-staircase")
    && !cfg!(feature = "bench-reverse-flat-start")
{
    50
} else {
    100
};
const RAMP_TICKS: u32 = if cfg!(feature = "bench-startup-fast") {
    300
} else {
    2_000
};
const HOLD_TICKS: u32 = if cfg!(feature = "bench-startup-fast") {
    400
} else {
    2_000
};
const RUN_TICKS: u32 = ALIGN_TICKS + START_TICKS + RAMP_TICKS + HOLD_TICKS;
// Leave the same 300 ms campaign tail for the under-drive handoff.
const HANDOFF_US: u32 = (RUN_TICKS - 300) * 1_000;
const CAPTURE_DIV: u32 = 1; // every 1 kHz sine/control update
const CAPTURE_HZ: u32 = CONTROL_HZ / CAPTURE_DIV;
const CAPTURE_N: usize = 256; // rolling final 256 ms; RAM shared budget with dense observer
const COAST_HZ: u32 = if cfg!(feature = "bench-fast-coast") {
    8_000
} else {
    2_000
};
const COAST_PERIOD_US: u16 = (1_000_000 / COAST_HZ) as u16;
const COAST_N: usize = 500; // nominal250ms, or62.5ms fast-coast; actual timestamps retained

// Raw capture record. Keeping ADC values unscaled matches minz's proven dump
// pattern: retain integer wire truth, carry VREFINT, convert on the host.
#[repr(C)]
#[derive(Clone, Copy)]
struct Capture {
    tick: u16,
    freq_chz: u16,
    ia: u16,
    ib: u16,
    ic: u16,
    vsenc: u16,
    neutral: u16,
    vbus: u16,
    vref: u16,
    theta: u8,
    flags: u8, // bit0 nFAULT high; bits1/2/3 comparator A/B/C high
    on_a: u8,
    on_b: u8,
    on_c: u8,
    stage: u8, // 0=fixed, 1=align, 2=start, 3=ramp, 4=hold
}

const EMPTY_CAPTURE: Capture = Capture {
    tick: 0,
    freq_chz: 0,
    ia: 0,
    ib: 0,
    ic: 0,
    vsenc: 0,
    neutral: 0,
    vbus: 0,
    vref: 0,
    theta: 0,
    flags: 0,
    on_a: 0,
    on_b: 0,
    on_c: 0,
    stage: 0,
};

static mut CAPTURE: [Capture; CAPTURE_N] = [EMPTY_CAPTURE; CAPTURE_N];

#[repr(C)]
#[derive(Clone, Copy)]
struct CoastCapture {
    tick: u16,
    vsenc: u16,
    neutral: u16,
    vbus: u16,
    vref: u16,
    flags: u8, // bit0 nFAULT high; bits1/2/3 comparator A/B/C high
    _pad: u8,
}

const EMPTY_COAST: CoastCapture = CoastCapture {
    tick: 0,
    vsenc: 0,
    neutral: 0,
    vbus: 0,
    vref: 0,
    flags: 0,
    _pad: 0,
};

static mut COAST_CAPTURE: [CoastCapture; COAST_N] = [EMPTY_COAST; COAST_N];
static mut COAST_TIME_US: [u32; COAST_N] = [0; COAST_N];

// sine LUT: 0..255, one electrical period (sine offset to 128 mid).
#[path = "support/sine_scale.rs"]
mod sine_scale;
#[path = "support/sine_table.rs"]
mod sine_table;
use sine_table::SINE_LUT;
#[path = "support/phase_schedule.rs"]
mod phase_schedule;

unsafe fn adc_read(ch: u8) -> u16 {
    let adc = &*stm32::ADC::ptr();
    adc.isr().write(|w| w.bits(1 << 13));
    adc.chselr0().write(|w| w.bits(1 << ch));
    while adc.isr().read().bits() & (1 << 13) == 0 {}
    adc.cr().modify(|r, w| w.bits(r.bits() | (1 << 2)));
    while adc.isr().read().bits() & (1 << 2) == 0 {}
    adc.dr().read().bits() as u16
}

// Two-millisecond, 6% PWM phase-pair diagnostic. High/low identities must
// already be qualified. OFF-window samples see recirculating winding current.
fn pair_probe<W: Write>(serial: &mut W, pair: usize) {
    let (source, sink) = [(0usize, 1usize), (1, 0), (0, 2), (2, 0), (1, 2), (2, 1)][pair];
    let tim = unsafe { &*stm32::TIM1::ptr() };
    gates_off();
    set_pin(3, 1, true);
    let wake = t17();
    while t17().wrapping_sub(wake) < 2_000 {}
    let mut baseline = [0i32; 3];
    for _ in 0..16 {
        for (i, ch) in [0u8, 1, 4].iter().enumerate() {
            baseline[i] += unsafe { adc_read(*ch) } as i32;
        }
    }
    for value in &mut baseline {
        *value /= 16;
    }
    let bus0 = unsafe { adc_read(6) };
    let mut reason = if !get_idr(1, 14) {
        2
    } else if bus0 < 800 {
        5
    } else {
        0
    };
    let mut mode = [0x40u32; 3]; // forced inactive for sink/floating
    mode[2 - source] = 0x68; // source PWM1 preload
    unsafe {
        tim.ccmr1_output().write(|w| w.bits(mode[0] | mode[1] << 8));
        tim.ccmr2_output().write(|w| w.bits(mode[2]));
        tim.ccer()
            .write(|w| w.bits((5 << ((2 - source) * 4)) | (5 << ((2 - sink) * 4))));
        tim.ccr1()
            .write(|w| w.bits(if source == 2 { 384 } else { 0 }));
        tim.ccr2()
            .write(|w| w.bits(if source == 1 { 384 } else { 0 }));
        tim.ccr3()
            .write(|w| w.bits(if source == 0 { 384 } else { 0 }));
        tim.egr().write(|w| w.bits(1));
    }
    let mut count = 0i32;
    let mut sums = [0i32; 3];
    let mut minimum = [4095i32; 3];
    let mut maximum = [-4095i32; 3];
    let mut bus_min = bus0;
    let started = t17();
    if reason == 0 {
        pwm_moe(true);
    }
    let mut last_sample = t17().wrapping_sub(100);
    while reason == 0 && count < 16 && t17().wrapping_sub(started) < 1_900 {
        if !get_idr(1, 14) {
            reason = 2;
            break;
        }
        // At most one scan per cycle, beginning 25-35 us into OFF interval.
        let cnt = tim.cnt().read().bits();
        if t17().wrapping_sub(last_sample) < 90 || !(1600..2240).contains(&cnt) {
            continue;
        }
        last_sample = t17();
        for j in 0..3 {
            let i = (j + count as usize) % 3;
            let value = unsafe { adc_read([0u8, 1, 4][i]) } as i32 - baseline[i];
            sums[i] += value;
            minimum[i] = minimum[i].min(value);
            maximum[i] = maximum[i].max(value);
            if value.abs() > 800 {
                reason = 4;
            }
        }
        let bus = unsafe { adc_read(6) };
        bus_min = bus_min.min(bus);
        if (bus as u32) * 10 < (bus0 as u32) * 7 {
            reason = 5;
        }
        count += 1;
    }
    gates_off();
    set_pin(3, 1, false);
    let elapsed = t17().wrapping_sub(started);
    unsafe {
        tim.ccmr1_output().write(|w| w.bits(0x6868));
        tim.ccmr2_output().write(|w| w.bits(0x68));
        tim.ccer().write(|w| w.bits(0x555));
        tim.egr().write(|w| w.bits(1));
    }
    let _ = writeln!(
        serial,
        "PAIR id={} source={} sink={} duty=60 n={} us={} reason={} bus0={} busmin={} en=0 moe=0",
        pair, source, sink, count, elapsed, reason, bus0, bus_min
    );
    for i in 0..3 {
        let _ = writeln!(
            serial,
            "PAIR adc={} zero={} delta_mean={} min={} max={}",
            [0, 1, 4][i],
            baseline[i],
            sums[i] / count.max(1),
            minimum[i],
            maximum[i]
        );
    }
}

fn set_pin(port: u8, bit: u32, on: bool) {
    #[cfg(feature = "bench-current-epoch")]
    if port == 3 && bit == 1 {
        calibration_live::write_enable(on);
        return;
    }
    let v = if on { 1 << bit } else { 1 << (bit + 16) };
    unsafe {
        match port {
            0 => (*stm32::GPIOA::ptr()).bsrr().write(|w| w.bits(v)),
            1 => (*stm32::GPIOB::ptr()).bsrr().write(|w| w.bits(v)),
            2 => (*stm32::GPIOC::ptr()).bsrr().write(|w| w.bits(v)),
            _ => (*stm32::GPIOD::ptr()).bsrr().write(|w| w.bits(v)),
        };
    }
}

// Fixed-order regular ADC sequence: channels 2 then 3. Caller selects sample
// duration while idle. No second channel-selection handshake between samples.
fn adc_bemf_pair() -> (u16, u16) {
    adc_bemf_prepare();
    adc_bemf_convert()
}
fn adc_bemf_prepare() {
    adc_pair_prepare((1 << 2) | (1 << 3));
}
fn adc_pair_prepare(mask: u32) {
    let adc = unsafe { &*stm32::ADC::ptr() };
    let started = t17();
    unsafe {
        adc.isr()
            .write(|w| w.bits((1 << 13) | (1 << 4) | (1 << 3) | (1 << 2)));
        adc.chselr0().write(|w| w.bits(mask));
    }
    while adc.isr().read().bits() & (1 << 13) == 0 {
        if t17().wrapping_sub(started) > 100 {
            panic!("ADC channel timeout");
        }
    }
}
fn adc_bemf_convert() -> (u16, u16) {
    let adc = unsafe { &*stm32::ADC::ptr() };
    let started = t17();
    unsafe {
        adc.cr().modify(|r, w| w.bits(r.bits() | (1 << 2)));
    }
    let mut result = [0u16; 2];
    for value in &mut result {
        while adc.isr().read().bits() & (1 << 2) == 0 {
            if t17().wrapping_sub(started) > 100 {
                panic!("ADC pair timeout");
            }
        }
        *value = adc.dr().read().bits() as u16;
    }
    if adc.isr().read().bits() & (1 << 4) != 0 {
        panic!("ADC pair overrun");
    }
    (result[0], result[1])
}
fn adc_single_convert() -> u16 {
    let adc = unsafe { &*stm32::ADC::ptr() };
    let started = t17();
    unsafe {
        adc.cr().modify(|r, w| w.bits(r.bits() | (1 << 2)));
    }
    while adc.isr().read().bits() & (1 << 2) == 0 {
        if t17().wrapping_sub(started) > 100 {
            panic!("ADC single timeout");
        }
    }
    let result = adc.dr().read().bits() as u16;
    if adc.isr().read().bits() & (1 << 4) != 0 {
        panic!("ADC single overrun");
    }
    result
}
fn bsrr_a(set: u32, clr: u32) {
    unsafe {
        (*stm32::GPIOA::ptr())
            .bsrr()
            .write(|w| w.bits(set | (clr << 16)));
    }
}
fn bsrr_b(set: u32, clr: u32) {
    unsafe {
        (*stm32::GPIOB::ptr())
            .bsrr()
            .write(|w| w.bits(set | (clr << 16)));
    }
}
fn get_odr(port: u8, bit: u32) -> bool {
    unsafe {
        let r = match port {
            0 => (*stm32::GPIOA::ptr()).odr().read().bits(),
            1 => (*stm32::GPIOB::ptr()).odr().read().bits(),
            2 => (*stm32::GPIOC::ptr()).odr().read().bits(),
            _ => (*stm32::GPIOD::ptr()).odr().read().bits(),
        };
        r & (1 << bit) != 0
    }
}
fn get_idr(port: u8, bit: u32) -> bool {
    unsafe {
        let r = match port {
            0 => (*stm32::GPIOA::ptr()).idr().read().bits(),
            1 => (*stm32::GPIOB::ptr()).idr().read().bits(),
            2 => (*stm32::GPIOC::ptr()).idr().read().bits(),
            _ => (*stm32::GPIOD::ptr()).idr().read().bits(),
        };
        r & (1 << bit) != 0
    }
}
fn pin_lookup(name: &[u8]) -> Option<(u8, u32)> {
    match name {
        b"led" => Some((1, 5)),
        b"ld4" => Some((0, 5)),
        _ => None,
    }
}
const ALL_PINS: [(&str, u8, u32); 11] = [
    ("ah", 0, 10),
    ("bh", 0, 9),
    ("ch", 0, 8),
    ("al", 1, 1),
    ("bl", 1, 0),
    ("cl", 0, 7),
    ("led", 1, 5),
    ("ld4", 0, 5),
    ("en", 3, 1),
    ("nflt", 1, 14),
    ("btn", 2, 13),
];
fn comp_read(code: u32) -> bool {
    comp_read_settle(code, false)
}
#[inline(never)]
fn comp_read_settle(code: u32, fast: bool) -> bool {
    unsafe {
        let v = core::ptr::read_volatile(COMP2_CSR);
        core::ptr::write_volatile(
            COMP2_CSR,
            (v & !(0xF << 4 | 0x3 << 8)) | (code << 4) | (0b10 << 8),
        );
    }
    if fast {
        let switched = t17();
        while t17().wrapping_sub(switched) < 10 {}
    } else {
        cortex_m::asm::delay(3000);
    }
    unsafe { core::ptr::read_volatile(COMP2_CSR) & (1 << 30) != 0 }
}

fn feedback_flags() -> u8 {
    (get_idr(1, 14) as u8)
        | ((comp_read(6) as u8) << 1)
        | ((comp_read(7) as u8) << 2)
        | ((comp_read(8) as u8) << 3)
}

// Last sine scan only; foreground-owned, never read by an ISR. Dump after
// safing, with cap1 only. Brackets include preemption, not just ADC aperture.
static mut CURRENT_TIMING: [[u16; 5]; 3] = [[0; 5]; 3];

fn current_read_rotated(index: usize) -> [u16; 3] {
    let mut current = [0u16; 3];
    let channels = CURRENT_ADC;
    let first = index % 3;
    for offset in 0..3 {
        let phase = (first + offset) % 3;
        let tim = unsafe { &*stm32::TIM1::ptr() };
        let ccr = tim.ccr(2 - phase).read().bits() as u16;
        let started = t17();
        let pwm_start = tim.cnt().read().bits() as u16;
        current[phase] = unsafe { adc_read(channels[phase]) };
        let pwm_end = tim.cnt().read().bits() as u16;
        let elapsed = t17().wrapping_sub(started);
        unsafe {
            CURRENT_TIMING[phase] = [pwm_start, pwm_end, elapsed, ccr, current[phase]];
        }
    }
    current
}

fn capture_sample(
    index: usize,
    tick: u32,
    freq_chz: u32,
    theta: u32,
    on: [u16; 3],
    stage: u8,
) -> Capture {
    #[cfg(feature = "bench-startup-adc")]
    {
        let raw = adc_stream::startup_sample()
            .map(|(raw, _)| raw)
            .unwrap_or([0; 5]);
        return Capture {
            tick: tick as u16,
            freq_chz: freq_chz as u16,
            ia: raw[0],
            ib: raw[1],
            ic: raw[2],
            vsenc: 0,
            neutral: 0,
            vbus: raw[3],
            vref: raw[4],
            theta: (theta >> 24) as u8,
            flags: feedback_flags(),
            on_a: on[0] as u8,
            on_b: on[1] as u8,
            on_c: on[2] as u8,
            stage,
        };
    }
    #[cfg(not(feature = "bench-startup-adc"))]
    {
        let current = current_read_rotated(index);
        let sample = Capture {
            tick: tick as u16,
            freq_chz: freq_chz as u16,
            ia: current[0],
            ib: current[1],
            ic: current[2],
            vsenc: unsafe { adc_read(2) },
            neutral: unsafe { adc_read(3) },
            vbus: unsafe { adc_read(6) },
            vref: unsafe { adc_read(13) },
            theta: (theta >> 24) as u8,
            flags: feedback_flags(),
            on_a: on[0] as u8,
            on_b: on[1] as u8,
            on_c: on[2] as u8,
            stage,
        };
        sample
    }
}
fn capture_store(index: usize, sample: Capture) {
    unsafe {
        core::ptr::addr_of_mut!(CAPTURE)
            .cast::<Capture>()
            .add(index)
            .write_volatile(sample);
    }
}

fn capture_read(index: usize) -> Capture {
    unsafe {
        core::ptr::addr_of!(CAPTURE)
            .cast::<Capture>()
            .add(index)
            .read_volatile()
    }
}

static mut COAST_ANCHOR_DELAY: u16 = 0;
// Early coast only: timestamp each real comparator read after mux settling.
// 32*3*2=192bytes; no change to legacy coast record layout or sample values.
static mut COAST_COMP_OFFSETS: [[u16; 3]; 32] = [[0; 3]; 32];
fn coast_write(index: usize, tick: u32, elapsed_us: u32) {
    #[cfg(feature = "bench-fast-coast")]
    assert!(
        !get_idr(3, 1) && powered_timer::outputs_disabled(),
        "coast outputs"
    );
    let scan_start = t17();
    if index == 0 {
        let (_, _, stopped, _) = wave_timer::stop_anchor();
        unsafe {
            COAST_ANCHOR_DELAY = t17().wrapping_sub(stopped);
        }
    }
    unsafe {
        core::ptr::addr_of_mut!(COAST_TIME_US)
            .cast::<u32>()
            .add(index)
            .write(elapsed_us);
    }
    let sample = CoastCapture {
        tick: tick as u16,
        vsenc: unsafe { adc_read(2) },
        neutral: unsafe { adc_read(3) },
        vbus: unsafe { adc_read(6) },
        vref: unsafe { adc_read(13) },
        flags: if index < 32 || cfg!(feature = "bench-fast-coast") {
            let mut flags = get_idr(1, 14) as u8;
            for phase in 0..3 {
                flags |= (comp_read_settle(6 + phase_direction::physical_phase(phase as u8) as u32, cfg!(feature = "bench-fast-coast"))
                    as u8)
                    << (phase + 1);
                if index < 32 {
                    unsafe {
                        COAST_COMP_OFFSETS[index][phase] = t17().wrapping_sub(scan_start);
                    }
                }
            }
            flags
        } else {
            feedback_flags()
        },
        _pad: 0,
    };
    unsafe {
        core::ptr::addr_of_mut!(COAST_CAPTURE)
            .cast::<CoastCapture>()
            .add(index)
            .write_volatile(sample);
    }
}

fn coast_read(index: usize) -> CoastCapture {
    unsafe {
        core::ptr::addr_of!(COAST_CAPTURE)
            .cast::<CoastCapture>()
            .add(index)
            .read_volatile()
    }
}

fn dump_capture<W: Write>(
    serial: &mut W,
    len: usize,
    head: usize,
    reason: u8,
    vcal: u32,
    stride: u32,
) {
    // Versioned Ascii85 records with CRC, emitted only after disable.
    let _ = writeln!(serial, "WIRE a85-v1");
    let _ = writeln!(serial, "IMAP ia=4 ib=1 ic=0");
    let _ = writeln!(
        serial,
        "CAPCADENCE adc_hz={} retain_every_ticks={} timestamps=control_ticks adc_fault_sample_retained=1",
        CAPTURE_HZ, stride
    );
    #[cfg(feature = "bench-average-diagnostic")]
    average_current_live::dump(serial);
    average_current_live::quality_summary(serial);
    #[cfg(feature = "bench-current-foldback-policy")]
    average_current_live::foldback_summary(serial);
    #[cfg(feature = "bench-startup-adc")]
    let _ = writeln!(
        serial,
        "EXPLOREPROTECTION pulse_stop=0 phase_rail_stop=0 phase_rail_count=1 bus_average_scans={} vref_validity_stop=1 average_stop=1 nominal_target_ma={} nominal_raw_sum={} calibrated=0",
        average_current_live::nominal::SCANS,
        average_current_live::nominal::TARGET_MA,
        average_current_live::nominal::RAW_LIMIT
    );
    #[cfg(feature = "bench-startup-adc")]
    let _ = writeln!(
        serial,
        "STARTUPADC fault={} scans={} period_us={} vsenc_valid=0 neutral_valid=0 cached_reads_not_average_samples=1",
        adc_stream::startup_reason(),
        adc_stream::completed_scans(),
        adc_stream::PERIOD_US
    );
    let adc_order = if cfg!(feature = "bench-startup-adc") {
        "DMA_ascending_0_1_4_6_13_logical_4_1_0"
    } else {
        "rotating_ABC_BCA_CAB"
    };
    let _ = writeln!(
        serial,
        "CAP n={} capacity={} head={} order=oldest_first sample_hz={} sample_point=control_tick_async_to_pwm adc_order={} vcal={} reason={} fields=tick,freq_chz,ia,ib,ic,vsenc,neutral,vbus,vref,theta,flags,on_a,on_b,on_c,stage",
        len,
        CAPTURE_N,
        head,
        CAPTURE_HZ / stride,
        adc_order,
        vcal,
        reason
    );
    let oldest = if len == CAPTURE_N { head } else { 0 };
    for logical in 0..len {
        let s = capture_read((oldest + logical) % CAPTURE_N);
        let _ = snapshot::record(
            serial,
            "D85",
            &[
                s.tick,
                s.freq_chz,
                s.ia,
                s.ib,
                s.ic,
                s.vsenc,
                s.neutral,
                s.vbus,
                s.vref,
                s.theta as u16,
                s.flags as u16,
                s.on_a as u16,
                s.on_b as u16,
                s.on_c as u16,
                s.stage as u16,
            ],
        );
    }
    let _ = writeln!(serial, "CAP END");
}

fn dump_coast<W: Write>(serial: &mut W, len: usize, vcal: u32) {
    let _ = writeln!(
        serial,
        "COAST n={} sample_hz={} sample_point=bridge_disabled vcal={} fields=tick,vsenc,neutral,vbus,vref,flags",
        len, COAST_HZ, vcal,
    );
    for index in 0..len {
        let s = coast_read(index);
        let elapsed = unsafe {
            core::ptr::addr_of!(COAST_TIME_US)
                .cast::<u32>()
                .add(index)
                .read()
        };
        let _ = snapshot::record(
            serial,
            "C85",
            &[
                s.tick,
                s.vsenc,
                s.neutral,
                s.vbus,
                s.vref,
                s.flags as u16,
                elapsed as u16,
                (elapsed >> 16) as u16,
            ],
        );
    }
    let _ = writeln!(serial, "COAST END");
}

// TIM17 1 MHz free-run timebase.
fn t17() -> u16 {
    unsafe { (*stm32::TIM17::ptr()).cnt().read().bits() as u16 }
}

// Call at least every 65 ms while energized. Idle/dump time need not be accurate;
// each run uses a fresh relative origin. No interrupt accesses this clock.
fn clock_us() -> u32 {
    static mut LAST: u16 = 0;
    static mut TOTAL: u32 = 0;
    unsafe {
        let now = t17();
        TOTAL = TOTAL.wrapping_add(now.wrapping_sub(LAST) as u32);
        LAST = now;
        TOTAL
    }
}

// Gate bits: INH A=PA10 B=PA9 C=PA8 (all GPIOA); INL A=PB1 B=PB0 (GPIOB) C=PA7 (GPIOA).
fn gates_off() {
    gates_off_inner::<true>();
}
// Only the validated sine -> driven transfer uses this. Fault paths always
// revoke ADC; the independent producer must survive this output-owner switch.
#[cfg(feature = "bench-startup-adc")]
fn gates_off_keep_adc() {
    gates_off_inner::<false>();
}
fn gates_off_inner<const REVOKE_ADC: bool>() {
    #[cfg(feature = "bench-reentry-next-edge-live")]
    flying_bench::cancel_follow();
    #[cfg(feature = "bench-driven-power")]
    pwm_sample_dma::stop();
    #[cfg(feature = "bench-driven-power")]
    driven_run::cancel();
    #[cfg(feature = "bench-driven-entry")]
    driven_probe::cancel();
    powered_timer::stop();
    wave_timer::stop();
    core_bench::live_stop();
    core_bench::polling_stop();
    bridge_clear();
    #[cfg(feature = "bench-adc-latest")]
    if REVOKE_ADC {
        adc_stream::quiesce();
    }
}
fn bridge_clear() {
    bridge_clear_inner::<false>();
}
// Only successful, gate-disabled recovery may retain a preloaded period.
// This never retains compare values, MOE, GPIO latches or role authority.
fn bridge_clear_inner<const KEEP_CARRIER: bool>() {
    unsafe {
        let tim = &*stm32::TIM1::ptr();
        tim.bdtr().modify(|r, w| w.bits(r.bits() & !(1 << 15))); // MOE=0
        tim.ccr1().write(|w| w.bits(0));
        tim.ccr2().write(|w| w.bits(0));
        tim.ccr3().write(|w| w.bits(0));
    }
    bsrr_a(0, (1 << 7) | (1 << 8) | (1 << 9) | (1 << 10));
    bsrr_b(0, (1 << 0) | (1 << 1));
    #[cfg(feature = "bench-pwm-roles")]
    phase_role_live::revoked_restore_af_inner::<KEEP_CARRIER>();
}
fn pwm_moe(on: bool) {
    unsafe {
        let tim = &*stm32::TIM1::ptr();
        tim.bdtr().modify(|r, w| {
            let bits = if on {
                r.bits() | (1 << 15)
            } else {
                r.bits() & !(1 << 15)
            };
            w.bits(bits)
        });
    }
}
// Six-step leaves channel roles configured after safing. Every sine entry
// restores all three complementary PWM channels while ENABLE and MOE are off.
fn prepare_sine() {
    gates_off();
    set_pin(3, 1, false);
    unsafe {
        let tim = &*stm32::TIM1::ptr();
        // BEMF changes ARR. Restore the sine compare scale on every restart,
        // not just boot, before UG loads the disabled timer's preloads.
        tim.arr().write(|w| w.bits(PWM_ARR));
        tim.ccmr1_output().write(|w| w.bits(0x6868));
        tim.ccmr2_output().write(|w| w.bits(0x68));
        tim.ccer().write(|w| w.bits(0x555));
        tim.egr().write(|w| w.bits(1));
        if tim.ccmr1_output().read().bits() != 0x6868
            || tim.ccmr2_output().read().bits() != 0x68
            || tim.ccer().read().bits() != 0x555
            || tim.arr().read().bits() != PWM_ARR
        {
            panic!("sine mode restore");
        }
    }
}
fn pwm_sine(duty_tenths: u32, theta: u32) -> [u16; 3] {
    let duty_tenths = duty_tenths.min(MAX_DUTY_TENTHS);
    pwm_sine_prepared(sine_scale::amplitude(PWM_ARR, duty_tenths), theta)
}
fn pwm_sine_prepared(amplitude: u32, theta: u32) -> [u16; 3] {
    const _: () = assert!(PWM_ARR == 6399 && PWM_PERIOD_US == 100);
    let mut ccr = [0u32; 3];
    let mut on_us = [0u16; 3];
    for i in 0..3 {
        let ix = (((theta >> 24) + (i as u32) * 85) & 0xFF) as usize;
        ccr[i] = sine_scale::compare(amplitude, SINE_LUT[ix]);
        on_us[i] = sine_scale::on_us_10k(ccr[i]);
    }
    unsafe {
        let tim = &*stm32::TIM1::ptr();
        // Native TIM1 pin pairing: CH3=A, CH2=B, CH1=C.
        tim.ccr3().write(|w| w.bits(ccr[phase_direction::physical_phase(0) as usize]));
        tim.ccr2().write(|w| w.bits(ccr[phase_direction::physical_phase(1) as usize]));
        tim.ccr1().write(|w| w.bits(ccr[2]));
    }
    on_us
}

// This board cannot use binz's EVLDRIVE panic safing map. Keep a local panic
// handler that clears all six DRV8304 inputs and ENABLE before halting.
#[unsafe(no_mangle)]
static PANIC_LINE: portable_atomic::AtomicU32 = portable_atomic::AtomicU32::new(0);
#[cfg(feature = "bench-host-abort-byte")]
static HOST_ABORT_OBS: portable_atomic::AtomicU32 = portable_atomic::AtomicU32::new(0);
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    cortex_m::interrupt::disable();
    gates_off();
    set_pin(3, 1, false);
    #[cfg(feature = "bench-filter-source")]
    filtered_irq_hw::stop();
    PANIC_LINE.store(
        info.location().map(|l| l.line()).unwrap_or(u32::MAX),
        portable_atomic::Ordering::Relaxed,
    );
    loop {
        cortex_m::asm::nop();
    }
}
// Boot-only breadcrumbs for silent-UART diagnosis. Read via SWD using this
// ELF's symbol address; never polled or printed during motor operation.
static mut UART_BOOT_CLOCKS: [u32; 4] = [u32::MAX; 4];
fn uart_boot_clock(index: usize) {
    unsafe {
        let value = (*stm32::RCC::ptr()).apbenr1().read().bits();
        core::ptr::addr_of_mut!(UART_BOOT_CLOCKS)
            .cast::<u32>()
            .add(index)
            .write_volatile(value);
    }
}
#[entry]
fn main() -> ! {
    stack_probe::paint();
    rtt_init_print!(rtt_target::ChannelMode::NoBlockSkip, 256);
    let dp = stm32::Peripherals::take().unwrap();
    let cp = cortex_m::Peripherals::take().unwrap();
    let mut rcc = dp.RCC.freeze(Config::pll());

    let gpioa = dp.GPIOA.split(&mut rcc);
    let gpiob = dp.GPIOB.split(&mut rcc);
    let gpioc = dp.GPIOC.split(&mut rcc);
    let gpiod = dp.GPIOD.split(&mut rcc);

    let _ = (
        gpioa.pa0.into_analog(),
        gpioa.pa1.into_analog(),
        gpioa.pa2.into_analog(),
        gpioa.pa3.into_analog(),
        gpioa.pa4.into_analog(),
        gpioa.pa6.into_analog(),
        gpiob.pb3.into_analog(),
        gpiob.pb7.into_analog(),
    );
    let _ = (
        gpiob.pb14.into_floating_input(),
        gpioc.pc13.into_floating_input(),
    );
    let _ = (
        gpioa.pa5.into_push_pull_output(),
        gpioa.pa7.into_push_pull_output(),
        gpioa.pa8.into_push_pull_output(),
        gpioa.pa9.into_push_pull_output(),
        gpioa.pa10.into_push_pull_output(),
        gpiob.pb0.into_push_pull_output(),
        gpiob.pb1.into_push_pull_output(),
        gpiob.pb5.into_push_pull_output(),
        gpiod.pd1.into_push_pull_output(),
    );
    for (p, b) in [
        (0, 5),
        (0, 7),
        (0, 8),
        (0, 9),
        (0, 10),
        (1, 0),
        (1, 1),
        (1, 5),
        (3, 1),
    ] {
        set_pin(p, b, false);
    }

    // Put the six native TIM1 pins into AF2 only after their GPIO latches are
    // low, then configure the proven raw-register complementary PWM pattern.
    unsafe {
        let rcc_raw = &*stm32::RCC::ptr();
        rcc_raw
            .apbenr2()
            .modify(|r, w| w.bits(r.bits() | (1 << 11))); // TIM1EN
        let pa = &*stm32::GPIOA::ptr();
        pa.moder().modify(|_, w| {
            w.moder7()
                .alternate()
                .moder8()
                .alternate()
                .moder9()
                .alternate()
                .moder10()
                .alternate()
        });
        pa.afrl().modify(|_, w| w.afr(7).af2());
        pa.afrh()
            .modify(|_, w| w.afr(0).af2().afr(1).af2().afr(2).af2());
        let pb = &*stm32::GPIOB::ptr();
        pb.moder()
            .modify(|_, w| w.moder0().alternate().moder1().alternate());
        pb.afrl().modify(|_, w| w.afr(0).af2().afr(1).af2());

        let tim = &*stm32::TIM1::ptr();
        tim.cr1().write(|w| w.bits(0));
        tim.cr2().write(|w| w.bits(0));
        tim.psc().write(|w| w.bits(0));
        tim.arr().write(|w| w.bits(PWM_ARR));
        tim.rcr().write(|w| w.bits(0));
        tim.ccmr1_output().write(|w| w.bits(0x6868)); // CH1/2 PWM1 + preload
        tim.ccmr2_output().write(|w| w.bits(0x0068)); // CH3 PWM1 + preload
        tim.ccer().write(|w| w.bits(0x0555)); // main + complementary, active high
        tim.ccr1().write(|w| w.bits(0));
        tim.ccr2().write(|w| w.bits(0));
        tim.ccr3().write(|w| w.bits(0));
        tim.bdtr().write(|w| w.bits((1 << 11) | (1 << 10) | 26)); // OSSR|OSSI|~0.4 us DT, MOE=0
        tim.egr().write(|w| w.bits(1));
        tim.sr().write(|w| w.bits(0));
        tim.cr1().write(|w| w.bits(0x81)); // ARPE|CEN
    }

    // Ensure the peripheral clock is live before HAL configuration writes.
    // Readback orders the APB enable before USART access on size/LTO builds.
    unsafe {
        let rcc_raw = &*stm32::RCC::ptr();
        rcc_raw
            .apbenr1()
            .modify(|r, w| w.bits(r.bits() | (1 << 18)));
        let _ = rcc_raw.apbenr1().read().bits();
        cortex_m::asm::dsb();
    }
    uart_boot_clock(0);
    let mut serial = dp
        .USART3
        .usart(
            (gpioc.pc10, gpioc.pc11),
            BasicConfig::default().baudrate(115_200.bps()),
            &mut rcc,
        )
        .unwrap();
    uart_boot_clock(1);

    unsafe {
        let rcc_raw = &*stm32::RCC::ptr();
        rcc_raw.apbenr2().modify(|r, w| w.bits(r.bits() | (1 << 0))); // SYSCFGEN
        rcc_raw
            .apbenr2()
            .modify(|r, w| w.bits(r.bits() | (1 << 20))); // ADCEN
        rcc_raw
            .apbenr2()
            .modify(|r, w| w.bits(r.bits() | (1 << 18))); // TIM17EN
        // TIM17 = 1 MHz free-run.
        let t = &*stm32::TIM17::ptr();
        t.psc().write(|w| w.bits(63)); // 64 MHz / 64 = 1 MHz
        t.arr().write(|w| w.bits(0xFFFF));
        t.egr().write(|w| w.bits(1)); // UG latch PSC
        t.cr1().write(|w| w.bits(1)); // CEN
        let adc = &*stm32::ADC::ptr();
        adc.cfgr2().write(|w| w.bits(0b10 << 30));
        adc.cr().write(|w| w.bits(1 << 28));
        cortex_m::asm::delay(64 * 30);
        adc.cr().modify(|r, w| w.bits(r.bits() | (1 << 31)));
        while adc.cr().read().bits() & (1 << 31) != 0 {}
        cortex_m::asm::delay(64 * 5);
        adc.smpr().write(|w| w.bits(0b111));
        adc.ccr().modify(|r, w| w.bits(r.bits() | (1 << 22)));
        adc.isr().write(|w| w.bits(1));
        adc.cr().modify(|r, w| w.bits(r.bits() | 1));
        while adc.isr().read().bits() & 1 == 0 {}
        core::ptr::write_volatile(
            COMP2_CSR,
            (0b1000 << 4)
                | (0b10 << 8)
                | if cfg!(feature = "bench-comp-hyst-low") {
                    1 << 16
                } else {
                    0
                }
                | 1,
        );
        cortex_m::asm::delay(320);
    }
    let vcal = unsafe { core::ptr::read_volatile(VREFINT_CAL_ADDR as *const u16) } as u32;
    uart_boot_clock(2);

    let mut syst = cp.SYST;
    syst.set_clock_source(SystClkSource::Core);
    syst.set_reload(64_000 - 1);
    syst.clear_current();
    syst.enable_counter();

    let _ = writeln!(serial, "\r\nDRV8304 TIM1 shell; ? help");
    while serial.flush().is_err() {}
    let _ = write!(serial, "> ");
    while serial.flush().is_err() {}
    // UART banner above is the boot announcement; omit duplicate RTT text.
    uart_boot_clock(3);

    let mut buf = [0u8; 32];
    let mut idx = 0usize;
    let mut millis: u32 = 0;
    let mut last_blink: u32 = 0;
    let mut blink = true;
    let mut bphase = false;

    // Drive mode: 0=idle, 1=fixed-frequency diagnostic, 2=startup campaign.
    let mut drive_mode: u8 = 0;
    let mut sfreq: u32 = 10; // electrical Hz
    let mut target_hz: u32 = 50;
    let mut target_duty_tenths: u32 = DEFAULT_DUTY_TENTHS;
    let mut catch_duty_tenths: u32 = CATCH_DUTY_TENTHS;
    let mut last_edge: u16 = 0;
    let mut sine_ticks: u32 = 0;
    let mut capture_head: usize = 0;
    let mut capture_len: usize = 0;
    let mut coast = false;
    let mut coast_edge: u16 = 0;
    let mut coast_len: usize = 0;
    let mut dump_pending = false;
    let mut dump_armed = false; // one-shot opt-in; never persist across completed attempts
    let mut dump_reason: u8 = 0; // 1=timeout, 2=nFAULT, 3=host, 4=ADC rail, 5=VBUS UV
    let mut run_start_us = 0u32;
    let mut drive_elapsed_us = 0u32;
    let mut coast_start_us = 0u32;
    let mut control_max_us = 0u16;
    let mut observe_armed = false;
    let mut engage_dry = false;
    let mut engage_power = false;
    let mut engage_duty: Option<u32> = None;
    let mut capture_stride = 1u32;
    let mut engage_result = 0u8;
    let mut observation_count = 0usize;
    #[cfg(feature = "bench-live-control")]
    let mut live_armed = false;
    #[cfg(feature = "bench-normal-restart")]
    let mut normal_restart: Option<normal_restart::Policy> = None;
    #[cfg(feature = "bench-normal-restart")]
    let mut first_tracking: [u32; 9] = [0; 9];
    #[cfg(feature = "bench-normal-restart")]
    let mut normal_restart_result = 0u32;
    #[cfg(feature = "bench-normal-restart")]
    let mut normal_restart_remaining_us = 0u32;
    #[cfg(feature = "bench-normal-restart")]
    let mut normal_restart_wait_start = 0u32;
    #[cfg(feature = "bench-normal-restart")]
    let mut normal_restart_handoff = false;
    #[cfg(feature = "bench-normal-restart")]
    let mut normal_restart_settings = false;
    #[cfg(feature = "bench-normal-restart")]
    let mut normal_restart_resume: Option<normal_restart::Resume> = None;
    #[cfg(feature = "bench-normal-restart")]
    let mut normal_restart_resume_target = 0u32;
    #[cfg(feature = "bench-normal-restart")]
    let mut normal_restart_resume_applied = 0u32;
    #[cfg(feature = "bench-normal-restart")]
    let mut normal_restart_resume_steps = 0u32;
    #[cfg(feature = "bench-duty50-ack-stamp")]
    let mut live50_ack_us = 0u32;
    #[cfg(feature = "bench-duty50-ack-stamp")]
    let mut live50_ack_seen = false;
    #[cfg(feature = "bench-target-ack-stamp")]
    let mut target_ack_us = [0u32; 3];
    #[cfg(feature = "bench-target-ack-stamp")]
    let mut target_ack_seen = [false; 3];
    #[cfg(feature = "bench-duty50-revisit-epoch")]
    let mut revisit50_before = (0u32, 0u32);
    #[cfg(feature = "bench-duty50-revisit-epoch")]
    let mut revisit48_before = (0u32, 0u32);
    #[cfg(feature = "bench-duty50-revisit-epoch")]
    let mut revisit48_seen = false;
    #[cfg(feature = "bench-revisit48-probe")]
    let mut revisit48_probe = (false, 0u32, 0u32, 0u32);

    loop {
        let wall_us = clock_us();
        if drive_mode != 0 && wave_timer::fault() != 0 {
            dump_reason = wave_timer::fault();
            gates_off();
            set_pin(3, 1, false);
            drive_mode = 0;
            coast = true;
            coast_edge = t17();
            coast_start_us = clock_us();
            coast_len = 0;
        }
        if drive_mode != 0 {
            drive_elapsed_us = wall_us.wrapping_sub(run_start_us);
            // Reserve one control interval for polling/scan latency so outputs
            // are already disabled by the five-second energized boundary.
            if drive_elapsed_us >= 4_999_000 {
                gates_off();
                set_pin(3, 1, false);
                drive_mode = 0;
                dump_reason = 1;
                coast = true;
                coast_edge = t17();
                coast_start_us = clock_us();
                coast_len = 0;
            }
        }
        if syst.has_wrapped() {
            millis = millis.wrapping_add(1);
        }
        if blink && millis.wrapping_sub(last_blink) >= 500 {
            last_blink = millis;
            bphase = !bphase;
            set_pin(0, 5, bphase);
            set_pin(1, 5, !bphase);
        }

        // Update the sine envelope at 1 kHz. TIM1 independently emits exact
        if engage_dry && drive_mode == 2 && drive_elapsed_us >= HANDOFF_US {
            engage_dry = false;
            gates_off();
            set_pin(3, 1, false);
            core_bench::prepare_early(target_hz);
            let acquired = if engage_power {
                if powered_timer::wake(&mut || serial.read().is_ok()) {
                    flying_bench::acquire_awake_feedback()
                } else {
                    flying_bench::refuse(13);
                    None
                }
            } else {
                flying_bench::acquire_feedback()
            };
            engage_result = if let Some((seed, origin)) = acquired {
                let ran = if engage_power {
                    core_bench::guarded_power_run(
                        seed,
                        origin,
                        run_start_us,
                        vcal,
                        engage_duty.unwrap_or(target_duty_tenths),
                        &mut || serial.read().is_ok(),
                    )
                } else {
                    core_bench::guarded_dry_run(seed, origin, run_start_us, vcal, &mut || {
                        serial.read().is_ok()
                    })
                };
                if ran { 1 } else { 2 }
            } else {
                3
            };
            gates_off();
            set_pin(3, 1, false);
            drive_elapsed_us = clock_us().wrapping_sub(run_start_us);
            drive_mode = 0;
            dump_reason = 1;
            coast = true;
            coast_edge = t17();
            coast_start_us = clock_us();
            coast_len = 0;
        }
        if observe_armed && drive_mode == 2 && drive_elapsed_us >= HANDOFF_US {
            observe_armed = false;
            engage_dry = false;
            // Any received byte is a conservative immediate abort during this
            // short microscope interval; no command parsing/formatting here.
            #[cfg(not(feature = "bench-driven-power"))]
            let handoff = wave_timer::halt_phase();
            #[cfg(not(feature = "bench-driven-power"))]
            let (n, reason, elapsed) =
                observation::run(handoff, target_hz, target_duty_tenths, || {
                    serial.read().is_ok()
                });
            #[cfg(all(feature = "bench-driven-power", not(feature = "bench-live-control")))]
            let (n, reason, elapsed) = driven_run::run(
                target_hz,
                target_duty_tenths,
                run_start_us,
                vcal,
                &mut || serial.read().is_ok(),
            );
            #[cfg(all(feature = "bench-driven-power", feature = "bench-live-control"))]
            let (n, reason, elapsed) = {
                let enabled = live_armed;
                live_armed = false; // one campaign only
                let mut parser = live_command::Parser::new();
                let mut reply = live_reply::Reply::new();
                #[cfg(feature = "bench-current-foldback-policy")]
                let mut current_governor = current_foldback::Governor::new();
                driven_run::run(
                    target_hz,
                    target_duty_tenths,
                    run_start_us,
                    vcal,
                    &mut || {
                        let received = serial.read(); // RX/stop always precedes TX
                        if !enabled || !powered_timer::owns() || !core_bench::real_irq_active() {
                            parser.clear();
                            #[cfg(feature = "bench-host-abort-byte")]
                            if !matches!(received, Err(nb::Error::WouldBlock)) {
                                let code = match &received {
                                    Ok(byte) => 0x100 | *byte as u32,
                                    Err(_) => 0x200,
                                } | ((powered_timer::owns() as u32) << 16)
                                    | ((core_bench::real_irq_active() as u32) << 17);
                                HOST_ABORT_OBS.store(code, portable_atomic::Ordering::Relaxed);
                            }
                            return !matches!(received, Err(nb::Error::WouldBlock));
                        }
                        let now = clock_us();
                        let action = match received {
                            Ok(b) => parser.byte(b, now),
                            Err(nb::Error::WouldBlock) => parser.poll(now),
                            Err(_) => {
                                #[cfg(feature = "bench-host-abort-byte")]
                                HOST_ABORT_OBS.store(0x300, portable_atomic::Ordering::Relaxed);
                                return true;
                            }
                        };
                        // An explicit stop wins before any foreground policy work.
                        if action == live_command::Action::Stop {
                            #[cfg(feature = "bench-host-abort-byte")]
                            HOST_ABORT_OBS.store(
                                match received {
                                    Ok(byte) => 0x400 | byte as u32,
                                    Err(_) => 0x500,
                                },
                                portable_atomic::Ordering::Relaxed,
                            );
                            return true;
                        }
                        #[cfg(feature = "bench-current-foldback-policy")]
                        if let Some(warning) = average_current_live::take_foldback_warning() {
                            // The Driver7 classifier must reach the retained
                            // hardware-fault boundary. Do not acknowledge or
                            // actuate on its first current block: a second
                            // consecutive over block still trips the unchanged
                            // terminal current guard in the DMA path.
                            #[cfg(feature = "bench-driver-fault-probe")]
                            let _ = warning;
                            #[cfg(not(feature = "bench-driver-fault-probe"))]
                            {
                                let Some(current) = core_bench::live_duty_current() else {
                                    return true;
                                };
                                #[cfg(not(feature = "bench-bus-recovery"))]
                                let reduction = current_governor.warning(
                                    current,
                                    warning.residual,
                                    warning.allowance,
                                );
                                #[cfg(feature = "bench-bus-recovery")]
                                let reduction = current_governor.bus_warning(current, now);
                                let Some(reduction) = reduction else {
                                    return true;
                                };
                                let Some(request) = live_duty::Prepared::new(
                                    phase_role_live::CARRIER.ticks(),
                                    reduction.duty,
                                ) else {
                                    return true;
                                };
                                if !powered_timer::update_live_duty(request)
                                    || !average_current_live::record_foldback(
                                        reduction.duty,
                                        reduction.step,
                                    )
                                {
                                    return true;
                                }
                                #[cfg(feature = "bench-normal-restart")]
                                if normal_restart_result == 1 {
                                    // A current-derived ceiling replaces the retained
                                    // restoration target; do not climb back into it.
                                    normal_restart_resume_target = reduction.duty;
                                    normal_restart_resume = normal_restart::Resume::new(
                                        reduction.duty,
                                        reduction.duty,
                                        now,
                                    );
                                    normal_restart_resume_applied = reduction.duty;
                                    normal_restart_resume_steps =
                                        normal_restart_resume.as_ref().map_or(0, |r| r.steps());
                                }
                            }
                        }
                        match action {
                            live_command::Action::Stop => unreachable!(),
                            live_command::Action::Duty(d) => {
                                #[cfg(feature = "bench-current-foldback-policy")]
                                let Some(duty) = current_governor.request(d as u32) else {
                                    #[cfg(feature = "bench-host-abort-byte")]
                                    HOST_ABORT_OBS.store(0x600, portable_atomic::Ordering::Relaxed);
                                    return true;
                                };
                                #[cfg(not(feature = "bench-current-foldback-policy"))]
                                let duty = d as u32;
                                let Some(request) = live_duty::Prepared::new(
                                    phase_role_live::CARRIER.ticks(),
                                    duty,
                                ) else {
                                    #[cfg(feature = "bench-host-abort-byte")]
                                    HOST_ABORT_OBS.store(0x700, portable_atomic::Ordering::Relaxed);
                                    return true;
                                };
                                if reply.byte().is_some() {
                                    #[cfg(feature = "bench-host-abort-byte")]
                                    HOST_ABORT_OBS.store(0x800, portable_atomic::Ordering::Relaxed);
                                    return true;
                                }
                                #[cfg(feature = "bench-normal-restart")]
                                if normal_restart_result == 1 {
                                    let Some(current) = core_bench::live_duty_current() else {
                                        return true;
                                    };
                                    if normal_restart::defer_upward(true, current, duty) {
                                        if let Some(resume) = normal_restart_resume.as_mut() {
                                            // A mismatch between the policy rung and the
                                            // published PWM is an ownership fault. Never
                                            // silently reset the two-second cadence.
                                            if !resume.retarget(duty, current) {
                                                return true;
                                            }
                                        } else {
                                            normal_restart_resume =
                                                normal_restart::Resume::new(duty, current, now);
                                        }
                                        let Some(resume) = normal_restart_resume.as_ref() else {
                                            return true;
                                        };
                                        normal_restart_resume_target = duty;
                                        normal_restart_resume_applied = current;
                                        normal_restart_resume_steps = resume.steps();
                                        // D reports the duty actually published, not an
                                        // upward target that recovery has merely queued.
                                        if !reply.queue(current, core_bench::live_interval()) {
                                            return true;
                                        }
                                        return false;
                                    }
                                }
                                if !powered_timer::update_live_duty(request) {
                                    #[cfg(feature = "bench-host-abort-byte")]
                                    HOST_ABORT_OBS.store(0x900, portable_atomic::Ordering::Relaxed);
                                    return true;
                                }
                                #[cfg(feature = "bench-bus-recovery")]
                                average_current_live::record_duty_change();
                                #[cfg(feature = "bench-normal-restart")]
                                if normal_restart_result == 1 {
                                    normal_restart_resume_target = duty;
                                    normal_restart_resume =
                                        normal_restart::Resume::new(duty, duty, now);
                                    normal_restart_resume_applied = duty;
                                    normal_restart_resume_steps =
                                        normal_restart_resume.as_ref().map_or(0, |r| r.steps());
                                }
                                if !reply.queue(duty, core_bench::live_interval()) {
                                    #[cfg(feature = "bench-host-abort-byte")]
                                    HOST_ABORT_OBS.store(0xa00, portable_atomic::Ordering::Relaxed);
                                    return true;
                                }
                                #[cfg(feature = "bench-target-ack-stamp")]
                                if let Some(index) = match duty {
                                    100 => Some(0),
                                    250 => Some(1),
                                    500 => Some(2),
                                    _ => None,
                                } {
                                    // Foreground-only, after guarded PWM publication and
                                    // ACK queue. No UART print or ISR work during the run.
                                    target_ack_us[index] = powered_timer::stream_now();
                                    target_ack_seen[index] = true;
                                }
                                #[cfg(feature = "bench-duty50-ack-stamp")]
                                if duty == 500 {
                                    // Foreground-only, after the guarded duty
                                    // transaction and ACK queue succeeded.
                                    live50_ack_us = powered_timer::stream_now();
                                    live50_ack_seen = true;
                                }
                                #[cfg(feature = "bench-duty50-revisit-epoch")]
                                if duty == 500 {
                                    revisit50_before = core_bench::running_revisit_counts();
                                }
                                #[cfg(feature = "bench-duty50-revisit-epoch")]
                                if duty == 480 {
                                    revisit48_before = core_bench::running_revisit_counts();
                                    revisit48_seen = true;
                                }
                                #[cfg(feature = "bench-revisit48-probe")]
                                if duty == 480 {
                                    let (attempts, accepts) = core_bench::running_revisit_counts();
                                    revisit48_probe = (true, powered_timer::stream_now(), attempts, accepts);
                                }
                                #[cfg(feature = "bench-rate-census")]
                                if (cfg!(feature = "bench-rate-curve")
                                    && matches!(duty, 50 | 100 | 150 | 200 | 250 | 300 | 350 | 400 | 450 | 500))
                                    || duty == 400
                                    || (cfg!(feature = "bench-phase-current-census") && duty == 450)
                                {
                                    rate_census::begin(powered_timer::stream_now(), duty);
                                }
                            }
                            live_command::Action::Status => {
                                let Some(d) = core_bench::live_duty_current() else {
                                    return true;
                                };
                                if !reply.queue(d, core_bench::live_interval()) {
                                    return true;
                                }
                            }
                            live_command::Action::None => {}
                        }
                        #[cfg(feature = "bench-bus-recovery")]
                        {
                            let Some(current) = core_bench::live_duty_current() else {
                                return true;
                            };
                            if let Some(duty) = current_governor.poll_recovery(now, current) {
                                let Some(request) = live_duty::Prepared::new(
                                    phase_role_live::CARRIER.ticks(),
                                    duty,
                                ) else {
                                    return true;
                                };
                                if !powered_timer::update_live_duty(request)
                                    || !average_current_live::record_recovery(duty)
                                {
                                    return true;
                                }
                            }
                        }
                        #[cfg(feature = "bench-normal-restart")]
                        if normal_restart_result == 1 {
                            if normal_restart_resume.is_none() {
                                let Some(current) = core_bench::live_duty_current() else {
                                    return true;
                                };
                                normal_restart_resume = normal_restart::Resume::new(
                                    normal_restart_resume_target,
                                    current,
                                    now,
                                );
                                normal_restart_resume_applied = current;
                            }
                            let due = normal_restart_resume.as_ref().and_then(|r| r.due(now));
                            if let Some(duty) = due {
                                #[cfg(feature = "bench-current-foldback-policy")]
                                let Some(duty) = current_governor.request(duty) else {
                                    return true;
                                };
                                let Some(request) = live_duty::Prepared::new(
                                    phase_role_live::CARRIER.ticks(),
                                    duty,
                                ) else {
                                    return true;
                                };
                                if !powered_timer::update_live_duty(request) {
                                    return true;
                                }
                                let Some(resume) = normal_restart_resume.as_mut() else {
                                    return true;
                                };
                                if !resume.applied(duty, now) {
                                    return true;
                                }
                                normal_restart_resume_applied = resume.current();
                                normal_restart_resume_steps = resume.steps();
                            }
                        }
                        // At most one byte; no flush/spin/formatting during drive.
                        if let Some(b) = reply.byte() {
                            match serial.write(b) {
                                Ok(()) => reply.sent(),
                                Err(nb::Error::WouldBlock) => {}
                                Err(_) => return true,
                            }
                        }
                        false
                    },
                )
            };
            observation_count = n;
            // Driver7 diagnostic images revoke every gate in the ISR, then
            // observe nFAULT with ENABLE high for 5.5 ms here in foreground.
            // Normal builds compile this to nothing.
            powered_timer::finish_driver_fault_probe();
            drive_elapsed_us = clock_us().wrapping_sub(run_start_us);
            drive_mode = 0;
            dump_reason = reason;
            #[cfg(feature = "bench-normal-restart")]
            if normal_restart_result == 1 {
                normal_restart_handoff = true;
            }
            #[cfg(feature = "bench-normal-restart")]
            if powered_timer::reason() == 8 && normal_restart.is_none() {
                first_tracking = powered_timer::stopped_snapshot();
                normal_restart_resume_target = core_bench::stopped_live_duty().unwrap_or(0);
                normal_restart = normal_restart::Policy::new(
                    run_start_us.wrapping_add(HANDOFF_US),
                    core_bench::configured_window_us(),
                );
                normal_restart_wait_start = clock_us();
            }
            coast = true;
            coast_edge = t17();
            coast_start_us = clock_us();
            #[cfg(feature = "bench-driven-power")]
            driven_run::coast_origin(coast_edge, t17());
            coast_len = 0;
            // No output until after coast so the capture starts without UART delay.
            let _ = elapsed;
        }

        // Envelope/feedback remain 1kHz and retain existing capture schema.
        if drive_mode != 0 {
            let now = t17();
            if now.wrapping_sub(last_edge) >= 1000 {
                control_max_us = control_max_us.max(now.wrapping_sub(last_edge));
                if now.wrapping_sub(last_edge) > 2000 {
                    gates_off();
                    set_pin(3, 1, false);
                    drive_mode = 0;
                    dump_reason = 7;
                    coast = true;
                    coast_edge = t17();
                    coast_start_us = clock_us();
                    coast_len = 0;
                    continue;
                }
                last_edge = now;
                let (freq_chz, effective_duty_tenths, stage) = if drive_mode == 2 {
                    if sine_ticks < ALIGN_TICKS {
                        // A short, proven-safe 1% static vector establishes the
                        // initial electrical reference without stall-current dwell.
                        (0, ALIGN_DUTY_TENTHS.min(target_duty_tenths), 1)
                    } else if sine_ticks < ALIGN_TICKS + START_TICKS {
                        (START_HZ * 100, catch_duty_tenths, 2)
                    } else if sine_ticks < ALIGN_TICKS + START_TICKS + RAMP_TICKS {
                        let elapsed = sine_ticks - ALIGN_TICKS - START_TICKS;
                        let target_chz =
                            if cfg!(feature = "bench-startup-staircase")
                                && !cfg!(feature = "bench-reverse-flat-start")
                                && target_hz == 200
                            {
                                campaign::staircase_target(sine_ticks) * 100
                            } else {
                                target_hz * 100
                            };
                        let start_chz = START_HZ * 100;
                        let freq_chz = campaign::ramp(start_chz, target_chz, elapsed, RAMP_TICKS);
                        let duty = campaign::ramp(
                            catch_duty_tenths,
                            target_duty_tenths,
                            elapsed,
                            RAMP_TICKS,
                        );
                        (freq_chz, duty, 3)
                    } else {
                        let hz = if cfg!(feature = "bench-startup-staircase")
                            && !cfg!(feature = "bench-reverse-flat-start")
                            && target_hz == 200
                        {
                            campaign::staircase_target(sine_ticks)
                        } else {
                            target_hz
                        };
                        (hz * 100, target_duty_tenths, 4)
                    }
                } else {
                    (sfreq * 100, target_duty_tenths, 0)
                };
                wave_timer::configure(campaign::phase_rate(freq_chz), effective_duty_tenths);
                let (sample_phase, on) = wave_timer::snapshot();
                sine_ticks += 1;
                if !get_idr(1, 14) {
                    drive_mode = 0;
                    gates_off();
                    set_pin(3, 1, false);
                    dump_reason = 2;
                    coast = true;
                    coast_edge = t17();
                    coast_start_us = clock_us();
                    coast_len = 0;
                    let _ = writeln!(serial, "\r\nnFAULT -> gates + en OFF; coast capture");
                    let _ = write!(serial, "> ");
                } else if sine_ticks % CAPTURE_DIV == 0 {
                    // ADC channel rotation stays at the original1kHz cadence,
                    // independent of how many records the fixture retains.
                    let sample = capture_sample(
                        (sine_ticks as usize - 1) % CAPTURE_N,
                        sine_ticks,
                        freq_chz,
                        sample_phase,
                        on,
                        stage,
                    );

                    let adc_railed = sample.ia == 0
                        || sample.ib == 0
                        || sample.ic == 0
                        || sample.ia >= 4095
                        || sample.ib >= 4095
                        || sample.ic >= 4095;
                    // Supplementary sample-block average during startup. The
                    // foreground cadence is not the steady 201 us DMA cadence:
                    // do not label this a calibrated 10 ms time-average.
                    #[cfg(all(
                        feature = "bench-average-current",
                        not(feature = "bench-startup-adc")
                    ))]
                    let average_failed = !average_current_live::scan_raw(
                        [sample.ia, sample.ib, sample.ic],
                        sample.vbus,
                        sample.vref,
                        vcal,
                    );
                    #[cfg(any(
                        not(feature = "bench-average-current"),
                        feature = "bench-startup-adc"
                    ))]
                    let average_failed = false;
                    // Same phase-peak backstop as the six-step observer.
                    // Not an estimate of average PSU current.
                    let phase_peak = !cfg!(any(
                        feature = "bench-startup-adc",
                        feature = "bench-average-current"
                    )) && [sample.ia, sample.ib, sample.ic]
                        .iter()
                        .any(|&v| (v as i32 - 2048).abs() > 1200);
                    let vdda_mv = if sample.vref > 0 {
                        3000 * vcal / sample.vref as u32
                    } else {
                        0
                    };
                    let vbus_mv = sample.vbus as u32 * vdda_mv / 4096 * 1194 / 100;
                    if campaign::retain_sample(
                        sine_ticks,
                        capture_stride,
                        sample.flags & 1 == 0
                            || adc_railed
                            || phase_peak
                            || average_failed
                            || vbus_mv < 8400,
                    ) {
                        capture_store(capture_head, sample);
                        capture_head = (capture_head + 1) % CAPTURE_N;
                        capture_len = (capture_len + 1).min(CAPTURE_N);
                    }
                    if sample.flags & 1 == 0 {
                        drive_mode = 0;
                        gates_off();
                        set_pin(3, 1, false);
                        dump_reason = 2;
                        coast = true;
                        coast_edge = t17();
                        coast_start_us = clock_us();
                        coast_len = 0;
                        let _ = writeln!(
                            serial,
                            "\r\nnFAULT during feedback scan -> gates + en OFF; coast capture"
                        );
                        let _ = write!(serial, "> ");
                    } else if adc_railed || phase_peak || average_failed {
                        drive_mode = 0;
                        gates_off();
                        set_pin(3, 1, false);
                        dump_reason = 4;
                        coast = true;
                        coast_edge = t17();
                        coast_start_us = clock_us();
                        coast_len = 0;
                        let _ = writeln!(
                            serial,
                            "\r\ncurrent ADC rail/peak -> gates + en OFF; coast capture"
                        );
                        let _ = write!(serial, "> ");
                    } else if vbus_mv < 8400 {
                        drive_mode = 0;
                        gates_off();
                        set_pin(3, 1, false);
                        dump_reason = 5;
                        coast = true;
                        coast_edge = t17();
                        coast_start_us = clock_us();
                        coast_len = 0;
                        let _ =
                            writeln!(serial, "\r\nVBUS below 6V -> gates + en OFF; coast capture");
                        let _ = write!(serial, "> ");
                    }
                }
                if drive_mode != 0 && sine_ticks >= RUN_TICKS {
                    drive_mode = 0;
                    gates_off();
                    set_pin(3, 1, false);
                    dump_reason = 1;
                    coast = true;
                    coast_edge = t17();
                    coast_start_us = clock_us();
                    coast_len = 0;
                    let _ = writeln!(
                        serial,
                        "\r\n5s energized limit -> gates + en OFF; coast capture"
                    );
                    let _ = write!(serial, "> ");
                }
            }
        }

        if coast {
            #[cfg(feature = "bench-startup-adc")]
            adc_stream::stop(); // foreground restore before any software ADC read
            if coast_len == 0 {
                let fly = flying_bench::take();
                let seed = core_bench::coast_take();
                if fly && seed != 0 {
                    flying_bench::refuse(10);
                    core_bench::coast_poll_arm(false);
                    let _ = writeln!(serial, "COASTREF refused conflicting_coastfly");
                } else if fly {
                    if dump_reason == 1 {
                        flying_bench::run();
                    } else {
                        flying_bench::refuse(9);
                    }
                } else if seed != 0 {
                    // Only a normal completed drive, never a fault/observer exit.
                    if dump_reason != 1 || !core_bench::coast_run(seed as u8, target_hz) {
                        let _ = writeln!(
                            serial,
                            "COASTREF refused normal_exit_and_167_250Hz_required"
                        );
                    }
                }
            }
            let now = t17();
            if now.wrapping_sub(coast_edge) >= COAST_PERIOD_US {
                coast_edge = now;
                coast_write(
                    coast_len,
                    coast_len as u32,
                    clock_us().wrapping_sub(coast_start_us),
                );
                coast_len += 1;
                if coast_len >= COAST_N {
                    coast = false;
                    dump_pending = true;
                }
            }
        }

        // A tracking stop may make exactly one fresh normal-startup attempt.
        // This is deliberately after the disabled coast interval, never from
        // an ISR and never from retained flying-seed state.
        #[cfg(feature = "bench-normal-restart")]
        if dump_pending
            && drive_mode == 0
            && !coast
            && normal_restart_result == 0
            && powered_timer::reason() == 8
            && clock_us().wrapping_sub(normal_restart_wait_start) >= normal_restart::SETTLE_US
        {
            let remaining = normal_restart.as_mut().and_then(|p|
                // Include the bounded prestart capture and driver wake that
                // happen before run_start_us is reset below, plus the timed
                // ADC first-frame admission used by an ordinary startup.
                p.restart(clock_us(),8,HANDOFF_US+52_000).ok());
            if let Some(remaining) = remaining {
                normal_restart_remaining_us = remaining;
                let remaining_ms = remaining / 1000;
                normal_restart_settings = driven_run::rearm_applied();
                if remaining_ms >= 20
                    && core_bench::power_window_ms(remaining_ms)
                    && normal_restart_settings
                    && driven_run::transfer_arm(true)
                {
                    prepare_sine();
                    set_pin(3, 1, true);
                    cortex_m::asm::delay(64_000);
                    let baseline = prestart_baseline::acquire(
                        &mut || matches!(serial.read(),Ok(b) if b!=b'\r' && b!=b'\n'),
                    );
                    let current = baseline && average_current_live::install();
                    #[cfg(feature = "bench-startup-adc")]
                    let startup_adc = if current && get_idr(1, 14) && adc_stream::startup_begin() {
                        let began = t17();
                        while adc_stream::startup_sample().is_none()
                            && t17().wrapping_sub(began) < 1000
                        {}
                        let ready = adc_stream::startup_sample().is_some();
                        if !ready {
                            adc_stream::stop();
                        }
                        ready
                    } else {
                        false
                    };
                    #[cfg(not(feature = "bench-startup-adc"))]
                    let startup_adc = true;
                    if current && get_idr(1, 14) && startup_adc {
                        pwm_sine(ALIGN_DUTY_TENTHS.min(target_duty_tenths), 0);
                        let restart_start_us = clock_us();
                        let restart_edge = t17();
                        let enabled = cortex_m::interrupt::free(|_| {
                            wave_timer::start(0, ALIGN_DUTY_TENTHS.min(target_duty_tenths));
                            #[cfg(feature = "bench-startup-adc")]
                            let adc_ready =
                                get_idr(3, 1) && adc_stream::startup_sample().is_some();
                            #[cfg(not(feature = "bench-startup-adc"))]
                            let adc_ready = true;
                            if get_idr(1, 14) && adc_ready {
                                pwm_moe(true);
                                true
                            } else {
                                gates_off();
                                set_pin(3, 1, false);
                                false
                            }
                        });
                        if enabled {
                            observe_armed = true;
                            engage_dry = false;
                            dump_pending = false;
                            #[cfg(feature = "bench-live-control")]
                            {
                                live_armed = true;
                            }
                            control_max_us = 0;
                            sine_ticks = 0;
                            capture_head = 0;
                            capture_len = 0;
                            coast_len = 0;
                            dump_reason = 0;
                            drive_mode = 2;
                            run_start_us = restart_start_us;
                            last_edge = restart_edge;
                            #[cfg(feature = "bench-duty50-ack-stamp")]
                            {
                                live50_ack_us = 0;
                                live50_ack_seen = false;
                            }
                            #[cfg(feature = "bench-target-ack-stamp")]
                            {
                                target_ack_us = [0; 3];
                                target_ack_seen = [false; 3];
                            }
                            #[cfg(feature = "bench-duty50-revisit-epoch")]
                            {
                                revisit50_before = (0, 0);
                                revisit48_before = (0, 0);
                                revisit48_seen = false;
                            }
                            #[cfg(feature = "bench-revisit48-probe")]
                            {
                                revisit48_probe = (false, 0, 0, 0);
                            }
                            normal_restart_result = 1;
                        } else {
                            #[cfg(feature = "bench-startup-adc")]
                            adc_stream::stop();
                            normal_restart_result = 3;
                        }
                    } else {
                        gates_off();
                        set_pin(3, 1, false);
                        normal_restart_result = 3;
                    }
                } else {
                    normal_restart_result = 4;
                }
            } else {
                normal_restart_result = 2;
            }
        }

        #[cfg(feature = "bench-normal-restart")]
        let normal_restart_waiting =
            normal_restart_result == 0 && normal_restart.is_some() && powered_timer::reason() == 8;
        #[cfg(not(feature = "bench-normal-restart"))]
        let normal_restart_waiting = false;
        if dump_pending && drive_mode == 0 && !coast && !normal_restart_waiting {
            dump_pending = false;
            #[cfg(feature = "bench-compact-qual")]
            {
                // Keep terminal summaries; omit optional bulk capture output.
                dump_armed = false;
            }
            stack_probe::report(&mut serial);
            let _ = writeln!(
                serial,
                "TIMING energized_us={} control_ticks={} max_control_interval_us={}",
                drive_elapsed_us, sine_ticks, control_max_us
            );
            let (updates, gap, cost) = wave_timer::stats();
            let _ = writeln!(
                serial,
                "WAVETIMING updates={} max_gap_us={} max_isr_us={} target_period_us=100 elapsed_phase=1 irq=1",
                updates, gap, cost
            );
            let coast_reference = core_bench::coast_report_pending();
            #[cfg(feature = "bench-duty50-ack-stamp")]
            {
                let stop_us = powered_timer::stopped_snapshot()[1];
                let _ = writeln!(
                    serial,
                    "LIVEACK50 seen={} accepted_us={} stop_us={} age_us={} same_powered_segment=1 ack_queued=1 pwm_transfer_within_one_carrier=1 postrun_only=1",
                    live50_ack_seen as u8,
                    live50_ack_us,
                    stop_us,
                    if live50_ack_seen { stop_us.saturating_sub(live50_ack_us) } else { 0 }
                );
            }
            #[cfg(feature = "bench-target-ack-stamp")]
            {
                let stop_us = powered_timer::stopped_snapshot()[1];
                for (index, duty) in [100u32, 250, 500].iter().enumerate() {
                    let seen = target_ack_seen[index];
                    let stamp = target_ack_us[index];
                    let _ = writeln!(
                        serial,
                        "LIVEACK target_tenths={} seen={} accepted_us={} stop_us={} age_us={} same_powered_segment=1 ack_queued=1 pwm_transfer_within_one_carrier=1 postrun_only=1",
                        duty, seen as u8, stamp, stop_us,
                        if seen { stop_us.saturating_sub(stamp) } else { 0 }
                    );
                }
            }
            #[cfg(feature = "bench-duty50-revisit-epoch")]
            {
                let after = core_bench::running_revisit_counts();
                let _ = writeln!(
                    serial,
                    "REVISIT48 ack_seen={} attempts_after_ack={} accepts_after_ack={} cutoff_tenths={} postrun_only=1",
                    revisit48_seen as u8,
                    if revisit48_seen { after.0.saturating_sub(revisit48_before.0) } else { 0 },
                    if revisit48_seen { after.1.saturating_sub(revisit48_before.1) } else { 0 },
                    if cfg!(feature = "bench-running-revisit-off48") { 480 } else { 0 }
                );
                let _ = writeln!(
                    serial,
                    "REVISIT50 ack_seen={} attempts_after_ack={} accepts_after_ack={} cutoff_tenths={} postrun_only=1",
                    live50_ack_seen as u8,
                    if live50_ack_seen { after.0.saturating_sub(revisit50_before.0) } else { 0 },
                    if live50_ack_seen { after.1.saturating_sub(revisit50_before.1) } else { 0 },
                    if cfg!(feature = "bench-running-revisit-off48") { 480 }
                    else if cfg!(feature = "bench-running-revisit-off50") { 500 }
                    else { 0 }
                );
            }
            #[cfg(feature = "bench-revisit48-probe")]
            {
                let (attempts, accepts) = core_bench::running_revisit_counts();
                let stop = powered_timer::stopped_snapshot()[1];
                let _ = writeln!(serial,
                    "R48 seen={} age={} attempts={} accepts={} cutoff={}",
                    revisit48_probe.0 as u8,
                    if revisit48_probe.0 { stop.saturating_sub(revisit48_probe.1) } else { 0 },
                    attempts.saturating_sub(revisit48_probe.2),
                    accepts.saturating_sub(revisit48_probe.3),
                    if cfg!(feature = "bench-running-revisit-off48") { 480 } else { 0 });
            }
            #[cfg(feature = "bench-normal-restart")]
            if normal_restart_result != 0 {
                let _ = writeln!(
                    serial,
                    "NORMALRESTART result={} first_reason={} first_stop_us={} first_commits={} original_deadline=1 flying_seed=0 one_shot=1",
                    normal_restart_result, first_tracking[0], first_tracking[1], first_tracking[2]
                );
                let second = powered_timer::stopped_snapshot();
                let _ = writeln!(
                    serial,
                    "NORMALRESTART2 handoff={} final_drive_reason={} final_power_reason={} final_power_stop_us={} settle_us={} settings_replayed={} outputs_disabled={}",
                    normal_restart_handoff as u8,
                    dump_reason,
                    second[0],
                    second[1],
                    normal_restart::SETTLE_US,
                    normal_restart_settings as u8,
                    powered_timer::outputs_disabled() as u8
                );
                let _ = writeln!(
                    serial,
                    "NORMALRESTART3 resume_target={} resume_applied={} resume_steps={} step_tenths={} period_us={} foreground_only=1",
                    normal_restart_resume_target,
                    normal_restart_resume_applied,
                    normal_restart_resume_steps,
                    normal_restart::RESUME_STEP_TENTHS,
                    normal_restart::RESUME_PERIOD_US
                );
                let _ = writeln!(
                    serial,
                    "NORMALRESTART4 planned_remaining_us={} actual_second_power_us={} window_rounding_us=1000 postrun_only=1",
                    normal_restart_remaining_us,
                    second[1]
                );
            }
            flying_bench::summary(&mut serial, dump_armed);
            if engage_result != 0 {
                if engage_power {
                    powered_timer::wake_summary(&mut serial);
                    let _ = writeln!(
                        serial,
                        "ENGAGEDUTY tenths={} requested_only=1",
                        engage_duty.unwrap_or(target_duty_tenths)
                    );
                }
                let _ = writeln!(
                    serial,
                    "{} path_result={} not_success_verdict=1 gate_authority={}",
                    if engage_power { "ENGAGE" } else { "ENGAGEDRY" },
                    engage_result,
                    engage_power as u8
                );
                if engage_result != 3 {
                    powered_timer::summary(&mut serial);
                    if dump_armed && engage_power {
                        powered_timer::coverage_dump(&mut serial);
                    }
                } else {
                    let _ = writeln!(serial, "POWERPATH not_started=1 acquisition_refused=1");
                }
                engage_result = 0;
            }
            #[cfg(feature = "bench-driven-power")]
            if observation_count > 0 && !dump_armed {
                driven_run::terminal_summary(&mut serial);
                #[cfg(all(feature = "bench-compact-qual", feature = "bench-lean-core"))]
                core_bench::lean_marker(&mut serial);
                #[cfg(feature = "bench-reverse-comp-dma-peer")]
                core_bench::reverse_priority_summary(&mut serial);
                #[cfg(feature = "bench-reverse-advance22-high")]
                core_bench::reverse_advance_summary(&mut serial);
                #[cfg(feature = "bench-com-lag")]
                core_bench::com_lag_summary(&mut serial);
                // Compact qualification suppresses the bulky waveform dump,
                // but a CPU diagnostic still needs its small aggregate rows.
                // The powered owner is already stopped at this point.
                #[cfg(all(feature = "bench-compact-qual", feature = "bench-cpu-aggregate"))]
                cpu_meter::dump(&mut serial);
                #[cfg(all(feature = "bench-compact-qual", feature = "bench-cpu-sparse"))]
                cpu_sparse::dump(&mut serial);
                #[cfg(feature = "bench-rate-census")]
                rate_census::dump(&mut serial, powered_timer::stopped_snapshot()[1]);
                #[cfg(feature = "bench-fast-sag-causal")]
                fast_sag_causal::dump(&mut serial, powered_timer::reason());
                #[cfg(feature = "bench-adc-latest-fault-frame")]
                adc_stream::fault_frame_dump(&mut serial);
                #[cfg(feature = "bench-running-level-revisit")]
                {
                    let (attempts, accepts) = core_bench::running_revisit_counts();
                    let _ = writeln!(serial, "RUNNINGREVISIT attempts={} accepts={} postrun_only=1", attempts, accepts);
                    #[cfg(feature = "bench-running-revisit-off35")]
                    let _ = writeln!(serial, "REVISITPOLICY cutoff_tenths=350 below=enabled at_or_above=physical_comp_only reverse_only=1 postrun_only=1");
                    #[cfg(feature = "bench-running-revisit-off50")]
                    let _ = writeln!(serial, "REVISITPOLICY cutoff_tenths=500 below=enabled at_or_above=physical_comp_only reverse_only=1 postrun_only=1");
                    #[cfg(feature = "bench-running-revisit-off48")]
                    let _ = writeln!(serial, "REVISITPOLICY cutoff_tenths=480 below=enabled at_or_above=physical_comp_only reverse_only=1 postrun_only=1");
                }
                #[cfg(feature = "bench-interval-tail")]
                if matches!(dump_reason, 8 | 26) {
                    core_bench::interval_tail_dump(&mut serial);
                }
                #[cfg(feature = "bench-reverse-low-irq-cap")]
                let _ = writeln!(serial, "IRQRATE peak={} limit={} bucket_us=1000 postrun_only=1", core_bench::irq_rate_peak(), core_bench::reverse_rate_limit());
                #[cfg(feature = "bench-reverse-irq-probe")]
                core_bench::reverse_irq_summary(&mut serial);
                #[cfg(feature = "bench-reverse-blank")]
                core_bench::reverse_blank_summary(&mut serial);
                #[cfg(feature = "bench-reverse-irq-cap24")]
                core_bench::observe_summary(&mut serial, false);
                #[cfg(feature = "bench-host-abort-byte")]
                let _ = writeln!(serial, "HOSTABORT observed={} code={} inactive_100byte_200error active_300rx_400byte_stop_500timeout_600governor_700prepare_800reply_busy_900writer_a00queue=1 owner_bit16_irq_bit17=1 postrun_only=1", (HOST_ABORT_OBS.load(portable_atomic::Ordering::Relaxed) != 0) as u8, HOST_ABORT_OBS.load(portable_atomic::Ordering::Relaxed));
                #[cfg(feature = "bench-fast-bus-sag")]
                average_current_live::quality_summary(&mut serial);
                #[cfg(feature = "bench-current-foldback-policy")]
                average_current_live::foldback_summary(&mut serial);
            }
            // Detailed core state can belong to the preceding run when an
            // acquisition is refused before prepare_driven. Keep it in the
            // opt-in dump, not the interactive attempt summary.
            if (observation_count > 0 || coast_reference)
                && (dump_armed || !cfg!(feature = "bench-driven-power") || observation_count == 0)
            {
                core_bench::observe_summary(&mut serial, dump_armed);
                if dump_armed {
                    core_bench::trace_dump(&mut serial);
                }
            }
            #[cfg(feature = "bench-fast-bus-tail")]
            powered_timer::bus_tail_dump(&mut serial);
            engage_dry = false;
            if dump_armed {
                dump_armed = false;
                let (valid, phase, stopped, six) = wave_timer::stop_anchor();
                if coast_len > 0
                    && valid
                    && !(cfg!(feature = "bench-driven-power") && observation_count > 0)
                {
                    let _ = writeln!(
                        serial,
                        "PHASEANCHOR commanded_q32={} stop_t17={} sixstep={} first_coast_delay_us={} counter_modulus_us=65536 before_moe_clear=1",
                        phase,
                        stopped,
                        six as u8,
                        unsafe { COAST_ANCHOR_DELAY }
                    );
                }
                // Measured read brackets remain valid even when the old
                // commanded-phase anchor does not describe this shutdown.
                if coast_len > 0 {
                    for i in 0..coast_len.min(32) {
                        let offsets = unsafe { COAST_COMP_OFFSETS[i] };
                        let _ = writeln!(
                            serial,
                            "COASTCOMP row={} a_us={} b_us={} c_us={} from_scan_start=1",
                            i, offsets[0], offsets[1], offsets[2]
                        );
                    }
                }
                if capture_len > 0 {
                    for phase in 0..3 {
                        let timing = unsafe { CURRENT_TIMING[phase] };
                        let _ = writeln!(
                            serial,
                            "CURRENTTIMING phase={} pwm_start={} pwm_end={} elapsed_us={} ccr={} raw={} bracket_not_aperture=1",
                            phase, timing[0], timing[1], timing[2], timing[3], timing[4]
                        );
                    }
                }
                dump_capture(
                    &mut serial,
                    capture_len,
                    capture_head,
                    dump_reason,
                    vcal,
                    capture_stride,
                );
                #[cfg(not(feature = "bench-driven-power"))]
                if observation_count > 0 {
                    observation::dump(&mut serial, observation_count);
                }
                #[cfg(feature = "bench-driven-power")]
                if observation_count > 0 {
                    driven_run::dump(&mut serial);
                }
                dump_coast(&mut serial, coast_len, vcal);
            } else {
                let _ = writeln!(
                    serial,
                    "DONE reason={} drive_records={} coast_records={} gates=off en=off",
                    dump_reason, capture_len, coast_len
                );
            }
            capture_len = 0;
            observation_count = 0;
            observe_armed = false;
            capture_head = 0;
            coast_len = 0;
            let _ = write!(serial, "> ");
            while serial.flush().is_err() {}
        }

        // Drain ALL available UART bytes each pass (no overrun).
        while let Ok(b) = serial.read() {
            if b == b'\r' || b == b'\n' {
                let line = &buf[..idx];
                let _ = writeln!(serial, "");
                if idx == 0 {
                } else if line == b"?" || line == b"help" {
                    let _ = writeln!(serial, "off p i | ehz<N> run<N> sf<N> du<N> | cap1 dump");
                } else if drive_mode == 2 && !coast && line.starts_with(b"ehz") {
                    // Live hold-only steps. Never restart the energized timer
                    // or permit duty/diagnostic commands while running.
                    if let Some(hz) = campaign::target(&line[3..]) {
                        if sine_ticks >= ALIGN_TICKS + START_TICKS + RAMP_TICKS
                            && hz.abs_diff(target_hz) <= 10
                        {
                            target_hz = hz;
                            let _ = writeln!(serial, "F{}", hz);
                        } else {
                            let _ = writeln!(serial, "!step");
                        }
                    } else {
                        let _ = writeln!(serial, "!hz");
                    }
                } else if (drive_mode != 0 || coast) && line != b"off" {
                    let _ = writeln!(serial, "!busy");
                } else if line.starts_with(b"catchdu") {
                    if let Some(duty) = campaign::target(&line[7..])
                        .filter(|&d| d <= MAX_DUTY_TENTHS && drive_mode == 0 && !coast)
                    {
                        catch_duty_tenths = duty;
                        let _ = writeln!(serial, "CATCHDUTY tenths={}", duty);
                    } else {
                        let _ = writeln!(serial, "!catchdu");
                    }
                } else if line.starts_with(b"capstride") {
                    let stride = core::str::from_utf8(&line[9..])
                        .ok()
                        .and_then(|s| s.parse::<u32>().ok());
                    if drive_mode == 0 && !coast && stride.is_some_and(|n| (1..=100).contains(&n)) {
                        capture_stride = stride.unwrap();
                        let _ = writeln!(
                            serial,
                            "CAPSTRIDE ticks={} adc_hz=1000 guards_unchanged=1",
                            capture_stride
                        );
                    } else {
                        let _ = writeln!(serial, "!capstride idle_only range1..100");
                    }
                } else if line == b"flycheck" {
                    flying_bench::run();
                    flying_bench::summary(&mut serial, dump_armed);
                } else if line.starts_with(b"flyseed") {
                    let sector = core::str::from_utf8(&line[7..])
                        .ok()
                        .and_then(|s| s.parse::<u32>().ok());
                    if drive_mode == 0 && !coast && sector.is_some_and(flying_bench::select_sector)
                    {
                        let _ = writeln!(serial, "FLYSEED sector={} zero_any=1", sector.unwrap());
                    } else {
                        let _ = writeln!(serial, "!flyseed idle_only range0..6");
                    }
                } else if line.starts_with(b"engagedu") {
                    if line == b"engagedu0" {
                        engage_duty = None;
                    } else if let Some(d) =
                        campaign::target(&line[8..]).filter(|&d| d <= MAX_DUTY_TENTHS)
                    {
                        engage_duty = Some(d);
                    } else {
                        let _ = writeln!(serial, "!engagedu");
                        continue;
                    }
                    let _ = writeln!(
                        serial,
                        "ENGAGEDUTY override_tenths={} zero_follows_run=1",
                        engage_duty.unwrap_or(0)
                    );
                } else if line.starts_with(b"engagems") {
                    let ms = core::str::from_utf8(&line[8..])
                        .ok()
                        .and_then(|s| s.parse::<u32>().ok());
                    if drive_mode == 0 && !coast && ms.is_some_and(core_bench::power_window_ms) {
                        let _ = writeln!(serial, "ENGAGEWINDOW ms={}", ms.unwrap());
                    } else {
                        let _ = writeln!(serial, "!engagems idle_only range20..600000");
                    }
                } else if line == b"reentry1" || line == b"reentry0" {
                    if drive_mode != 0 || coast {
                        let _ = writeln!(serial, "!reentry idle_only");
                        continue;
                    }
                    core_bench::reentry_arm(line == b"reentry1");
                    let _ = writeln!(
                        serial,
                        "REENTRY armed={} tracking_only=1 attempts_max=1",
                        (line == b"reentry1") as u8
                    );
                } else if line == b"dropout1" || line == b"dropout0" {
                    if drive_mode != 0 || coast {
                        let _ = writeln!(serial, "!dropout idle_only");
                        continue;
                    }
                    core_bench::dropout_arm(line == b"dropout1");
                    let _ = writeln!(
                        serial,
                        "DROPOUT armed={} after_ms=2000 stop_only=1",
                        (line == b"dropout1") as u8
                    );
                } else if line == b"stack" {
                    stack_probe::report(&mut serial);
                } else if cfg!(feature = "bench-driven-handoff")
                    && (line == b"drivereentry1" || line == b"drivereentry0")
                    && drive_mode == 0
                    && !coast
                {
                    #[cfg(feature = "bench-driven-handoff")]
                    {
                        core_bench::driven_reentry_arm(line == b"drivereentry1");
                        let _ = writeln!(
                            serial,
                            "DRIVEREENTRY armed={} tracking_only=1 attempts_max=1 one_shot=1",
                            (line == b"drivereentry1") as u8
                        );
                    }
                } else if cfg!(feature = "bench-driven-handoff")
                    && line.starts_with(b"drivedropms")
                    && drive_mode == 0
                    && !coast
                {
                    #[cfg(feature = "bench-driven-handoff")]
                    {
                        let ms = core::str::from_utf8(&line[11..])
                            .ok()
                            .and_then(|s| s.parse::<u32>().ok());
                        if ms.is_some_and(core_bench::driven_dropout_after_ms) {
                            let _ = writeln!(
                                serial,
                                "DRIVEDROPWINDOW ms={} range=2000..10000 idle_only=1",
                                ms.unwrap()
                            );
                        } else {
                            let _ = writeln!(serial, "!drivedropms idle_only range2000..10000");
                        }
                    }
                } else if cfg!(feature = "bench-normal-restart")
                    && (line == b"drivetrack1" || line == b"drivetrack0")
                    && drive_mode == 0
                    && !coast
                {
                    #[cfg(feature = "bench-normal-restart")]
                    {
                        core_bench::driven_tracking_arm(line == b"drivetrack1");
                        let _ = writeln!(
                            serial,
                            "DRIVETRACK armed={} configured_window=1 immediate_trip=1 one_shot=1",
                            (line == b"drivetrack1") as u8
                        );
                    }
                } else if cfg!(feature = "bench-driven-handoff")
                    && (line == b"drivedrop1" || line == b"drivedrop0")
                    && drive_mode == 0
                    && !coast
                {
                    #[cfg(feature = "bench-driven-handoff")]
                    {
                        core_bench::driven_dropout_arm(line == b"drivedrop1");
                        let _ = writeln!(
                            serial,
                            "DRIVEDROP armed={} configured_window=1 stop_only=1 one_shot=1",
                            (line == b"drivedrop1") as u8
                        );
                    }
                } else if cfg!(feature = "bench-driven-handoff")
                    && (line == b"drivex1" || line == b"drivex0")
                    && drive_mode == 0
                    && !coast
                {
                    #[cfg(feature = "bench-driven-handoff")]
                    {
                        let ok = driven_run::transfer_arm(line == b"drivex1");
                        let _ = writeln!(
                            serial,
                            "DRIVEX armed={} accepted={} one_shot=1",
                            (line == b"drivex1") as u8,
                            ok as u8
                        );
                    }
                } else if cfg!(feature = "bench-driven-power")
                    && line.starts_with(b"drivephase")
                    && drive_mode == 0
                    && !coast
                {
                    #[cfg(feature = "bench-driven-power")]
                    if let Some(degrees) = core::str::from_utf8(&line[10..])
                        .ok()
                        .and_then(|s| s.parse::<i32>().ok())
                    {
                        let ok = driven_run::set_phase(degrees);
                        let _ = writeln!(
                            serial,
                            "DRIVEPHASE target={} accepted={} one_shot=1 gate_authority=0",
                            degrees, ok as u8
                        );
                    }
                } else if cfg!(feature = "bench-driven-handoff")
                    && line == b"dutycheck"
                    && drive_mode == 0
                    && !coast
                {
                    #[cfg(feature = "bench-driven-handoff")]
                    driven_run::duty_check(&mut serial);
                } else if cfg!(feature = "bench-driven-handoff")
                    && line.starts_with(b"bemfdu")
                    && drive_mode == 0
                    && !coast
                {
                    #[cfg(feature = "bench-driven-handoff")]
                    if let Some(duty) = core::str::from_utf8(&line[6..])
                        .ok()
                        .and_then(|s| s.parse::<u32>().ok())
                    {
                        let ok = driven_run::set_bemf_duty(duty);
                        let _ = writeln!(
                            serial,
                            "BEMFDUTY target={} accepted={} one_shot=1 gate_authority=0",
                            duty, ok as u8
                        );
                    }
                } else if cfg!(feature = "bench-driven-power")
                    && line.starts_with(b"drivedu")
                    && drive_mode == 0
                    && !coast
                {
                    #[cfg(feature = "bench-driven-power")]
                    if let Some(duty) = core::str::from_utf8(&line[7..])
                        .ok()
                        .and_then(|s| s.parse::<u32>().ok())
                    {
                        let ok = driven_run::set_duty(duty);
                        let _ = writeln!(
                            serial,
                            "DRIVEDUTY target={} accepted={} one_shot=1 gate_authority=0",
                            duty, ok as u8
                        );
                    }
                } else if cfg!(feature = "bench-driven-power")
                    && line.starts_with(b"drivepwm")
                    && drive_mode == 0
                    && !coast
                {
                    #[cfg(feature = "bench-driven-power")]
                    if let Some(target) = core::str::from_utf8(&line[8..])
                        .ok()
                        .and_then(|s| s.parse::<u32>().ok())
                    {
                        let ok = pwm_sample_dma::set_target(target);
                        let _ = writeln!(
                            serial,
                            "DRIVEPWM target={} accepted={} gate_authority=0",
                            target, ok as u8
                        );
                    }
                } else if cfg!(feature = "bench-driven-power")
                    && line == b"pwmdmatiming"
                    && drive_mode == 0
                    && !coast
                {
                    #[cfg(feature = "bench-driven-power")]
                    pwm_sample_dma::timing(&mut serial, vcal);
                } else if cfg!(all(
                    feature = "bench-driven-entry",
                    not(feature = "bench-driven-irq")
                )) && (line == b"drivencheck"
                    || line == b"drivenadc"
                    || line == b"drivencomp"
                    || line == b"drivenstop"
                    || line == b"drivenphase")
                {
                    #[cfg(feature = "bench-driven-entry")]
                    if drive_mode == 0 && !coast {
                        driven_probe::run(
                            &mut serial,
                            line != b"drivencheck",
                            line == b"drivencomp"
                                || line == b"drivenstop"
                                || line == b"drivenphase",
                            line == b"drivenstop",
                            line == b"drivenphase",
                            vcal,
                        );
                    }
                } else if cfg!(feature = "bench-driven-handoff")
                    && line == b"transfercheck"
                    && drive_mode == 0
                    && !coast
                {
                    #[cfg(feature = "bench-driven-handoff")]
                    driven_run::transfer_check(&mut serial);
                } else if cfg!(feature = "bench-qualification-direct")
                    && line == b"directcheck"
                    && drive_mode == 0
                    && !coast
                {
                    #[cfg(feature = "bench-qualification-direct")]
                    qualification_direct_live::check(&mut serial);
                } else if cfg!(feature = "bench-qualification-event")
                    && line == b"qeventcheck"
                    && drive_mode == 0
                    && !coast
                {
                    #[cfg(feature = "bench-qualification-event")]
                    comp_path_live::event_check(&mut serial);
                } else if cfg!(feature = "bench-qualification-sparse")
                    && line == b"sparsecheck"
                    && drive_mode == 0
                    && !coast
                {
                    #[cfg(feature = "bench-qualification-sparse")]
                    qualification_sparse_live::check(&mut serial);
                } else if cfg!(feature = "bench-qualification-window")
                    && line == b"qualcheck"
                    && drive_mode == 0
                    && !coast
                {
                    #[cfg(feature = "bench-qualification-window")]
                    qualification_live::check(&mut serial);
                } else if cfg!(feature = "bench-comp-decisions")
                    && line == b"decisioncheck"
                    && drive_mode == 0
                    && !coast
                {
                    #[cfg(feature = "bench-comp-decisions")]
                    core_bench::decisioncheck(&mut serial);
                } else if cfg!(feature = "bench-comp-critical")
                    && line == b"criticalcheck"
                    && drive_mode == 0
                    && !coast
                {
                    #[cfg(feature = "bench-comp-critical")]
                    core_bench::criticalcheck(&mut serial);
                } else if cfg!(feature = "bench-static-comp")
                    && line == b"readcadencecheck"
                    && drive_mode == 0
                    && !coast
                {
                    #[cfg(feature = "bench-static-comp")]
                    core_bench::readcadencecheck(&mut serial);
                } else if cfg!(feature = "bench-final-edge-check")
                    && line == b"finalprepcheck"
                    && drive_mode == 0
                    && !coast
                {
                    #[cfg(feature = "bench-final-edge-check")]
                    core_bench::finalprepcheck(&mut serial);
                } else if cfg!(feature = "bench-seedmask-check")
                    && line == b"seedmaskcheck"
                    && drive_mode == 0
                    && !coast
                {
                    #[cfg(feature = "bench-seedmask-check")]
                    core_bench::seedmaskcheck(&mut serial);
                } else if cfg!(feature = "bench-dma-peer")
                    && line == b"dmaprioritycheck"
                    && drive_mode == 0
                    && !coast
                {
                    #[cfg(feature = "bench-dma-peer")]
                    dma_priority_probe::check(&mut serial);
                } else if cfg!(feature = "bench-com-peer")
                    && line == b"prioritycheck"
                    && drive_mode == 0
                    && !coast
                {
                    #[cfg(feature = "bench-com-peer")]
                    priority_probe::check(&mut serial);
                } else if cfg!(feature = "bench-filter-source")
                    && line == b"filtersourcecheck"
                    && drive_mode == 0
                    && !coast
                {
                    #[cfg(feature = "bench-filter-source")]
                    filtered_irq_check::check(&mut serial);
                } else if cfg!(feature = "bench-capture-filter")
                    && line == b"filtercheck"
                    && drive_mode == 0
                    && !coast
                {
                    #[cfg(feature = "bench-capture-filter")]
                    capture_filter_check::check(&mut serial);
                } else if line == b"atomiccheck" && drive_mode == 0 && !coast {
                    atomic_check::check(&mut serial);
                } else if cfg!(feature = "bench-cpu-timing")
                    && line == b"cpucheck"
                    && drive_mode == 0
                    && !coast
                {
                    #[cfg(feature = "bench-cpu-timing")]
                    cpu_meter::check(&mut serial);
                } else if cfg!(feature = "bench-reentry-staging")
                    && line == b"reentrystatscheck"
                    && drive_mode == 0
                    && !coast
                {
                    #[cfg(feature = "bench-reentry-staging")]
                    powered_timer::reentry_stats_check(&mut serial);
                } else if cfg!(feature = "bench-reentry-staging")
                    && line == b"archivecheck"
                    && drive_mode == 0
                    && !coast
                {
                    #[cfg(feature = "bench-reentry-staging")]
                    flying_bench::archive_check(&mut serial);
                } else if cfg!(feature = "bench-current-epoch")
                    && line == b"epochcheck"
                    && drive_mode == 0
                    && !coast
                {
                    #[cfg(feature = "bench-current-epoch")]
                    calibration_live::check(&mut serial);
                } else if cfg!(feature = "bench-average-current")
                    && line == b"avgnominal"
                    && drive_mode == 0
                    && !coast
                {
                    #[cfg(feature = "bench-average-current")]
                    {
                        let safe = !get_idr(3, 1) && powered_timer::outputs_disabled();
                        let vref = if safe {
                            (unsafe { adc_read(13) }) as u32
                        } else {
                            0
                        };
                        // Ceil VDDA: rounding cannot raise the raw allowance.
                        let vdda = if vref != 0 {
                            (3000 * vcal as u32 + vref - 1) / vref
                        } else {
                            0
                        };
                        let raw = average_current_live::nominal::raw_for_vdda(vdda).unwrap_or(0);
                        let accepted = safe && average_current_live::configure(raw);
                        let _ = writeln!(
                            serial,
                            "AVGNOMINAL accepted={} raw_sum={} scans={} target_ma={} vdda_mv={} gain=10 shunt_mohm=7 calibrated=0 uncertainty_bounded=0",
                            accepted as u8,
                            raw,
                            average_current_live::nominal::SCANS,
                            average_current_live::nominal::TARGET_MA,
                            vdda
                        );
                    }
                } else if cfg!(feature = "bench-average-current")
                    && line.starts_with(b"avgraw")
                    && drive_mode == 0
                    && !coast
                {
                    #[cfg(feature = "bench-average-current")]
                    {
                        let accepted = core::str::from_utf8(&line[6..])
                            .ok()
                            .and_then(|s| s.parse::<u32>().ok())
                            .is_some_and(average_current_live::configure);
                        let _ = writeln!(
                            serial,
                            "AVGRAW accepted={} scans={} units=raw_sum calibration_required=1",
                            accepted as u8,
                            average_current_live::nominal::SCANS
                        );
                    }
                } else if cfg!(feature = "bench-current-baseline")
                    && line == b"basecheck"
                    && drive_mode == 0
                    && !coast
                {
                    #[cfg(feature = "bench-current-baseline")]
                    prestart_baseline::check(&mut serial);
                } else if cfg!(feature = "bench-pwm-roles")
                    && line.starts_with(b"roledu")
                    && drive_mode == 0
                    && !coast
                {
                    #[cfg(feature = "bench-pwm-roles")]
                    if let Some(duty) = core::str::from_utf8(&line[6..])
                        .ok()
                        .and_then(|s| s.parse::<u32>().ok())
                    {
                        phase_role_live::check_duty(&mut serial, duty);
                    }
                } else if cfg!(feature = "bench-live-control")
                    && (line == b"live1" || line == b"live0")
                    && drive_mode == 0
                    && !coast
                {
                    #[cfg(feature = "bench-live-control")]
                    {
                        live_armed = line == b"live1";
                        let _ = writeln!(
                            serial,
                            "LIVE armed={} one_shot=1 duty_max={}",
                            live_armed as u8,
                            duty_envelope::MAX
                        );
                    }
                } else if line == b"recoverpwmcheck" && drive_mode == 0 && !coast {
                    #[cfg(all(
                        feature = "bench-recovery-duty-check",
                        not(feature = "bench-recovery-runtime-only")
                    ))]
                    phase_role_live::recovery_duty_check(&mut serial);
                    #[cfg(feature = "bench-recovery-runtime-only")]
                    let _ = writeln!(serial, "!recoverpwmcheck not linked");
                } else if line == b"livedutycheck" && drive_mode == 0 && !coast {
                    #[cfg(feature = "bench-live-duty-check")]
                    live_duty_hw::check(&mut serial);
                } else if cfg!(feature = "bench-pwm-roles")
                    && line == b"rolecheck"
                    && drive_mode == 0
                    && !coast
                {
                    #[cfg(feature = "bench-pwm-roles")]
                    phase_role_live::check(&mut serial);
                } else if cfg!(feature = "bench-preserve-mux-pending")
                    && line == b"muxpending"
                    && drive_mode == 0
                    && !coast
                {
                    #[cfg(feature = "bench-preserve-mux-pending")]
                    comp_input::mux_pending_check(&mut serial);
                } else if cfg!(feature = "bench-baseline-dma")
                    && (line == b"basemode" || line == b"basemode20")
                    && drive_mode == 0
                    && !coast
                {
                    #[cfg(feature = "bench-baseline-dma")]
                    adc_stream::baseline_check(
                        &mut serial,
                        if line == b"basemode20" { 20000 } else { 1000 },
                    );
                } else if cfg!(feature = "bench-filter-raw")
                    && line == b"rawadccheck"
                    && drive_mode == 0
                    && !coast
                {
                    #[cfg(feature = "bench-filter-raw")]
                    adc_stream::raw_check(&mut serial);
                } else if cfg!(feature = "bench-adc-phase")
                    && line == b"adcphasecheck"
                    && drive_mode == 0
                    && !coast
                {
                    #[cfg(feature = "bench-adc-phase")]
                    adc_phase_dma::check(&mut serial);
                } else if line == b"guardcheck" {
                    powered_timer::diagnostic(&mut serial);
                } else if line == b"recordcheck" {
                    if drive_mode != 0 || coast {
                        let _ = writeln!(serial, "!recordcheck idle_only");
                        continue;
                    }
                    core_bench::recordcheck(&mut serial);
                } else if line == b"engagedry1"
                    || line == b"engagedry0"
                    || line == b"engage1"
                    || line == b"engage0"
                {
                    if drive_mode != 0 || coast {
                        let _ = writeln!(serial, "!engage idle_only");
                        continue;
                    }
                    engage_power = line == b"engage1";
                    engage_dry = line == b"engagedry1" || engage_power;
                    if engage_dry {
                        observe_armed = false;
                        flying_bench::arm(false);
                        core_bench::coast_arm(0);
                        core_bench::coast_poll_arm(false);
                    }
                    let _ = writeln!(
                        serial,
                        "{} armed={} clears_other_probes={} gate_authority={}",
                        if engage_power { "ENGAGE" } else { "ENGAGEDRY" },
                        engage_dry as u8,
                        engage_dry as u8,
                        engage_power as u8
                    );
                } else if line == b"coasttrack1" || line == b"coasttrack0" {
                    flying_bench::arm_track(line == b"coasttrack1");
                    let _ = writeln!(
                        serial,
                        "COASTTRACK armed={} one_shot=1 bridge_disabled_only=1 gate_authority=0",
                        (line == b"coasttrack1") as u8
                    );
                } else if line == b"coastfly1" || line == b"coastfly0" {
                    flying_bench::arm(line == b"coastfly1");
                    let _ = writeln!(
                        serial,
                        "COASTFLY armed={} one_shot=1 bridge_disabled_only=1",
                        (line == b"coastfly1") as u8
                    );
                } else if line == b"coastpoll1" || line == b"coastpoll0" {
                    core_bench::coast_poll_arm(line == b"coastpoll1");
                    let _ = writeln!(
                        serial,
                        "COASTPOLL armed={} one_shot=1 bridge_disabled_only=1",
                        (line == b"coastpoll1") as u8
                    );
                } else if line == b"coastcheck" {
                    if core_bench::coast_run(1, 200) {
                        core_bench::coast_report_pending();
                        core_bench::observe_summary(&mut serial, dump_armed);
                        dump_armed = false;
                    } else {
                        let _ = writeln!(serial, "COASTREF refused outputs_must_be_disabled");
                    }
                } else if line.starts_with(b"coastref") {
                    let selected = core::str::from_utf8(&line[8..])
                        .ok()
                        .and_then(|s| s.trim().parse::<u32>().ok());
                    if let Some(s) = selected.filter(|&s| s <= 6 && drive_mode == 0 && !coast) {
                        core_bench::coast_arm(s);
                        let _ = writeln!(
                            serial,
                            "COASTREF armed={} one_shot=1 bridge_disabled_only=1",
                            s
                        );
                    } else {
                        let _ = writeln!(serial, "!coastref");
                    }
                } else if line.starts_with(b"obssector") {
                    let selected = core::str::from_utf8(&line[9..])
                        .ok()
                        .and_then(|s| s.trim().parse::<u32>().ok());
                    if let Some(s) = selected.filter(|&s| s <= 6 && drive_mode == 0 && !coast) {
                        core_bench::select_sector(s);
                        let _ = writeln!(serial, "OBSSECTOR armed={} one_shot=1", s);
                    } else {
                        let _ = writeln!(serial, "!obssector");
                    }
                } else if line.starts_with(b"obsphase") {
                    let degrees = core::str::from_utf8(&line[8..])
                        .ok()
                        .and_then(|s| s.trim().parse::<i32>().ok());
                    if let Some(d) =
                        degrees.filter(|d| (-60..=60).contains(d) && drive_mode == 0 && !coast)
                    {
                        observation::set_phase(d);
                        let _ = writeln!(serial, "OBSPHASE armed={}", d);
                    } else {
                        let _ = writeln!(serial, "!obsphase");
                    }
                } else if line.starts_with(b"obsdu") {
                    let value = core::str::from_utf8(&line[5..])
                        .ok()
                        .and_then(|s| s.parse::<u32>().ok());
                    if let Some(v) = value.filter(|v| *v <= MAX_DUTY_TENTHS) {
                        observation::set_duty(v);
                        let _ = writeln!(serial, "OBSDUTY armed={}", v);
                    } else {
                        let _ = writeln!(serial, "!obsdu");
                    }
                } else if cfg!(feature = "bench-driven-power")
                    && line == b"driveobs1"
                    && drive_mode == 0
                    && !coast
                {
                    observe_armed = true;
                    engage_dry = false;
                    let _ = writeln!(
                        serial,
                        "DRIVEOBS armed=1 window_us=20000 handoff_authority=0 one_shot=1"
                    );
                } else if !cfg!(feature = "bench-driven-power") && line == b"obs1" {
                    observe_armed = true;
                    let _ = writeln!(serial, "OBS armed one-shot 96ms after 4700ms sine");
                } else if line.starts_with(b"ehz") {
                    if line.len() == 3 {
                        let _ = writeln!(
                            serial,
                            "TARGET ehz={} duty_tenths={} ceiling_tenths={}",
                            target_hz, target_duty_tenths, MAX_DUTY_TENTHS
                        );
                    } else if let Some(hz) = campaign::target(&line[3..]) {
                        target_hz = hz;
                        let _ = writeln!(
                            serial,
                            "TARGET ehz={} (not started; run to start)",
                            target_hz
                        );
                    } else {
                        let _ = writeln!(serial, "?ehz <1..250>");
                    }
                } else if line == b"obs0" {
                    observe_armed = false;
                    let _ = writeln!(serial, "OBS cancelled");
                } else if line == b"cap1" {
                    dump_armed = true;
                    let _ = writeln!(serial, "CAPTURE armed one-shot a85-v1");
                } else if line == b"cap0" {
                    dump_armed = false;
                    let _ = writeln!(serial, "CAPTURE quiet");
                } else if line.len() == 5
                    && &line[..4] == b"pair"
                    && (b'0'..=b'5').contains(&line[4])
                {
                    pair_probe(&mut serial, (line[4] - b'0') as usize);
                } else if line.len() == 4
                    && &line[..3] == b"map"
                    && (b'0'..=b'5').contains(&line[3])
                {
                    // UNLOADED diagnostic only: operator must unplug motor first.
                    // One physical MCU input asserted, never a high/low pair.
                    let selected = (line[3] - b'0') as usize;
                    let tim = unsafe { &*stm32::TIM1::ptr() };
                    gates_off();
                    set_pin(3, 1, true);
                    let wake = t17();
                    while t17().wrapping_sub(wake) < 2_000 {}
                    let mut records = [[0u16; 10]; 3];
                    let mut fault = !get_idr(1, 14);
                    let mut pulse_us = 0u16;
                    for stage in 0..3 {
                        let start = t17();
                        if stage == 1 && !fault {
                            unsafe {
                                // Force active main for H, inactive main for N.
                                // Only the selected output's enable bit is set.
                                let mode = if selected < 3 { 0x50 } else { 0x40 };
                                tim.ccmr1_output().write(|w| w.bits(mode | (mode << 8)));
                                tim.ccmr2_output().write(|w| w.bits(mode));
                                let shift = (2 - selected % 3) * 4;
                                // N needs the main channel enabled as well on this
                                // TIM1 setup; forced-inactive keeps its pin LOW.
                                let enable = if selected < 3 { 1 } else { 5 };
                                tim.ccer().write(|w| w.bits(enable << shift));
                            }
                            pwm_moe(true);
                        }
                        if stage == 2 {
                            gates_off();
                        }
                        while t17().wrapping_sub(start) < 300 {}
                        for (i, channel) in [0u8, 1, 4, 2, 3, 6, 13].iter().enumerate() {
                            records[stage][i] = unsafe { adc_read(*channel) };
                        }
                        records[stage][7] = feedback_flags() as u16;
                        records[stage][8] = [(0, 10), (0, 9), (0, 8), (1, 1), (1, 0), (0, 7)]
                            .iter()
                            .enumerate()
                            .fold(0u16, |mask, (i, &(port, pin))| {
                                mask | ((get_idr(port, pin) as u16) << i)
                            });
                        records[stage][9] = t17().wrapping_sub(wake);
                        fault |= !get_idr(1, 14);
                        if stage == 1 {
                            gates_off();
                            pulse_us = t17().wrapping_sub(start);
                        }
                    }
                    gates_off();
                    set_pin(3, 1, false);
                    unsafe {
                        tim.ccmr1_output().write(|w| w.bits(0x6868));
                        tim.ccmr2_output().write(|w| w.bits(0x68));
                        tim.ccer().write(|w| w.bits(0x555));
                        tim.egr().write(|w| w.bits(1));
                    }
                    let _ = writeln!(
                        serial,
                        "MAP input={} pulse_us={} fault={} en=0 moe=0 fields=ia,ib,ic,vc,neutral,bus,vref,flags,gates,us",
                        selected, pulse_us, fault as u8
                    );
                    for (stage, r) in records.iter().enumerate() {
                        let _ = writeln!(
                            serial,
                            "MAP stage={} {} {} {} {} {} {} {} {} {} {}",
                            stage, r[0], r[1], r[2], r[3], r[4], r[5], r[6], r[7], r[8], r[9]
                        );
                    }
                } else if cfg!(feature = "bench-adc-probes") && line == b"adcwindow" {
                    gates_off();
                    set_pin(3, 1, false);
                    let adc = unsafe { &*stm32::ADC::ptr() };
                    let tim = unsafe { &*stm32::TIM1::ptr() };
                    let mut results = [[0u32; 5]; 2];
                    for (i, smp) in [2u32, 3].iter().enumerate() {
                        unsafe {
                            adc.smpr().write(|w| w.bits(*smp));
                            tim.ccr3().write(|w| w.bits(384));
                            tim.egr().write(|w| w.bits(1));
                        }
                        pwm_moe(true);
                        let mut start_min = 6400;
                        let mut start_max = 0;
                        let mut end_min = 6400;
                        let mut end_max = 0;
                        let mut failed = 0;
                        for _ in 0..128 {
                            adc_bemf_prepare();
                            let wait = t17();
                            while tim.cnt().read().bits() < 500 {
                                if t17().wrapping_sub(wait) > 200 {
                                    panic!("PWM wait timeout");
                                }
                            }
                            while !(32..64).contains(&tim.cnt().read().bits()) {
                                if t17().wrapping_sub(wait) > 200 {
                                    panic!("PWM window timeout");
                                }
                            }
                            let start = tim.cnt().read().bits();
                            let pin_before = get_idr(0, 10);
                            core::hint::black_box(adc_bemf_convert());
                            let end = tim.cnt().read().bits();
                            let pin_after = get_idr(0, 10);
                            if !pin_before || !pin_after || end >= 384 || end < start {
                                failed += 1;
                            }
                            start_min = start_min.min(start);
                            start_max = start_max.max(start);
                            end_min = end_min.min(end);
                            end_max = end_max.max(end);
                        }
                        gates_off();
                        results[i] = [start_min, start_max, end_min, end_max, failed];
                    }
                    unsafe {
                        adc.smpr().write(|w| w.bits(7));
                    }
                    for (i, r) in results.iter().enumerate() {
                        let _ = writeln!(
                            serial,
                            "ADCWINDOW smp={} n=128 start_min={} start_max={} end_min={} end_max={} failed={} en=0 moe=0",
                            i + 2,
                            r[0],
                            r[1],
                            r[2],
                            r[3],
                            r[4]
                        );
                    }
                } else if cfg!(feature = "bench-adc-probes") && line == b"adcsingle" {
                    gates_off();
                    set_pin(3, 1, false);
                    let adc = unsafe { &*stm32::ADC::ptr() };
                    let tim = unsafe { &*stm32::TIM1::ptr() };
                    for smp in [4u32, 5, 6] {
                        let mut stats = [6400u32, 0, 6400, 0, 0];
                        unsafe {
                            adc.smpr().write(|w| w.bits(smp));
                        }
                        for _ in 0..128 {
                            adc_pair_prepare(1 << 3);
                            let wait = t17();
                            while tim.cnt().read().bits() < 500 {
                                if t17().wrapping_sub(wait) > 200 {
                                    panic!("ADC single wait");
                                }
                            }
                            while !(32..64).contains(&tim.cnt().read().bits()) {
                                if t17().wrapping_sub(wait) > 200 {
                                    panic!("ADC single window");
                                }
                            }
                            let start = tim.cnt().read().bits();
                            core::hint::black_box(adc_single_convert());
                            let end = tim.cnt().read().bits();
                            stats[0] = stats[0].min(start);
                            stats[1] = stats[1].max(start);
                            stats[2] = stats[2].min(end);
                            stats[3] = stats[3].max(end);
                            if start < 26 || end >= 384 || end < start {
                                stats[4] += 1;
                            }
                        }
                        let _ = writeln!(
                            serial,
                            "ADCSINGLE ch=3 smp={} n=128 start_min={} start_max={} end_min={} end_max={} failed={} en=0 moe=0",
                            smp, stats[0], stats[1], stats[2], stats[3], stats[4]
                        );
                    }
                    unsafe {
                        adc.smpr().write(|w| w.bits(7));
                    }
                } else if cfg!(feature = "bench-adc-probes") && line == b"comtiming" {
                    com_timing::run(&mut serial);
                } else if cfg!(feature = "bench-adc-probes") && line == b"comirq" {
                    com_timer::diagnostic(&mut serial);
                } else if line == b"seedcheck" {
                    #[cfg(feature = "bench-seed-timing-reanchor")]
                    #[cfg(not(feature = "bench-live-control"))]
                    seed_timing_check::run(&mut serial);
                } else if line == b"deferredcheck" {
                    #[cfg(feature = "bench-startup-adc")]
                    driven_irq_live::deferred_check(&mut serial);
                } else if line == b"irqbudgetcheck" {
                    #[cfg(feature = "bench-seed-timing-reanchor")]
                    driven_irq_live::budget_check(&mut serial);
                } else if line == b"preparedload" {
                    #[cfg(feature = "bench-prepared-load")]
                    com_timer::prepared_load_check(&mut serial, vcal, 0x80);
                } else if line == b"preparedload0" {
                    #[cfg(feature = "bench-prepared-load")]
                    com_timer::prepared_load_check(&mut serial, vcal, 0);
                } else if line == b"preparedcheck" {
                    #[cfg(feature = "bench-prepared-timer")]
                    com_timer::prepared_check(&mut serial);
                } else if line == b"comarmcheck" {
                    #[cfg(feature = "bench-com-keep-running")]
                    com_timer::arm_check(&mut serial);
                } else if line == b"coretrace1" || line == b"coretrace0" {
                    if drive_mode == 0 && !coast {
                        core_bench::trace_enable(line == b"coretrace1");
                        let _ = writeln!(
                            serial,
                            "CORETRACE enabled={}",
                            (line == b"coretrace1") as u8
                        );
                    }
                } else if line == b"corepoll1" || line == b"corepoll0" {
                    core_bench::polling_arm(line == b"corepoll1");
                    let _ = writeln!(
                        serial,
                        "COREPOLL armed={} one_shot=1",
                        (line == b"corepoll1") as u8
                    );
                } else if cfg!(feature = "bench-adc-probes") && line == b"corebench" {
                    core_bench::run(&mut serial, false, dump_armed);
                    dump_armed = false;
                } else if cfg!(feature = "bench-adc-probes") && line == b"coreirq" {
                    core_bench::run(&mut serial, true, dump_armed);
                    dump_armed = false;
                } else if cfg!(feature = "bench-transport-probes") && line.starts_with(b"uartdma") {
                    let baud = core::str::from_utf8(&line[7..])
                        .ok()
                        .and_then(|s| s.trim().parse::<u32>().ok());
                    if let Some(baud) = baud.filter(|b| {
                        [
                            115200, 230400, 460800, 921600, 1000000, 2000000, 3000000, 4000000,
                            6000000, 8000000,
                        ]
                        .contains(b)
                    }) {
                        uart_dma::run(&mut serial, baud);
                    } else {
                        let _ = writeln!(serial, "?uartdma <supported baud>");
                    }
                } else if cfg!(feature = "bench-adc-probes") && line == b"compirq" {
                    comp_input::diagnostic(&mut serial, false);
                } else if cfg!(feature = "bench-adc-probes") && line == b"compirqref" {
                    comp_input::diagnostic(&mut serial, true);
                } else if (!cfg!(feature = "bench-driven-entry")
                    || cfg!(feature = "bench-adc-probes"))
                    && line == b"phasecheck"
                {
                    gates_off();
                    set_pin(3, 1, false);
                    wave_timer::start(campaign::phase_rate(20000), 0);
                    let reset_valid = wave_timer::stop_anchor().0;
                    let began = t17();
                    while t17().wrapping_sub(began) < 400 {}
                    gates_off();
                    let first = wave_timer::stop_anchor();
                    gates_off();
                    let second = wave_timer::stop_anchor();
                    let _ = writeln!(
                        serial,
                        "PHASECHECK reset_valid={} stopped_valid={} stable={} phase_q32={} six={} en={} moe={}",
                        reset_valid as u8,
                        first.0 as u8,
                        (first == second) as u8,
                        first.1,
                        first.3 as u8,
                        get_idr(3, 1) as u8,
                        unsafe { ((*stm32::TIM1::ptr()).bdtr().read().bits() >> 15) & 1 }
                    );
                } else if cfg!(feature = "bench-adc-probes")
                    && (line == b"obstiming" || line == b"obstiming200")
                {
                    gates_off();
                    set_pin(3, 1, false);
                    let (n, reason, us) = observation::run(
                        0,
                        if line == b"obstiming200" { 200 } else { 50 },
                        60,
                        || false,
                    );
                    observation::timing_summary(&mut serial, n);
                    let _ = writeln!(
                        serial,
                        "OBSTIMING reason={} us={} driver_disabled=1",
                        reason, us
                    );
                } else if cfg!(feature = "bench-adc-probes")
                    && (line == b"adcsettle" || line == b"adcsweep")
                {
                    gates_off();
                    set_pin(3, 1, false);
                    let adc = unsafe { &*stm32::ADC::ptr() };
                    let sweep = line == b"adcsweep";
                    for ch in 2u8..=3 {
                        if !sweep && ch != 3 {
                            continue;
                        }
                        for smp in 0..8 {
                            if !sweep && smp != 2 {
                                continue;
                            }
                            for prior in [2u8, 13u8] {
                                let mut sums = [0u32; 4];
                                for _ in 0..32 {
                                    unsafe {
                                        adc.smpr().write(|w| w.bits(7));
                                        adc_read(prior);
                                        adc.smpr().write(|w| w.bits(smp));
                                    }
                                    for j in 0..8 {
                                        let v = unsafe { adc_read(ch) } as u32;
                                        if j == 0 {
                                            sums[0] += v;
                                        }
                                        if j == 1 {
                                            sums[1] += v;
                                        }
                                        if j == 7 {
                                            sums[2] += v;
                                        }
                                    }
                                    unsafe {
                                        adc.smpr().write(|w| w.bits(7));
                                    }
                                    sums[3] += unsafe { adc_read(ch) } as u32;
                                }
                                let _ = writeln!(
                                    serial,
                                    "ADCSETTLE ch={} smp={} prior={} n=32 first={} second={} eighth={} long={} en=0 moe=0",
                                    ch,
                                    smp,
                                    prior,
                                    sums[0] / 32,
                                    sums[1] / 32,
                                    sums[2] / 32,
                                    sums[3] / 32
                                );
                            }
                        }
                    }
                } else if cfg!(feature = "bench-adc-probes") && line == b"adcverify" {
                    gates_off();
                    set_pin(3, 1, false);
                    let adc = unsafe { &*stm32::ADC::ptr() };
                    let _ = writeln!(
                        serial,
                        "ADCVERIFY cfgr1={:08x} cfgr2={:08x} comp={:08x} en=0 moe=0",
                        adc.cfgr1().read().bits(),
                        adc.cfgr2().read().bits(),
                        unsafe { core::ptr::read_volatile(COMP2_CSR) }
                    );
                    for &(a, b) in &[(2u8, 3u8), (6, 13)] {
                        for smp in [2, 7] {
                            unsafe {
                                adc.smpr().write(|w| w.bits(smp));
                            }
                            let mut sums = [0u32; 4];
                            let mut delta_min = [4095i32; 2];
                            let mut delta_max = [-4095i32; 2];
                            for _ in 0..32 {
                                let singles = [unsafe { adc_read(a) }, unsafe { adc_read(b) }];
                                adc_pair_prepare((1 << a) | (1 << b));
                                let pair = adc_bemf_convert();
                                let values = [singles[0], singles[1], pair.0, pair.1];
                                for j in 0..4 {
                                    sums[j] += values[j] as u32;
                                }
                                for j in 0..2 {
                                    let d = values[j + 2] as i32 - values[j] as i32;
                                    delta_min[j] = delta_min[j].min(d);
                                    delta_max[j] = delta_max[j].max(d);
                                }
                            }
                            let _ = writeln!(
                                serial,
                                "ADCVERIFY ch={},{} smp={} n=32 single={},{} pair={},{} delta_min={},{} delta_max={},{}",
                                a,
                                b,
                                smp,
                                sums[0] / 32,
                                sums[1] / 32,
                                sums[2] / 32,
                                sums[3] / 32,
                                delta_min[0],
                                delta_min[1],
                                delta_max[0],
                                delta_max[1]
                            );
                        }
                    }
                    unsafe {
                        adc.smpr().write(|w| w.bits(7));
                    }
                } else if cfg!(feature = "bench-adc-probes") && line == b"adctiming" {
                    gates_off();
                    set_pin(3, 1, false);
                    let adc = unsafe { &*stm32::ADC::ptr() };
                    let tim = unsafe { &*stm32::TIM1::ptr() };
                    let mut results = [[0u32; 2]; 8];
                    for smp in 0..8 {
                        unsafe {
                            adc.smpr().write(|w| w.bits(smp));
                        }
                        let mut minimum = 6400;
                        let mut maximum = 0;
                        for _ in 0..32 {
                            let start = tim.cnt().read().bits();
                            let pair = adc_bemf_pair();
                            core::hint::black_box(pair);
                            let end = tim.cnt().read().bits();
                            let delta = (end + 6400 - start) % 6400;
                            minimum = minimum.min(delta);
                            maximum = maximum.max(delta);
                        }
                        results[smp as usize] = [minimum, maximum];
                    }
                    unsafe {
                        adc.smpr().write(|w| w.bits(7));
                    }
                    for (smp, r) in results.iter().enumerate() {
                        let _ = writeln!(
                            serial,
                            "ADCTIME smp={} min_ticks={} max_ticks={} clock_hz=64000000 en=0 moe=0",
                            smp, r[0], r[1]
                        );
                    }
                } else if (!cfg!(feature = "bench-driven-entry")
                    || cfg!(feature = "bench-adc-probes"))
                    && line == b"sixcheck"
                {
                    gates_off();
                    set_pin(3, 1, false);
                    let mut results = [[0u32; 5]; 6];
                    for step in 1..=6 {
                        let plan = sixstep_apply(step, 60);
                        let start = t17();
                        let mut any = 0u32;
                        let mut every = 63u32;
                        let mut samples = 0u32;
                        while t17().wrapping_sub(start) < 1_000 {
                            let bits = [(0, 10), (0, 9), (0, 8), (1, 1), (1, 0), (0, 7)]
                                .iter()
                                .enumerate()
                                .fold(0u32, |mask, (i, &(port, pin))| {
                                    mask | ((get_idr(port, pin) as u32) << i)
                                });
                            any |= bits;
                            every &= bits;
                            samples += 1;
                        }
                        gates_off();
                        let expected_any =
                            (1 << plan.source) | (1 << (plan.source + 3)) | (1 << (plan.sink + 3));
                        let expected_every = 1 << (plan.sink + 3);
                        results[step as usize - 1] =
                            [any, every, samples, expected_any, expected_every];
                    }
                    let tim = unsafe { &*stm32::TIM1::ptr() };
                    unsafe {
                        tim.ccmr1_output().write(|w| w.bits(0x6868));
                        tim.ccmr2_output().write(|w| w.bits(0x68));
                        tim.ccer().write(|w| w.bits(0x555));
                        tim.egr().write(|w| w.bits(1));
                    }
                    for (i, r) in results.iter().enumerate() {
                        let _ = writeln!(
                            serial,
                            "SIXCHECK step={} any={} every={} n={} expected_any={} expected_every={} en={}",
                            i + 1,
                            r[0],
                            r[1],
                            r[2],
                            r[3],
                            r[4],
                            get_idr(3, 1) as u8
                        );
                    }
                } else if ((!cfg!(feature = "bench-dma-feedback")
                    && !cfg!(feature = "bench-driven-entry"))
                    || cfg!(feature = "bench-current-probes"))
                    && line == b"sensezero"
                {
                    // Legacy probe kept separately from the guarded reader.
                    // Wake the CSA without enabling any gate. No UART while awake.
                    gates_off();
                    let mut low = [4095u16; 7];
                    let mut high = [0u16; 7];
                    let mut sum = [0u32; 7];
                    let channels = [0u8, 1, 4, 2, 3, 6, 13];
                    let started = t17();
                    set_pin(3, 1, true);
                    let mut fault = false;
                    while t17().wrapping_sub(started) < 2_000 {
                        fault |= !get_idr(1, 14);
                    }
                    let mut count = 0u32;
                    let mut scan_fault = false;
                    while count < 32 && t17().wrapping_sub(started) < 10_000 {
                        for (i, &channel) in channels.iter().enumerate() {
                            let value = unsafe { adc_read(channel) };
                            low[i] = low[i].min(value);
                            high[i] = high[i].max(value);
                            sum[i] += value as u32;
                        }
                        scan_fault |= !get_idr(1, 14);
                        count += 1;
                    }
                    gates_off();
                    set_pin(3, 1, false);
                    let elapsed = t17().wrapping_sub(started);
                    let _ = writeln!(
                        serial,
                        "ZERO n={} awake_us={} wake_fault={} scan_fault={} en=0 moe=0",
                        count, elapsed, fault as u8, scan_fault as u8
                    );
                    for i in 0..7 {
                        let _ = writeln!(
                            serial,
                            "ZERO ch={} min={} mean={} max={}",
                            channels[i],
                            low[i],
                            sum[i] / count.max(1),
                            high[i]
                        );
                    }
                } else if cfg!(feature = "bench-current-probes") && line == b"adctriggercheck" {
                    if drive_mode != 0 || coast {
                        let _ = writeln!(serial, "!adctriggercheck idle_only");
                        continue;
                    }
                    adc_trigger_probe::run(&mut serial, 0, false);
                } else if cfg!(feature = "bench-current-probes") && line == b"adcscancheck" {
                    if drive_mode != 0 || coast {
                        let _ = writeln!(serial, "!adcscancheck idle_only");
                        continue;
                    }
                    adc_trigger_probe::run(&mut serial, 1, dump_armed);
                    dump_armed = false;
                } else if cfg!(feature = "bench-current-probes")
                    && (line == b"adccycliccheck" || line == b"adccyclicstall")
                {
                    if drive_mode != 0 || coast {
                        let _ = writeln!(serial, "!adccyclic idle_only");
                        continue;
                    }
                    adc_trigger_probe::run(
                        &mut serial,
                        if line == b"adccyclicstall" { 3 } else { 2 },
                        dump_armed,
                    );
                    dump_armed = false;
                } else if cfg!(feature = "bench-current-probes") && line == b"zerocheck" {
                    if drive_mode != 0 || coast {
                        let _ = writeln!(serial, "!zerocheck idle_only");
                        continue;
                    }
                    powered_timer::zero_check(&mut serial, dump_armed);
                    dump_armed = false;
                } else if ((!cfg!(feature = "bench-dma-feedback")
                    && !cfg!(feature = "bench-driven-entry"))
                    || cfg!(feature = "bench-adc-probes"))
                    && line == b"pwmcheck"
                {
                    gates_off();
                    set_pin(3, 1, false);
                    let tim = unsafe { &*stm32::TIM1::ptr() };
                    unsafe {
                        tim.ccr1().write(|w| w.bits(384));
                        tim.ccr2().write(|w| w.bits(384));
                        tim.ccr3().write(|w| w.bits(384));
                        tim.egr().write(|w| w.bits(1));
                    }
                    pwm_moe(true);
                    let start = t17();
                    let mut previous = false;
                    let mut edges = 0u32;
                    let mut first = 0u16;
                    let mut last = 0u16;
                    while t17().wrapping_sub(start) < 20_000 && edges < 101 {
                        let value = get_idr(0, 10);
                        if value && !previous {
                            let stamp = t17();
                            if edges == 0 {
                                first = stamp;
                            }
                            last = stamp;
                            edges += 1;
                        }
                        previous = value;
                    }
                    gates_off();
                    let _ = writeln!(
                        serial,
                        "PWMCHECK en={} edges={} span_us={} PSC={} ARR={} CCMR1={:x} CCMR2={:x} CCER={:x} BDTR={:x}",
                        get_idr(3, 1) as u8,
                        edges,
                        last.wrapping_sub(first),
                        tim.psc().read().bits(),
                        tim.arr().read().bits(),
                        tim.ccmr1_output().read().bits(),
                        tim.ccmr2_output().read().bits(),
                        tim.ccer().read().bits(),
                        tim.bdtr().read().bits()
                    );
                } else if line == b"a" {
                    let vref = unsafe { adc_read(13) } as u32;
                    let vdda = if vref > 0 { 3000 * vcal / vref } else { 0 };
                    let mv = |raw: u16| raw as u32 * vdda / 4096;
                    let ia = mv(unsafe { adc_read(CURRENT_ADC[0]) });
                    let ib = mv(unsafe { adc_read(CURRENT_ADC[1]) });
                    let ic = mv(unsafe { adc_read(CURRENT_ADC[2]) });
                    let vbus = mv(unsafe { adc_read(6) }) * 1194 / 100;
                    let vsc = mv(unsafe { adc_read(2) });
                    let neu = mv(unsafe { adc_read(3) });
                    let _ = writeln!(serial, "VDDA={}mV VBUS={}mV", vdda, vbus);
                    // CSA gain/offset are not bench-calibrated. Report measured
                    // voltages, not misleading nominal mA (and soft divisions).
                    let _ = writeln!(serial, "IA={}mV IB={}mV IC={}mV", ia, ib, ic);
                    let _ = writeln!(serial, "VSENC={}mV NEU={}mV", vsc, neu);
                } else if line.starts_with(b"run") {
                    let requested = if line.len() == 3 {
                        Some(target_hz)
                    } else {
                        campaign::target(&line[3..])
                    };
                    if let Some(n) = requested.filter(|_| drive_mode == 0 && !coast) {
                        #[cfg(feature = "bench-normal-restart")]
                        {
                            normal_restart = None;
                            first_tracking = [0; 9];
                            normal_restart_result = 0;
                            normal_restart_remaining_us = 0;
                            normal_restart_wait_start = 0;
                            normal_restart_handoff = false;
                            normal_restart_settings = false;
                            normal_restart_resume = None;
                            normal_restart_resume_target = 0;
                            normal_restart_resume_applied = 0;
                            normal_restart_resume_steps = 0;
                        }
                        #[cfg(feature = "bench-duty50-ack-stamp")]
                        {
                            live50_ack_us = 0;
                            live50_ack_seen = false;
                        }
                        #[cfg(feature = "bench-target-ack-stamp")]
                        {
                            target_ack_us = [0; 3];
                            target_ack_seen = [false; 3];
                        }
                        #[cfg(feature = "bench-duty50-revisit-epoch")]
                        {
                            revisit50_before = (0, 0);
                            revisit48_before = (0, 0);
                            revisit48_seen = false;
                        }
                        #[cfg(feature = "bench-revisit48-probe")]
                        {
                            revisit48_probe = (false, 0, 0, 0);
                        }
                        target_hz = n;
                        #[cfg(feature = "bench-host-abort-byte")]
                        HOST_ABORT_OBS.store(0, portable_atomic::Ordering::Relaxed);
                        prepare_sine();
                        set_pin(3, 1, true);
                        cortex_m::asm::delay(64_000); // DRV wake before applying lows
                        #[cfg(feature = "bench-current-baseline")]
                        // The command parser dispatches on CR; its LF may still
                        // be pending. Only terminators are ignored, not an off
                        // command's first byte or an explicit control-C abort.
                        if !prestart_baseline::acquire(
                            &mut || matches!(serial.read(),Ok(b) if b!=b'\r' && b!=b'\n'),
                        ) {
                            gates_off();
                            set_pin(3, 1, false);
                            let _ = writeln!(
                                serial,
                                "RUN refused: prestart baseline incomplete; gates + en OFF"
                            );
                            if dump_armed {
                                prestart_baseline::dump(&mut serial);
                            }
                            continue;
                        }
                        #[cfg(feature = "bench-average-current")]
                        if !average_current_live::install() {
                            gates_off();
                            set_pin(3, 1, false);
                            let _ = writeln!(
                                serial,
                                "RUN refused: average current configuration missing; gates + en OFF"
                            );
                            continue;
                        }
                        if get_idr(1, 14) {
                            pwm_sine(ALIGN_DUTY_TENTHS.min(target_duty_tenths), 0);
                            control_max_us = 0;
                            sine_ticks = 0;
                            capture_head = 0;
                            capture_len = 0;
                            coast_len = 0;
                            dump_pending = false;
                            dump_reason = 0;
                            drive_mode = 2;
                            let _ = writeln!(
                                serial,
                                "RUN: align={}ms@{}.{:01}% catch={}Hz/{}ms@{}.{:01}% ramp={}ms target={}Hz/{}.{:01}% hold={}ms",
                                ALIGN_TICKS,
                                ALIGN_DUTY_TENTHS.min(target_duty_tenths) / 10,
                                ALIGN_DUTY_TENTHS.min(target_duty_tenths) % 10,
                                START_HZ,
                                START_TICKS,
                                catch_duty_tenths / 10,
                                catch_duty_tenths % 10,
                                RAMP_TICKS,
                                target_hz,
                                target_duty_tenths / 10,
                                target_duty_tenths % 10,
                                HOLD_TICKS,
                            );
                            // UART banner can take several control periods.
                            // Finish it with MOE off, then start the drive clock.
                            while serial.flush().is_err() {}
                            #[cfg(feature = "bench-startup-adc")]
                            {
                                if !adc_stream::startup_begin() {
                                    gates_off();
                                    set_pin(3, 1, false);
                                    drive_mode = 0;
                                    let _ = writeln!(
                                        serial,
                                        "RUN refused: timed startup ADC admission; gates + en OFF"
                                    );
                                    continue;
                                }
                                let began = t17();
                                while adc_stream::startup_sample().is_none()
                                    && t17().wrapping_sub(began) < 1000
                                {
                                }
                                if adc_stream::startup_sample().is_none() {
                                    gates_off();
                                    set_pin(3, 1, false);
                                    adc_stream::stop();
                                    drive_mode = 0;
                                    let _ = writeln!(
                                        serial,
                                        "RUN refused: timed startup ADC first frame; gates + en OFF"
                                    );
                                    continue;
                                }
                            }
                            run_start_us = clock_us();
                            last_edge = t17();
                            cortex_m::interrupt::free(|_| {
                                wave_timer::start(0, ALIGN_DUTY_TENTHS.min(target_duty_tenths));
                                // Arm before interrupts resume: an ISR fault must
                                // never be followed by foreground re-enabling MOE.
                                #[cfg(feature = "bench-startup-adc")]
                                let adc_ready =
                                    get_idr(3, 1) && adc_stream::startup_sample().is_some();
                                #[cfg(not(feature = "bench-startup-adc"))]
                                let adc_ready = true;
                                if get_idr(1, 14) && adc_ready {
                                    pwm_moe(true);
                                } else {
                                    gates_off();
                                    set_pin(3, 1, false);
                                }
                            });
                        } else {
                            gates_off();
                            set_pin(3, 1, false);
                            let _ = writeln!(serial, "RUN refused: nFAULT low; gates + en OFF");
                        }
                    } else if drive_mode != 0 || coast {
                        let _ = writeln!(serial, "?busy; use off first");
                    } else {
                        let _ = writeln!(serial, "?run [1..250 eHz]");
                    }
                } else if !cfg!(feature = "bench-follow-prevalidate")
                    && line[0] == b'r'
                    && !line.starts_with(b"rf")
                {
                    let mut n = 0u32;
                    let mut any = false;
                    for &c in &line[1..] {
                        if c.is_ascii_digit() {
                            n = n * 10 + (c - b'0') as u32;
                            any = true;
                        }
                    }
                    if any && n <= 18 {
                        let vref = unsafe { adc_read(13) } as u32;
                        let vdda = if vref > 0 { 3000 * vcal / vref } else { 0 };
                        let raw = unsafe { adc_read(n as u8) };
                        let _ =
                            writeln!(serial, "ch{} raw={} {}mV", n, raw, raw as u32 * vdda / 4096);
                    } else {
                        let _ = writeln!(serial, "?r <0..18>");
                    }
                } else if !cfg!(feature = "bench-follow-prevalidate") && line == b"c" {
                    let s = |x: bool| if x { "hi" } else { "lo" };
                    let _ = writeln!(
                        serial,
                        "BEMF: A={} B={} C={}",
                        s(comp_read(6)),
                        s(comp_read(7)),
                        s(comp_read(8))
                    );
                } else if line == b"i" {
                    let _ = write!(serial, "IN:");
                    for (nm, p, b) in ALL_PINS {
                        let _ = write!(serial, " {}={}", nm, if get_idr(p, b) { 1 } else { 0 });
                    }
                    let _ = writeln!(serial, "");
                } else if line == b"p" {
                    let g = |p, b| if get_odr(p, b) { '1' } else { '0' };
                    let tim = unsafe { &*stm32::TIM1::ptr() };
                    let moe = (tim.bdtr().read().bits() >> 15) & 1;
                    let _ = writeln!(
                        serial,
                        "OUT: ah={} bh={} ch={} al={} bl={} cl={} led={} ld4={} en={} | mode={} coast={} sf={}Hz target={}Hz duty={}.{:01}% TIM1:moe={} ccrA={} ccrB={} ccrC={} arr={}",
                        g(0, 10),
                        g(0, 9),
                        g(0, 8),
                        g(1, 1),
                        g(1, 0),
                        g(0, 7),
                        g(1, 5),
                        g(0, 5),
                        g(3, 1),
                        drive_mode,
                        if coast { 1 } else { 0 },
                        sfreq,
                        target_hz,
                        target_duty_tenths / 10,
                        target_duty_tenths % 10,
                        moe,
                        tim.ccr3().read().bits(),
                        tim.ccr2().read().bits(),
                        tim.ccr1().read().bits(),
                        tim.arr().read().bits(),
                    );
                } else if !cfg!(feature = "bench-prepared-handoff") && line == b"blink" {
                    blink = !blink;
                    let _ = writeln!(serial, "blink {}", if blink { "on" } else { "off" });
                } else if line == b"off" {
                    #[cfg(feature = "bench-live-control")]
                    {
                        live_armed = false;
                    }
                    #[cfg(feature = "bench-driven-power")]
                    driven_run::clear_bemf_duty();
                    #[cfg(feature = "bench-driven-handoff")]
                    core_bench::driven_reentry_arm(false);
                    #[cfg(feature = "bench-driven-handoff")]
                    core_bench::driven_dropout_arm(false);
                    #[cfg(feature = "bench-driven-handoff")]
                    {
                        let _ = driven_run::transfer_arm(false);
                    }
                    let was_driving = drive_mode != 0;
                    drive_mode = 0;
                    gates_off();
                    set_pin(3, 1, false);
                    if was_driving && capture_len > 0 {
                        dump_reason = 3;
                        dump_pending = false;
                        coast = true;
                        coast_edge = t17();
                        coast_start_us = clock_us();
                        coast_len = 0;
                        let _ = writeln!(serial, "gates + en OFF; host-stop coast capture");
                    } else if coast {
                        coast = false;
                        dump_pending = capture_len > 0;
                        let _ = writeln!(serial, "gates + en OFF; coast stopped");
                    } else {
                        let _ = writeln!(serial, "gates + en OFF, idle");
                    }
                } else if !cfg!(feature = "bench-follow-prevalidate") && line == b"sine" {
                    if drive_mode == 0 && !coast {
                        prepare_sine();
                        set_pin(3, 1, true);
                        cortex_m::asm::delay(64_000);
                        if !get_idr(1, 14) {
                            gates_off();
                            set_pin(3, 1, false);
                            let _ = writeln!(serial, "SINE refused: nFAULT low; gates + en OFF");
                        } else {
                            drive_mode = 1;
                            pwm_sine(target_duty_tenths, 0);
                            control_max_us = 0;
                            sine_ticks = 0;
                            capture_head = 0;
                            capture_len = 0;
                            coast_len = 0;
                            dump_pending = false;
                            dump_reason = 0;
                            let _ = writeln!(
                                serial,
                                "SINE on: TIM1 10kHz carrier, {}.{:01}% max, {}Hz, en=1, 5s + coast capture",
                                target_duty_tenths / 10,
                                target_duty_tenths % 10,
                                sfreq
                            );
                            while serial.flush().is_err() {}
                            run_start_us = clock_us();
                            last_edge = t17();
                            cortex_m::interrupt::free(|_| {
                                wave_timer::start(
                                    campaign::phase_rate(sfreq * 100),
                                    target_duty_tenths,
                                );
                                if get_idr(1, 14) {
                                    pwm_moe(true);
                                } else {
                                    gates_off();
                                    set_pin(3, 1, false);
                                }
                            });
                        }
                    } else {
                        if capture_len > 0 {
                            dump_reason = 3;
                            dump_pending = true;
                        }
                        drive_mode = 0;
                        coast = false;
                        gates_off();
                        set_pin(3, 1, false);
                        let _ = writeln!(serial, "SINE off: gates + en OFF");
                    }
                } else if line.starts_with(b"du") {
                    let mut n = 0u32;
                    let mut any = false;
                    for &c in &line[2..] {
                        if c.is_ascii_digit() {
                            n = n * 10 + (c - b'0') as u32;
                            any = true;
                        }
                    }
                    if any && n >= 1 && n <= MAX_DUTY_TENTHS {
                        target_duty_tenths = n;
                        let _ = writeln!(
                            serial,
                            "du={} tenths-percent ({}.{:01}%)",
                            target_duty_tenths,
                            target_duty_tenths / 10,
                            target_duty_tenths % 10
                        );
                    } else {
                        let _ = writeln!(serial, "?du <1..100 tenths-percent>");
                    }
                } else if line.starts_with(b"sf") {
                    let mut n = 0u32;
                    let mut any = false;
                    for &c in &line[2..] {
                        if c.is_ascii_digit() {
                            n = n * 10 + (c - b'0') as u32;
                            any = true;
                        }
                    }
                    if any && n >= 1 && n <= 500 {
                        sfreq = n;
                        let _ = writeln!(serial, "sf={}Hz", sfreq);
                    } else {
                        let _ = writeln!(serial, "?sf <1..500>");
                    }
                } else if let Some(eq) = line.iter().position(|&c| c == b'=') {
                    let name = &line[..eq];
                    let val = line.get(eq + 1).copied().unwrap_or(b'0');
                    let on = val == b'1' || val == b'h';
                    match pin_lookup(name) {
                        Some((p, b)) => {
                            if name == b"led" || name == b"ld4" {
                                blink = false;
                            }
                            set_pin(p, b, on);
                            let _ = writeln!(
                                serial,
                                "{}={}",
                                core::str::from_utf8(name).unwrap_or("?"),
                                if on { 1 } else { 0 }
                            );
                        }
                        None => {
                            let _ = writeln!(serial, "?pin");
                        }
                    }
                } else {
                    let _ = writeln!(serial, "?cmd");
                }
                idx = 0;
                let _ = write!(serial, "> ");
            } else if idx < buf.len() - 1 && b >= 0x20 {
                buf[idx] = b;
                idx += 1;
                let _ = serial.write(b);
            }
        }
    }
}
