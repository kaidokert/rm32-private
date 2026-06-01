//! Motor tester: sinewave open-loop drive with UART key commands.
//!
//! Sinewave commutation runs from the TIM7 ISR at DRIVE_HZ (6 kHz),
//! so the electrical frequency is honoured regardless of main-loop load.
//!
//! Wire: USB-TTL TX → PB4 (RX in), PB3 → USB-TTL RX (status out), GND ↔ GND.
//!
//! Keys (QWERTY vertical pairs — top adds, bottom subtracts):
//!   f / v   electrical frequency  +10 / -10 Hz
//!   d / c   electrical frequency   +1 /  -1 Hz
//!   a / z   amplitude              +1 /  -1 % (capped at AMP_CAP)
//!   M       toggle waveform: sine ↔ 6-step BLDC
//!   m       dump last 2-rev raw ADC ring, full 16-bit hex (4-char + space per sample)
//!   p       cycle raw ring observed phase: A (ch17) → B (ch5) → C (ch14) → A …
//!   l       dump last 2-rev ADC-level waveform (dense ring-buffer, per-PWM-tick sample)
//!   e       de-staircased sector view: 6 sectors × unique triggered steps, role-labelled
//!           (most useful in triggered-triplet mode; compresses 4.8× repeats to 1 per step)
//!   s       toggle ADC capture mode: async-legacy (fresh 96kHz) ↔ triggered-triplet (TIM1_TRGO)
//!   w       kill — duties to midpoint, no current
//!   q       reset — re-enable at defaults
//!   i       VBUS + phase-U current proxy + NTC temperature + CPU busy %
//!           (no bus-current shunt; OPAMP1/PA1 is the only single-channel proxy)

#![no_std]
#![no_main]

use core::fmt::Write;
use cortex_m::peripheral::NVIC;
use cortex_m_rt::{entry, exception};
use embedded_io::{Read, ReadReady};
use portable_atomic::{AtomicBool, AtomicU32, Ordering};
use rtt_target::rprintln;
use systick_timer::Timer;

use rinz::hal;
use rinz::hal::adc::{AdcClaim, AdcCommonExt, Configured, config::SampleTime};
use rinz::hal::opamp::Gain;
use rinz::hal::prelude::*;
use rinz::hal::pwm::PwmAdvExt;
use rinz::hal::pwr::{PwrExt, VoltageScale};
use rinz::hal::rcc::{PllConfig, PllMDiv, PllNMul, PllRDiv, PllSrc};
use rinz::hal::serial::FullConfig;
use rinz::hal::time::{ExtU32, Hertz, RateExtU32};
use rinz::hal::{rcc, stm32};
use rinz::idle_loop::IdleLoop;

// 170 MHz from 8 MHz HSE: M=2 N=85 R=2
const SYSCLK_HZ: u32 = 170_000_000;
const TICK_HZ: u64 = 100_000; // 10 µs ticks
const SYSTICK_RELOAD: u32 = 0x00FF_FFFF; // fires ~once/s, no starvation
const SECOND_TICKS: u64 = TICK_HZ;
const MICROLOOP_TICKS: u64 = TICK_HZ / 1_000; // 1 ms

/// TIM7 ISR rate. Drives both commutation step accumulation and ADC sampling.
/// Higher values give more BEMF/current samples per sector (floor: ~2.8 µs for
/// two ADC reads → hard ceiling ~150 kHz). At 96 kHz: 57 samples/sector at 280 Hz.
const DRIVE_HZ: u32 = 96_000;

/// Triplet buffer: max samples per sector. 340 × 6 = 2040 ≤ 2048.
/// Covers ~10 Hz electrical floor (20000/(340×6)≈9.8 Hz).
const MAX_TRIP_SPS: usize = 340;

static TIMER: Timer = Timer::new(TICK_HZ, SYSTICK_RELOAD, SYSCLK_HZ as u64);

fn now_u64() -> u64 {
    TIMER.now()
}

/// 48-step sine table centred at 127 (0 = trough, 254 = peak).
static SINE48: [u8; 48] = [
    127, 144, 160, 176, 191, 205, 217, 227, 237, 244, 250, 253, 254, 253, 250, 244, 237, 227, 217,
    205, 191, 176, 160, 144, 127, 110, 94, 79, 64, 50, 37, 27, 17, 10, 4, 1, 0, 1, 4, 10, 17, 27,
    37, 50, 64, 79, 94, 110,
];

/// Safety ceiling: user cannot push amplitude above this percentage.
const AMP_CAP: u32 = 30;
const AMP_START: u32 = 8;
const FREQ_MIN: u32 = 1;
const FREQ_MAX: u32 = 600;
const FREQ_START: u32 = 60;
/// Initial waveform mode. true = six-step, false = sine. Used at boot and on 'q' reset.
const SIX_STEP_START: bool = true;

// Shared state written by main, read by TIM7 ISR.
static RUNNING: AtomicBool = AtomicBool::new(false);
static AMPLITUDE: AtomicU32 = AtomicU32::new(AMP_START);
static ELECTRICAL_HZ: AtomicU32 = AtomicU32::new(FREQ_START);
/// false = sine, true = 6-step BLDC
static SIX_STEP: AtomicBool = AtomicBool::new(SIX_STEP_START);

/// Main loop requests a clean restart of raw ring diagnostics by setting this.
/// TIM7 owns the hot-path state and performs the actual reset in one place.
static RAW_RING_RESET: AtomicBool = AtomicBool::new(true);

/// ADC capture mode: false = async continuous (default), true = TIM1-triggered.
static ADC_TRIGGERED: AtomicBool = AtomicBool::new(true);

// ---------------------------------------------------------------------------
// Raw BEMF sample ring — 96 kHz, enabled when electrical_hz > 100 ('l' dump)
// ---------------------------------------------------------------------------

/// 2048 × (1/96000 s) ≈ 21.3 ms ≈ 2.1 revs at the 100 Hz floor.
const RAW_SAMPLE_LEN: usize = 2048;
const RAW_SAMPLE_MASK: usize = RAW_SAMPLE_LEN - 1;

/// Raw ADC2 u16 samples written by TIM7 ISR at 96 kHz.
/// Main reads with TIM7 masked (motor stalls for the duration of the 'l' dump).
/// Safety: TIM7 is sole writer; main only reads under NVIC::mask(TIM7).
static mut RAW_SAMPLE_BUF: [u16; RAW_SAMPLE_LEN] = [0u16; RAW_SAMPLE_LEN];

/// Monotonically-incrementing write cursor. Next write slot = WR & MASK.
static RAW_SAMPLE_WR: AtomicU32 = AtomicU32::new(0);

/// ADC2 channel to store in RAW_SAMPLE_BUF. 0 = follow floating phase (commutated).
/// Nonzero = always sample this fixed channel regardless of sector:
///   17 = phase A (PA4/IN17), 5 = phase B (PC4/IN5), 14 = phase C (PB11/IN14).
static RING_CHAN_FIXED: AtomicU32 = AtomicU32::new(17);

/// Driven-high / driven-low phase index per six-step sector (A=0, B=1, C=2).
const SIX_STEP_HIGH: [usize; 6] = [0, 0, 1, 1, 2, 2];
const SIX_STEP_LOW: [usize; 6] = [1, 2, 2, 0, 0, 1];
/// phase_idx (0=A,1=B,2=C) → ADC2_DMA_BUF slot [ch17/A=0, ch14/C=1, ch5/B=2].
const PHASE_TO_DMA: [usize; 3] = [0, 2, 1];

/// Circular DMA destination for ADC2 continuous scan: [ch17/A, ch14/C, ch5/B].
/// Written continuously by DMA1_CH1; ISR reads latest value by channel index.
static mut ADC2_DMA_BUF: [u16; 3] = [0u16; 3];

/// Per-sector sample-ring start index for the in-progress revolution.
/// `[s]` = RAW_SAMPLE_WR when commanded sector `s` first started this rev.
/// Sentinel u32::MAX = sector not yet started. Written by TIM7 ISR only.
static mut CUR_REV_SECTOR_STARTS: [u32; 6] = [u32::MAX; 6];

/// Circular history of the last 3 completed revolutions.
/// `[i][s]` = RAW_SAMPLE_WR when commanded sector `s` started in revolution `i`.
/// Oldest entry at index REV_HIST_HEAD, newest at (REV_HIST_HEAD + 2) % 3.
/// Written by TIM7 ISR; read by main with TIM7 masked.
static mut REV_SECTOR_STARTS: [[u32; 6]; 3] = [[0u32; 6]; 3];
static mut REV_HIST_HEAD: usize = 0;
static mut REV_HIST_COUNT: usize = 0;

/// Snapshot buffer for 'l'/'m'/'e' dumps. Filled under TIM7 mask, printed after unmask.
/// Sized for worst-case 1 rev at the 101 Hz floor: 96000/101 ≈ 951 samples; 2048 is ample.
const SNAP_LEN: usize = 2048;
static mut SNAP_BUF: [u16; SNAP_LEN] = [0u16; SNAP_LEN];
/// Parallel V_neut snapshot for 'e' dump. Same index space as SNAP_BUF.
static mut SNAP_NEUT_BUF: [u16; SNAP_LEN] = [0u16; SNAP_LEN];
/// V_neut = (V_hi + V_lo)/2 per ring slot. Written by TIM7 in triggered mode; 0 in async.
static mut RAW_NEUT_BUF: [u16; RAW_SAMPLE_LEN] = [0u16; RAW_SAMPLE_LEN];

/// Per-sector sample count for triplet buffer. TIM7-only writer; read under TIM7 mask or when frozen.
static mut TRIP_COUNTS: [usize; 6] = [0usize; 6];
/// Snapshot of TRIP_COUNTS taken at rev-wrap before s0 counter is reset. Main reads this.
static mut TRIP_COUNTS_SNAP: [usize; 6] = [0usize; 6];
/// Main sets to request freeze at next rev boundary; ISR sets TRIP_FROZEN and clears this.
static TRIP_FREEZE_REQUESTED: AtomicBool = AtomicBool::new(false);
/// When set, ISR stops writing triplet buffers. Main reads freely. Clear to resume.
static TRIP_FROZEN: AtomicBool = AtomicBool::new(false);
/// Fractional accumulator for 20 kHz triplet write cadence inside the 96 kHz TIM7 ISR.
/// Advances by 20000 each tick; when it reaches DRIVE_HZ one triplet is written and
/// DRIVE_HZ is subtracted. Gives exactly 20000 writes/sec, one per TIM1 trigger on average.
static mut TRIP_FRAC: u32 = 0;

/// Busy-wait delay that doesn't need SYST (taken by systick-timer).
struct AsmDelay;
impl embedded_hal::delay::DelayNs for AsmDelay {
    fn delay_ns(&mut self, ns: u32) {
        cortex_m::asm::delay(SYSCLK_HZ / 1_000_000 * ns / 1_000 + 1);
    }
}

/// Read ADC1 channel 5 (PB14/NTC thermistor).
/// stm32g4xx-hal 0.1.0 omits PB14 from its G431 Channel impl, so we configure
/// the sequence and sample-time registers directly via PAC.  The ADC is already
/// enabled in the `Configured` state; we just override SQR1/SMPR1, fire ADSTART,
/// and read DR via `current_sample`.
fn read_adc1_ch5(adc1: &mut hal::adc::Adc<stm32::ADC1, Configured>) -> u16 {
    unsafe {
        let r = &*stm32::ADC1::ptr();
        // SQR1: L=0 (1 conversion), SQ1=5 (channel 5, bits 10:6)
        r.sqr1().write(|w| w.bits(5u32 << 6));
        // SMPR1: SMP5=0b111 = 640.5 cycles (bits 17:15)
        r.smpr1()
            .modify(|r, w| w.bits((r.bits() & !(7u32 << 15)) | (7u32 << 15)));
        r.isr().write(|w| w.eoc().clear());
        r.cr().modify(|_, w| w.adstart().set_bit());
        while r.isr().read().eoc().bit_is_clear() {}
    }
    adc1.current_sample()
}

/// Start ADC2 continuous 3-channel scan (ch5/B, ch14/C, ch17/A) → DMA1_CH1 circular.
/// ADC2 must already be enabled by the HAL before calling this.
/// After this returns, ADC2_DMA_BUF is refreshed at ~65 kHz per triplet.
unsafe fn start_adc2_scan_dma() {
    unsafe {
        (*stm32::RCC::ptr())
            .ahb1enr()
            .modify(|_, w| w.dma1en().set_bit().dmamux1en().set_bit());

        let adc2 = &*stm32::ADC2::ptr();

        if adc2.cr().read().adstart().bit_is_set() {
            adc2.cr().modify(|_, w| w.adstp().set_bit());
            while adc2.cr().read().adstp().bit_is_set() {}
        }

        // 3-channel sequence: SQ1=ch17(A) SQ2=ch14(C) SQ3=ch5(B), L=2 (3 conversions)
        // A sampled first so its conversion completes within the low-duty ON-window.
        adc2.sqr1()
            .write(|w| w.l().bits(2).sq1().bits(17).sq2().bits(14).sq3().bits(5));

        // Sample time 6.5 cycles: fits all 3 channels within ~8% duty ON-window.
        adc2.smpr1().modify(|_, w| w.smp5().cycles6_5());
        adc2.smpr2()
            .modify(|_, w| w.smp14().cycles6_5().smp17().cycles6_5());

        // CFGR: add continuous + DMA circular (keep existing resolution/alignment)
        adc2.cfgr()
            .modify(|_, w| w.cont().set_bit().dmaen().set_bit().dmacfg().set_bit());

        // DMA1 CH1: ADC2_DR → ADC2_DMA_BUF, 3 elements, 16-bit, circular
        let dma = &*stm32::DMA1::ptr();
        let ch = dma.ch1();
        ch.cr().write(|w| w.bits(0)); // disable
        ch.par().write(|w| w.bits(adc2.dr().as_ptr() as u32));
        ch.mar()
            .write(|w| w.bits(core::ptr::addr_of!(ADC2_DMA_BUF) as u32));
        ch.ndtr().write(|w| w.bits(3));
        // CIRC(5)|MINC(7)|PSIZE=16(0b01<<8)|MSIZE=16(0b01<<10)
        ch.cr()
            .write(|w| w.bits((1 << 5) | (1 << 7) | (0b01 << 8) | (0b01 << 10)));

        // DMAMUX1 ch0 (→ DMA1_CH1) = ADC2 request ID 36
        (*stm32::DMAMUX::ptr())
            .ccr(0)
            .write(|w| w.dmareq_id().bits(36));

        // Enable DMA then start ADC
        ch.cr().modify(|r, w| w.bits(r.bits() | 1));
        adc2.cr().modify(|_, w| w.adstart().set_bit());
    }
}

/// Switch ADC2 between two genuinely different acquisition backends.
///
/// triggered=false — Async legacy (UNFUCK2):
///   ADC2 is idle; TIM7 ISR performs a fresh single-channel SW conversion every tick.
///   Produces true 96 kHz independent samples. DMA channel stays disabled.
///   The ring shows the real aliasing/clamping pattern for observability.
///
/// triggered=true — Triggered triplet:
///   TIM1_TRGO (CCR4=1 → OC4REF pulse at CNT=0) triggers one 3-channel ADC2 scan per
///   20 kHz PWM period. DMA fills ADC2_DMA_BUF[B,C,A]. TIM7 reads the latest triggered
///   sample, producing a 96 kHz-rate staircase (same value ~4.8× per trigger).
///
/// Bit positions follow RM0440 G4 ADC CFGR (UNFUCK1):
///   EXTSEL[4:0] = bits [9:5],  EXTEN[1:0] = bits [11:10],  CONT = bit 13
unsafe fn configure_adc_capture(triggered: bool) {
    unsafe {
        let adc2 = &*stm32::ADC2::ptr();
        let dma = &*stm32::DMA1::ptr();
        let ch = dma.ch1();
        let t1 = &*stm32::TIM1::ptr();

        // Stop any in-progress conversion
        if adc2.cr().read().adstart().bit_is_set() {
            adc2.cr().modify(|_, w| w.adstp().set_bit());
            while adc2.cr().read().adstp().bit_is_set() {}
        }
        // Disable DMA channel and clear flags in all paths
        ch.cr().modify(|r, w| w.bits(r.bits() & !1));
        dma.ifcr().write(|w| w.bits(0x0F));
        adc2.isr().write(|w| w.bits(0xFFFF_FFFF));

        if triggered {
            // --- Triggered-triplet backend ---
            // TIM1 CC4: CCR4=1 so OC4REF pulses HIGH for exactly CNT=0 (one counter
            // tick ≈ 6 ns). Produces one TRGO rising edge per 20 kHz PWM period.
            // CCR4=0 makes OC4REF always LOW (CNT < 0 never true) → no TRGO.
            t1.ccr4().write(|w| w.ccr().bits(1));
            // CCMR2 output: CC4S=00 (output mode), OC4M=0b110 (PWM mode 1)
            t1.ccmr2_output()
                .modify(|r, w| w.bits((r.bits() & !(0x7000 | 0x300)) | (0b110u32 << 12)));
            // CR2 MMS[6:4] = 0b111 → OC4REF as TRGO
            t1.cr2()
                .modify(|r, w| w.bits((r.bits() & !0x70) | (0b111u32 << 4)));

            // SQR1: 3-channel sequence ch17(A)→ch14(C)→ch5(B), L=2 (3 conversions)
            adc2.sqr1()
                .write(|w| w.l().bits(2).sq1().bits(17).sq2().bits(14).sq3().bits(5));

            // CFGR: CONT=0, EXTEN=0b01 (rising), EXTSEL=9 (TIM1_TRGO), DMAEN|DMACFG
            // RM0440 G4: EXTSEL[4:0] at bits[9:5]=0x3E0, EXTEN[1:0] at bits[11:10]=0xC00
            adc2.cfgr().modify(|r, w| {
                w.bits(
                    (r.bits() & !(1u32 << 13 | 0xC00 | 0x3E0 | 0x3)) // clear CONT,EXTEN,EXTSEL,DMAEN/CFG
                        | (0b01u32 << 10)  // EXTEN = rising edge
                        | (9u32 << 5)      // EXTSEL = TIM1_TRGO
                        | (1u32 << 0)      // DMAEN
                        | (1u32 << 1), // DMACFG (circular)
                )
            });

            // Reset DMA transfer count, re-enable channel, arm ADC for first trigger
            ch.ndtr().write(|w| w.bits(3));
            ch.cr().modify(|r, w| w.bits(r.bits() | 1));
            adc2.cr().modify(|_, w| w.adstart().set_bit());
        } else {
            // --- Async-legacy backend ---
            // Restore MMS to 0b000 (no TRGO source from CC4)
            t1.cr2().modify(|r, w| w.bits(r.bits() & !0x70));

            // CFGR: CONT=0, EXTEN=0 (SW only), EXTSEL=0, DMAEN=0, DMACFG=0
            // TIM7 ISR will drive fresh per-tick SW conversions; no DMA needed.
            adc2.cfgr()
                .modify(|r, w| w.bits(r.bits() & !(1u32 << 13 | 0xC00 | 0x3E0 | 0x3)));
            // ADC2 stays enabled (ADEN=1); ISR will write SQR1 and fire ADSTART each tick
        }
    }
}

// ---------------------------------------------------------------------------
// 6-step BLDC helpers — B-G431B-ESC1 pin layout
//
// Phase A = TIM1 CH1/CH1N : PA8 (hi, AF6)  + PC13 (lo, AF4)
// Phase B = TIM1 CH2/CH2N : PA9 (hi, AF6)  + PA12 (lo, AF6)
// Phase C = TIM1 CH3/CH3N : PA10 (hi, AF6) + PB15 (lo, AF4)
//
// Floating a phase: switch both its pins from AF(0b10) → OUTPUT(0b01),
// then drive ODR=0 via BSRR.BR. TIM1 channels keep toggling internally
// but the MODER override isolates them from the pad — same mechanism as
// AM32 `phaseXFLOAT`.
// ---------------------------------------------------------------------------

/// Set MODER for all six motor pins. AF=0b10, OUTPUT=0b01.
/// Also resets ODR to 0 for any floated pins via BSRR.
unsafe fn set_phase_modes(float_a: bool, float_b: bool, float_c: bool) {
    const AF: u32 = 0b10;
    const OUT: u32 = 0b01;

    unsafe {
        // GPIOA: PA8(17:16)=CH1H, PA9(19:18)=CH2H, PA10(21:20)=CH3H, PA12(25:24)=CH2N
        let ga = &*stm32::GPIOA::ptr();
        let (ma8, ma9, ma10, ma12) = (
            if float_a { OUT } else { AF },
            if float_b { OUT } else { AF },
            if float_c { OUT } else { AF },
            if float_b { OUT } else { AF },
        );
        ga.moder().modify(|r, w| {
            w.bits(
                r.bits() & !(3 << 16 | 3 << 18 | 3 << 20 | 3 << 24)
                    | ma8 << 16
                    | ma9 << 18
                    | ma10 << 20
                    | ma12 << 24,
            )
        });

        // GPIOB: PB15(31:30)=CH3N
        let gb = &*stm32::GPIOB::ptr();
        let mb15 = if float_c { OUT } else { AF };
        gb.moder()
            .modify(|r, w| w.bits((r.bits() & !(3 << 30)) | mb15 << 30));

        // GPIOC: PC13(27:26)=CH1N
        let gc = &*stm32::GPIOC::ptr();
        let mc13 = if float_a { OUT } else { AF };
        gc.moder()
            .modify(|r, w| w.bits((r.bits() & !(3 << 26)) | mc13 << 26));

        // Drive floated pins LOW via BSRR bit-reset (BSRR.BR[n] = bit 16+n)
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
            ga.bsrr().write(|w| w.bits(ba));
        }
        if bb != 0 {
            gb.bsrr().write(|w| w.bits(bb));
        }
        if bc != 0 {
            gc.bsrr().write(|w| w.bits(bc));
        }
    }
}

/// Restore all six pins to AF so TIM1 drives them. Safe to call from main.
fn restore_all_af() {
    unsafe { set_phase_modes(false, false, false) }
}

/// 6-step commutation: two phases driven, one floating.
/// `sector` 0..5; `duty` is raw CCR value (0..ARR).
/// Standard BLDC table: sector → (high, low, float).
unsafe fn set_six_step(sector: u8, duty: u32) {
    const HIGH: [usize; 6] = [0, 0, 1, 1, 2, 2];
    const LOW: [usize; 6] = [1, 2, 2, 0, 0, 1];
    let s = (sector % 6) as usize;
    let hi = HIGH[s];
    let lo = LOW[s];

    unsafe {
        let t1 = &*stm32::TIM1::ptr();
        t1.ccr1()
            .write(|w| w.ccr().bits(if hi == 0 { duty } else { 0 }));
        t1.ccr2()
            .write(|w| w.ccr().bits(if hi == 1 { duty } else { 0 }));
        t1.ccr3()
            .write(|w| w.ccr().bits(if hi == 2 { duty } else { 0 }));
        set_phase_modes(hi != 0 && lo != 0, hi != 1 && lo != 1, hi != 2 && lo != 2);
    }
}

// ---------------------------------------------------------------------------

struct BoardInit {
    cp: cortex_m::Peripherals,
    clocks: hal::rcc::Clocks,
    rcc: hal::rcc::Rcc,
}

fn board_init(cp: cortex_m::Peripherals, dp_rcc: stm32::RCC, dp_pwr: stm32::PWR) -> BoardInit {
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
    let clocks = rcc.clocks;
    rinz::panic::ensure_rtt();
    BoardInit { cp, clocks, rcc }
}

// ---------------------------------------------------------------------------

#[entry]
fn main() -> ! {
    let cp = cortex_m::Peripherals::take().unwrap();
    let dp = stm32::Peripherals::take().unwrap();

    let BoardInit {
        mut cp,
        clocks,
        mut rcc,
    } = board_init(cp, dp.RCC, dp.PWR);

    rprintln!(
        "motor_tester: sys_clk={} apb1={}",
        clocks.sys_clk.raw(),
        clocks.apb1_clk.raw()
    );

    let gpioa = dp.GPIOA.split(&mut rcc);
    let gpiob = dp.GPIOB.split(&mut rcc);
    let gpioc = dp.GPIOC.split(&mut rcc);

    // PC6 = red STATUS LED
    let mut led = gpioc.pc6.into_push_pull_output();

    // ADC + OPAMP — before TIMER.start() so AsmDelay can be used freely.
    // ADC1: VBUS (PA0/IN1), phase-U current via OPAMP1 (internal), NTC (PB14/IN5 via PAC)
    // ADC2: BEMF1 (PA4/IN17), BEMF2 (PC4/IN5), BEMF3 (PB11/IN14) — all via PAC reads
    // GPIO_BEMF = PB5 driven LOW → enables the three-phase BEMF resistor divider network.
    let pa0_vbus = gpioa.pa0.into_analog();
    let pa1_isns = gpioa.pa1.into_analog();
    let _pb14_ntc = gpiob.pb14.into_analog();
    let _pa4_bemf1 = gpioa.pa4.into_analog();
    let _pc4_bemf2 = gpioc.pc4.into_analog();
    let _pb11_bemf3 = gpiob.pb11.into_analog();
    let mut gpio_bemf = gpiob.pb5.into_push_pull_output();
    gpio_bemf.set_low();
    let (opamp1, ..) = dp.OPAMP.split(&mut rcc);
    let opamp1_pga = opamp1.pga(pa1_isns, Gain::Gain16);
    let adc12_common = dp
        .ADC12_COMMON
        .claim(hal::adc::config::ClockMode::AdcHclkDiv4, &mut rcc);
    let mut adc1 = adc12_common.claim_and_configure(
        dp.ADC1,
        hal::adc::config::AdcConfig::default(),
        &mut AsmDelay,
    );
    let _adc2 = adc12_common.claim_and_configure(
        dp.ADC2,
        hal::adc::config::AdcConfig::default(),
        &mut AsmDelay,
    );
    unsafe { start_adc2_scan_dma() }; // one-time DMA routing init (DMAMUX, PAR, MAR, CIRC)
    unsafe { configure_adc_capture(false) }; // async until TIM1 is running

    // USART2: PB3=TX, PB4=RX, 115200 8N1
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
        "motor_tester ready  f/v=±10Hz d/c=±1Hz a/z=±1amp w=off q=reset i=info\r"
    )
    .ok();

    // TIM1: 20 kHz center-aligned complementary PWM, 100 ns dead-time.
    // High-side CH1/2/3 on PA8/PA9/PA10 (AF6).
    // Low-side CH1N/2N/3N on PC13 (AF4) / PA12 (AF6) / PB15 (AF4).
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
        .frequency(20_000u32.Hz())
        .with_deadtime(100u32.nanos())
        .center_aligned()
        .finalize();

    let mut c1 = c1.into_complementary(gpioc.pc13.into_alternate::<4>());
    let mut c2 = c2.into_complementary(gpioa.pa12.into_alternate::<6>());
    let mut c3 = c3.into_complementary(gpiob.pb15.into_alternate::<4>());

    let max_duty = c1.max_duty_cycle() as u32;
    let half = max_duty / 2;

    c1.enable();
    c2.enable();
    c3.enable();
    unsafe { configure_adc_capture(true) }; // TIM1 running — switch to triggered-triplet

    // Print ADC timing so we know what clock/sample-time is actually configured.
    // All values derived from HAL clocks so they stay correct if PLL or div changes.
    {
        // HCLK = SYSCLK (AHB prescaler=1); ADC clock = HCLK/4 (AdcHclkDiv4).
        let cpu_hz = clocks.sys_clk.raw() as u64;
        let adc_hz = cpu_hz / 4;
        // SMPR=1 → 6.5 sample + 12.5 conversion = 19 cycles per channel.
        let ns_per_ch = 19_000_000_000u64 / adc_hz;
        let samp_ns = 6_500_000_000u64 / adc_hz;
        // 3rd channel's sample ends 2 full conversions after TRGO plus one sample hold.
        let third_end_ns = 2 * ns_per_ch + samp_ns;
        // Minimum amp% so the 3rd-channel sample ends before the PWM half-window closes.
        // half_window_ns = (arr * amp * 2 / 300) / cpu_hz * 1e9
        // → min_amp = ceil(third_end_ns * 300 * cpu_hz / (arr * 2 * 1e9))
        let arr = max_duty as u64;
        let min_amp =
            (third_end_ns * 300 * cpu_hz + arr * 2_000_000_000 - 1) / (arr * 2_000_000_000);
        writeln!(
            tx,
            "ADC: clk={}.{}MHz samp=6.5cy {}ns/ch 3ch-end={}ns min_amp={}%\r",
            adc_hz / 1_000_000,
            (adc_hz % 1_000_000) / 100_000,
            ns_per_ch,
            third_end_ns,
            min_amp,
        )
        .ok();
    }

    // Park duties at midpoint — no current while stopped.
    let _ = c1.set_duty_cycle(half as u16);
    let _ = c2.set_duty_cycle(half as u16);
    let _ = c3.set_duty_cycle(half as u16);

    // TIM7: sinewave heartbeat at DRIVE_HZ.
    rinz::tim7_drive::init(dp.TIM7, DRIVE_HZ, &clocks);
    unsafe { NVIC::unmask(stm32::Interrupt::TIM7) };

    TIMER.start(&mut cp.SYST);

    let mut idle = IdleLoop::new();
    idle.calibrate(SECOND_TICKS, &now_u64);

    let mut epoch: u32 = 0;
    let mut epoch_start = now_u64();
    let mut last_busy: u8 = 0;

    loop {
        let next_epoch = epoch_start + SECOND_TICKS;
        let mut next_microloop = epoch_start + MICROLOOP_TICKS;

        while now_u64() < next_epoch {
            idle.run_until(next_microloop, &now_u64);

            // --- command poll (non-blocking) ---
            if rx.read_ready().unwrap_or(false) {
                let mut buf = [0u8; 1];
                if rx.read(&mut buf).is_ok() {
                    match buf[0] {
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
                        b'd' => {
                            let hz = (ELECTRICAL_HZ.load(Ordering::Relaxed) + 1).min(FREQ_MAX);
                            ELECTRICAL_HZ.store(hz, Ordering::Relaxed);
                            writeln!(tx, "freq={}Hz\r", hz).ok();
                        }
                        b'c' => {
                            let hz = ELECTRICAL_HZ
                                .load(Ordering::Relaxed)
                                .saturating_sub(1)
                                .max(FREQ_MIN);
                            ELECTRICAL_HZ.store(hz, Ordering::Relaxed);
                            writeln!(tx, "freq={}Hz\r", hz).ok();
                        }
                        b'a' => {
                            let amp = (AMPLITUDE.load(Ordering::Relaxed) + 1).min(AMP_CAP);
                            AMPLITUDE.store(amp, Ordering::Relaxed);
                            writeln!(tx, "amp={}%\r", amp).ok();
                        }
                        b'z' => {
                            let amp = AMPLITUDE.load(Ordering::Relaxed).saturating_sub(1);
                            AMPLITUDE.store(amp, Ordering::Relaxed);
                            writeln!(tx, "amp={}%\r", amp).ok();
                        }
                        b'l' => {
                            if ADC_TRIGGERED.load(Ordering::Relaxed) {
                                writeln!(tx, "l: only in async mode ('s' to switch)\r").ok();
                            } else {
                                let f_hz = ELECTRICAL_HZ.load(Ordering::Relaxed);
                                if f_hz <= 100 {
                                    writeln!(
                                        tx,
                                        "l: disabled below 100 Hz (current {} Hz)\r",
                                        f_hz
                                    )
                                    .ok();
                                } else {
                                    let starts: [[u32; 6]; 3];
                                    let hist_head: usize;
                                    let hist_count: usize;
                                    NVIC::mask(stm32::Interrupt::TIM7);
                                    unsafe {
                                        starts = REV_SECTOR_STARTS;
                                        hist_head = REV_HIST_HEAD;
                                        hist_count = REV_HIST_COUNT;
                                    }
                                    if hist_count < 3 {
                                        unsafe { NVIC::unmask(stm32::Interrupt::TIM7) };
                                        writeln!(
                                            tx,
                                            "l: only {} complete revs (need 3)\r",
                                            hist_count
                                        )
                                        .ok();
                                    } else {
                                        let idx0 = hist_head;
                                        let idx1 = (hist_head + 1) % 3;
                                        let idx2 = (hist_head + 2) % 3;
                                        let win_start = starts[idx0][0];
                                        let win_len =
                                            starts[idx2][0].wrapping_sub(win_start) as usize;
                                        if win_len == 0 || win_len > SNAP_LEN {
                                            unsafe { NVIC::unmask(stm32::Interrupt::TIM7) };
                                            writeln!(tx, "l: window {} out of range\r", win_len)
                                                .ok();
                                        } else {
                                            unsafe {
                                                for i in 0..win_len {
                                                    let src = (win_start as usize).wrapping_add(i)
                                                        & RAW_SAMPLE_MASK;
                                                    SNAP_BUF[i] = RAW_SAMPLE_BUF[src];
                                                }
                                                NVIC::unmask(stm32::Interrupt::TIM7);
                                            }
                                            writeln!(tx, "l: 2 revs @ {} Hz\r", f_hz).ok();
                                            for row in 0..12usize {
                                                let rev = row / 6;
                                                let sec = row % 6;
                                                let rev_idx = if rev == 0 { idx0 } else { idx1 };
                                                let nxt_idx = if rev == 0 { idx1 } else { idx2 };
                                                let row_start = starts[rev_idx][sec];
                                                let row_end = if sec < 5 {
                                                    starts[rev_idx][sec + 1]
                                                } else {
                                                    starts[nxt_idx][0]
                                                };
                                                let n = row_end.wrapping_sub(row_start) as usize;
                                                let rel =
                                                    row_start.wrapping_sub(win_start) as usize;
                                                write!(tx, "[r{} s{}]: ", rev, sec).ok();
                                                if n == 0 || rel + n > win_len {
                                                    writeln!(tx, "(invalid)\r").ok();
                                                    continue;
                                                }
                                                unsafe {
                                                    for i in 0..n {
                                                        let v = SNAP_BUF[rel + i];
                                                        write!(
                                                            tx,
                                                            "{}{}",
                                                            b"0123456789abcdef"
                                                                [((v >> 8) & 0xF) as usize]
                                                                as char,
                                                            b"0123456789abcdef"
                                                                [((v >> 4) & 0xF) as usize]
                                                                as char,
                                                        )
                                                        .ok();
                                                    }
                                                }
                                                writeln!(tx, "\r").ok();
                                            }
                                        }
                                    }
                                }
                            } // end async-only else
                        }
                        b'm' => {
                            if ADC_TRIGGERED.load(Ordering::Relaxed) {
                                writeln!(tx, "m: only in async mode ('s' to switch)\r").ok();
                            } else {
                                let f_hz = ELECTRICAL_HZ.load(Ordering::Relaxed);
                                if f_hz <= 100 {
                                    writeln!(
                                        tx,
                                        "m: disabled below 100 Hz (current {} Hz)\r",
                                        f_hz
                                    )
                                    .ok();
                                } else {
                                    let starts: [[u32; 6]; 3];
                                    let hist_head: usize;
                                    let hist_count: usize;
                                    NVIC::mask(stm32::Interrupt::TIM7);
                                    unsafe {
                                        starts = REV_SECTOR_STARTS;
                                        hist_head = REV_HIST_HEAD;
                                        hist_count = REV_HIST_COUNT;
                                    }
                                    if hist_count < 3 {
                                        unsafe { NVIC::unmask(stm32::Interrupt::TIM7) };
                                        writeln!(
                                            tx,
                                            "m: only {} complete revs (need 3)\r",
                                            hist_count
                                        )
                                        .ok();
                                    } else {
                                        let idx0 = hist_head;
                                        let idx1 = (hist_head + 1) % 3;
                                        let idx2 = (hist_head + 2) % 3;
                                        let win_start = starts[idx0][0];
                                        let win_len =
                                            starts[idx2][0].wrapping_sub(win_start) as usize;
                                        if win_len == 0 || win_len > SNAP_LEN {
                                            unsafe { NVIC::unmask(stm32::Interrupt::TIM7) };
                                            writeln!(tx, "m: window {} out of range\r", win_len)
                                                .ok();
                                        } else {
                                            unsafe {
                                                for i in 0..win_len {
                                                    let src = (win_start as usize).wrapping_add(i)
                                                        & RAW_SAMPLE_MASK;
                                                    SNAP_BUF[i] = RAW_SAMPLE_BUF[src];
                                                }
                                                NVIC::unmask(stm32::Interrupt::TIM7);
                                            }
                                            writeln!(tx, "m: 2 revs @ {} Hz\r", f_hz).ok();
                                            let hex = b"0123456789abcdef";
                                            for row in 0..12usize {
                                                let rev = row / 6;
                                                let sec = row % 6;
                                                let rev_idx = if rev == 0 { idx0 } else { idx1 };
                                                let nxt_idx = if rev == 0 { idx1 } else { idx2 };
                                                let row_start = starts[rev_idx][sec];
                                                let row_end = if sec < 5 {
                                                    starts[rev_idx][sec + 1]
                                                } else {
                                                    starts[nxt_idx][0]
                                                };
                                                let n = row_end.wrapping_sub(row_start) as usize;
                                                let rel =
                                                    row_start.wrapping_sub(win_start) as usize;
                                                write!(tx, "[r{} s{}]: ", rev, sec).ok();
                                                if n == 0 || rel + n > win_len {
                                                    writeln!(tx, "(invalid)\r").ok();
                                                    continue;
                                                }
                                                unsafe {
                                                    for i in 0..n {
                                                        let v = SNAP_BUF[rel + i];
                                                        write!(
                                                            tx,
                                                            "{}{} ",
                                                            hex[((v >> 8) & 0xF) as usize] as char,
                                                            hex[((v >> 4) & 0xF) as usize] as char,
                                                        )
                                                        .ok();
                                                    }
                                                }
                                                writeln!(tx, "\r").ok();
                                            }
                                        }
                                    }
                                }
                            } // end async-only else
                        }
                        b'p' => {
                            let cur = RING_CHAN_FIXED.load(Ordering::Relaxed);
                            let next = match cur {
                                17 => 5,
                                5 => 14,
                                _ => 17,
                            };
                            RING_CHAN_FIXED.store(next, Ordering::Relaxed);
                            RAW_RING_RESET.store(true, Ordering::Relaxed);
                            let name = match next {
                                5 => "B (ch5/PC4)",
                                14 => "C (ch14/PB11)",
                                _ => "A (ch17/PA4)",
                            };
                            writeln!(tx, "ring_chan={}\r", name).ok();
                        }
                        b'M' => {
                            let was = SIX_STEP.load(Ordering::Relaxed);
                            if was {
                                // returning to sine: restore AF so all three phases are driven
                                restore_all_af();
                            }
                            SIX_STEP.store(!was, Ordering::Relaxed);
                            RAW_RING_RESET.store(true, Ordering::Relaxed);
                            let name = if was { "sine" } else { "six-step" };
                            writeln!(tx, "mode={}\r", name).ok();
                        }
                        b'w' => {
                            RUNNING.store(false, Ordering::Relaxed);
                            restore_all_af(); // un-float any phase left by six-step
                            RAW_RING_RESET.store(true, Ordering::Relaxed);
                            TRIP_FROZEN.store(false, Ordering::Relaxed);
                            TRIP_FREEZE_REQUESTED.store(false, Ordering::Relaxed);
                            let _ = c1.set_duty_cycle(half as u16);
                            let _ = c2.set_duty_cycle(half as u16);
                            let _ = c3.set_duty_cycle(half as u16);
                            writeln!(tx, "kill\r").ok();
                            rprintln!("KILL");
                        }
                        b'q' => {
                            restore_all_af();
                            SIX_STEP.store(SIX_STEP_START, Ordering::Relaxed);
                            ELECTRICAL_HZ.store(FREQ_START, Ordering::Relaxed);
                            AMPLITUDE.store(AMP_START, Ordering::Relaxed);
                            RUNNING.store(true, Ordering::Relaxed);
                            RAW_RING_RESET.store(true, Ordering::Relaxed);
                            let mode = if SIX_STEP_START { "six-step" } else { "sine" };
                            writeln!(
                                tx,
                                "reset: freq={}Hz amp={}% mode={}\r",
                                FREQ_START, AMP_START, mode
                            )
                            .ok();
                            rprintln!("reset");
                        }
                        b'i' => {
                            let raw = adc1.convert(&pa0_vbus, SampleTime::Cycles_640_5);
                            let adc_mv = adc1.sample_to_millivolts(raw) as u32;
                            let vbus_mv = adc_mv * 1039 / 100;

                            let raw_i = adc1.convert(&opamp1_pga, SampleTime::Cycles_640_5);
                            let opamp_mv = adc1.sample_to_millivolts(raw_i) as u32;
                            let iu_ma = opamp_mv * 1000 / 48;

                            let raw_t = read_adc1_ch5(&mut adc1);
                            let temp_mv = adc1.sample_to_millivolts(raw_t) as i32;
                            // NTC pull-down: V decreases with T.
                            // MC SDK: T = T0 + (V0 - V_out) / dV_dT = 25 + (1400 - V_mV) / 19
                            let temp_c = 25 + (1400 - temp_mv) / 19;

                            writeln!(
                                tx,
                                "vbus={}.{}V  iu={}mA (phase-U proxy)  temp={}°C  busy={}%\r",
                                vbus_mv / 1000,
                                (vbus_mv % 1000) / 100,
                                iu_ma,
                                temp_c,
                                last_busy,
                            )
                            .ok();
                            rprintln!(
                                "vbus={}mV opamp1={}mV iu={}mA temp={}mV({}C)",
                                vbus_mv,
                                opamp_mv,
                                iu_ma,
                                temp_mv,
                                temp_c,
                            );
                        }
                        b's' => {
                            let was = ADC_TRIGGERED.load(Ordering::Relaxed);
                            let now = !was;
                            ADC_TRIGGERED.store(now, Ordering::Relaxed);
                            unsafe { configure_adc_capture(now) };
                            RAW_RING_RESET.store(true, Ordering::Relaxed);
                            TRIP_FROZEN.store(false, Ordering::Relaxed);
                            TRIP_FREEZE_REQUESTED.store(false, Ordering::Relaxed);
                            let name = if now {
                                "triggered-triplet (TIM1_TRGO DMA staircase)"
                            } else {
                                "async-legacy (fresh 96kHz direct samples)"
                            };
                            writeln!(tx, "adc_mode={}\r", name).ok();
                        }
                        b'n' => {
                            // Register state dump: ADC2 + DMA1_CH1 + DMAMUX + live DR.
                            // Use this to diagnose DMA/ADC2 config issues.
                            unsafe {
                                let adc2 = &*stm32::ADC2::ptr();
                                let dma = &*stm32::DMA1::ptr();
                                let ch = dma.ch1();
                                let dmamux = &*stm32::DMAMUX::ptr();

                                let adc2_cr = adc2.cr().read().bits();
                                let adc2_cfgr = adc2.cfgr().read().bits();
                                let adc2_isr = adc2.isr().read().bits();
                                let adc2_sqr1 = adc2.sqr1().read().bits();
                                let adc2_dr = adc2.dr().read().rdata().bits();
                                let dma_cr = ch.cr().read().bits();
                                let dma_ndtr = ch.ndtr().read().bits();
                                let dma_par = ch.par().read().bits();
                                let dma_mar = ch.mar().read().bits();
                                let dma_isr = dma.isr().read().bits();
                                let mux_ccr0 = dmamux.ccr(0).read().bits();
                                let buf_addr = core::ptr::addr_of!(ADC2_DMA_BUF) as u32;
                                let buf0 = core::ptr::addr_of!(ADC2_DMA_BUF)
                                    .cast::<u16>()
                                    .add(0)
                                    .read_volatile();
                                let buf1 = core::ptr::addr_of!(ADC2_DMA_BUF)
                                    .cast::<u16>()
                                    .add(1)
                                    .read_volatile();
                                let buf2 = core::ptr::addr_of!(ADC2_DMA_BUF)
                                    .cast::<u16>()
                                    .add(2)
                                    .read_volatile();
                                writeln!(
                                    tx,
                                    "adc2 cr={:08x} cfgr={:08x} isr={:08x} sqr1={:08x} dr={:04x}\r",
                                    adc2_cr, adc2_cfgr, adc2_isr, adc2_sqr1, adc2_dr
                                )
                                .ok();
                                writeln!(
                                    tx,
                                    "dma1ch1 cr={:08x} ndtr={} par={:08x} mar={:08x} isr={:08x}\r",
                                    dma_cr, dma_ndtr, dma_par, dma_mar, dma_isr
                                )
                                .ok();
                                writeln!(
                                    tx,
                                    "dmamux ccr0={:08x}  buf@{:08x} [{:04x},{:04x},{:04x}]\r",
                                    mux_ccr0, buf_addr, buf0, buf1, buf2
                                )
                                .ok();
                            }
                        }
                        b'e' => {
                            let triggered = ADC_TRIGGERED.load(Ordering::Relaxed);
                            if !triggered {
                                writeln!(tx, "e: only in triggered mode ('s' to switch)\r").ok();
                            } else if !RUNNING.load(Ordering::Relaxed) {
                                writeln!(tx, "e: motor not running\r").ok();
                            } else {
                                let f_hz = ELECTRICAL_HZ.load(Ordering::Relaxed);
                                // Request freeze at next rev boundary, wait for ISR to honour it.
                                TRIP_FROZEN.store(false, Ordering::Relaxed);
                                TRIP_FREEZE_REQUESTED.store(true, Ordering::Relaxed);
                                while !TRIP_FROZEN.load(Ordering::Relaxed) {
                                    cortex_m::asm::nop();
                                }
                                // Buffers frozen; ISR will not write until we clear TRIP_FROZEN.
                                let counts: [usize; 6] = unsafe { TRIP_COUNTS_SNAP };
                                let avg_sps = counts.iter().sum::<usize>() / 6;
                                writeln!(
                                    tx,
                                    "e: trip 1 rev @ {} Hz  ~{} samp/sec\r",
                                    f_hz, avg_sps
                                )
                                .ok();

                                const PNAME: [&str; 3] = ["A", "B", "C"];
                                const RAMP_FALLING: [bool; 6] =
                                    [true, false, true, false, true, false];
                                const BLANK: usize = 2; // 2 × 50 µs = 100 µs at 20 kHz
                                const ZC_CONFIRM: usize = 2;
                                const ZC_LATE: usize = usize::MAX;
                                const ZC_EARLY: usize = usize::MAX - 1;
                                let hex = b"0123456789abcdef";

                                for sec in 0..6usize {
                                    let n = counts[sec];
                                    let base = sec * MAX_TRIP_SPS;
                                    let hi = SIX_STEP_HIGH[sec];
                                    let lo = SIX_STEP_LOW[sec];
                                    let fl = 3 - hi - lo; // float phase: A=0,B=1,C=2

                                    // Float-phase BEMF from the appropriate buffer.
                                    let get_fl = |i: usize| -> u16 {
                                        unsafe {
                                            match fl {
                                                0 => RAW_SAMPLE_BUF[base + i],
                                                2 => SNAP_BUF[base + i],
                                                _ => RAW_NEUT_BUF[base + i],
                                            }
                                        }
                                    };
                                    let get_n =
                                        |i: usize| -> u16 { unsafe { SNAP_NEUT_BUF[base + i] } };

                                    // ZC scan on float phase.
                                    let rf = RAMP_FALLING[sec];
                                    let zc_i: usize = if n > BLANK + ZC_CONFIRM {
                                        let xd = |i: usize| -> bool {
                                            let vb = get_fl(i);
                                            let vn = get_n(i);
                                            if vn == 0 || vb == 0 {
                                                return false;
                                            }
                                            if rf { vb <= vn } else { vb >= vn }
                                        };
                                        if (BLANK..(BLANK + ZC_CONFIRM)).all(xd) {
                                            ZC_LATE
                                        } else {
                                            let mut found = ZC_EARLY;
                                            'scan: for i in BLANK..n.saturating_sub(ZC_CONFIRM - 1)
                                            {
                                                if (i..(i + ZC_CONFIRM)).all(xd) {
                                                    found = i;
                                                    break 'scan;
                                                }
                                            }
                                            found
                                        }
                                    } else {
                                        ZC_EARLY
                                    };

                                    // Header line.
                                    write!(
                                        tx,
                                        "[s{} {}=hi {}=lo {}=fl ",
                                        sec, PNAME[hi], PNAME[lo], PNAME[fl]
                                    )
                                    .ok();
                                    if zc_i == ZC_LATE {
                                        writeln!(tx, "ZC<0]:\r").ok();
                                    } else if zc_i < ZC_EARLY {
                                        writeln!(tx, "ZC@{}/{}]:\r", zc_i, n).ok();
                                    } else {
                                        writeln!(tx, "ZC>{}]:\r", n).ok();
                                    }

                                    // Float BEMF row.
                                    write!(tx, "  {}: ", PNAME[fl]).ok();
                                    for i in 0..n {
                                        if i == zc_i {
                                            write!(tx, "*").ok();
                                        }
                                        let v = get_fl(i);
                                        write!(
                                            tx,
                                            "{}{} ",
                                            hex[((v >> 8) & 0xF) as usize] as char,
                                            hex[((v >> 4) & 0xF) as usize] as char,
                                        )
                                        .ok();
                                    }
                                    writeln!(tx, "\r").ok();

                                    // Neutral row.
                                    write!(tx, "  N: ").ok();
                                    for i in 0..n {
                                        let v = get_n(i);
                                        write!(
                                            tx,
                                            "{}{} ",
                                            hex[((v >> 8) & 0xF) as usize] as char,
                                            hex[((v >> 4) & 0xF) as usize] as char,
                                        )
                                        .ok();
                                    }
                                    writeln!(tx, "\r").ok();
                                }

                                // Unfreeze: ISR resumes writing on next tick.
                                TRIP_FROZEN.store(false, Ordering::Relaxed);
                            }
                        }
                        _ => {}
                    }
                }
            }

            next_microloop += MICROLOOP_TICKS;
        }

        last_busy = idle.busy_percentage(idle.latch());
        led.toggle();
        rprintln!(
            "epoch={} busy={}% f={}Hz amp={} run={}",
            epoch,
            last_busy,
            ELECTRICAL_HZ.load(Ordering::Relaxed),
            AMPLITUDE.load(Ordering::Relaxed),
            RUNNING.load(Ordering::Relaxed) as u8,
        );
        epoch = epoch.wrapping_add(1);
        epoch_start = next_epoch;
    }
}

// ---------------------------------------------------------------------------
// TIM7 ISR — motor commutation heartbeat at DRIVE_HZ
// ---------------------------------------------------------------------------

#[allow(non_snake_case)]
#[unsafe(no_mangle)]
extern "C" fn TIM7() {
    rinz::tim7_drive::clear_update_flag();

    static mut STEP: u32 = 0;
    static mut PHASE_FRAC: u32 = 0;
    static mut PREV_SECTOR: u8 = 255; // 255 = uninitialized sentinel

    if !RUNNING.load(Ordering::Relaxed) {
        return;
    }

    let electrical_hz = ELECTRICAL_HZ.load(Ordering::Relaxed);
    let amplitude = AMPLITUDE.load(Ordering::Relaxed);
    let six_step = SIX_STEP.load(Ordering::Relaxed);

    // Fractional phase accumulator: advances electrical_hz×48 sub-steps per
    // tick, draining DRIVE_HZ sub-steps per integer step.
    unsafe {
        if RAW_RING_RESET.swap(false, Ordering::Relaxed) {
            PREV_SECTOR = 255;
            CUR_REV_SECTOR_STARTS = [u32::MAX; 6];
            REV_HIST_HEAD = 0;
            REV_HIST_COUNT = 0;
            TRIP_COUNTS = [0usize; 6];
            TRIP_COUNTS_SNAP = [0usize; 6];
            TRIP_FRAC = 0;
        }

        PHASE_FRAC += electrical_hz * 48;
        let advance = PHASE_FRAC / DRIVE_HZ;
        PHASE_FRAC %= DRIVE_HZ;
        STEP = (STEP + advance) % 48;

        let arr = (*stm32::TIM1::ptr()).arr().read().arr().bits() as u32;

        if six_step {
            // 48 steps / 6 sectors = 8 steps per sector.
            // Scale duty by 2/3 (~0.667). Pure peak-matching gives 3/4, but six-step's
            // discrete commutation has an inherent open-loop sync advantage; 2/3 partially
            // compensates so the modes feel closer at the same amplitude setting.
            let sector = (STEP / 8) as u8;
            let duty = arr * amplitude * 2 / (100 * 3);
            set_six_step(sector, duty);

            if PREV_SECTOR != sector {
                let now_t = now_u64() as u32;
                let rev_wrapped = PREV_SECTOR != 255 && PREV_SECTOR == 5 && sector == 0;
                // l-path: record sample-index boundary for the sector starting now.
                if electrical_hz > 100 {
                    let wr = RAW_SAMPLE_WR.load(Ordering::Relaxed);
                    if rev_wrapped {
                        // Push the completed revolution (sectors 0..5) to history.
                        let tail = (REV_HIST_HEAD + REV_HIST_COUNT) % 3;
                        REV_SECTOR_STARTS[tail] = CUR_REV_SECTOR_STARTS;
                        if REV_HIST_COUNT < 3 {
                            REV_HIST_COUNT += 1;
                        } else {
                            REV_HIST_HEAD = (REV_HIST_HEAD + 1) % 3;
                        }
                        CUR_REV_SECTOR_STARTS = [u32::MAX; 6];
                    }
                    CUR_REV_SECTOR_STARTS[sector as usize] = wr;
                }
                let _ = now_t;
                PREV_SECTOR = sector;
                // Triplet buffer: reset per-sector counter on sector entry.
                if ADC_TRIGGERED.load(Ordering::Relaxed) {
                    if rev_wrapped && TRIP_FREEZE_REQUESTED.load(Ordering::Relaxed) {
                        TRIP_COUNTS_SNAP = TRIP_COUNTS; // snapshot complete rev before reset
                        TRIP_FROZEN.store(true, Ordering::Relaxed);
                        TRIP_FREEZE_REQUESTED.store(false, Ordering::Relaxed);
                    }
                    TRIP_COUNTS[sector as usize] = 0;
                }
            }

            // ADC sampling for raw ring ('l'/'m'/'e' dumps).
            if electrical_hz > 100 {
                let s = (sector % 6) as usize;
                let triggered = ADC_TRIGGERED.load(Ordering::Relaxed);

                if triggered {
                    if !TRIP_FROZEN.load(Ordering::Relaxed) {
                        // Fractional accumulator: fires once per TIM1 trigger on average
                        // (DRIVE_HZ/20000 = 4.8 TIM7 ticks per write). Carry-over between
                        // sectors keeps the cadence uniform across sector boundaries.
                        TRIP_FRAC += 20_000;
                        if TRIP_FRAC >= DRIVE_HZ {
                            TRIP_FRAC -= DRIVE_HZ;
                            let cnt = TRIP_COUNTS[s];
                            if cnt < MAX_TRIP_SPS {
                                let dma = core::ptr::addr_of!(ADC2_DMA_BUF).cast::<u16>();
                                let idx = s * MAX_TRIP_SPS + cnt;
                                let va = dma.add(0).read_volatile(); // ch17 / A
                                let vc = dma.add(1).read_volatile(); // ch14 / C
                                let vb = dma.add(2).read_volatile(); // ch5  / B
                                let v_hi = dma.add(PHASE_TO_DMA[SIX_STEP_HIGH[s]]).read_volatile();
                                let v_lo = dma.add(PHASE_TO_DMA[SIX_STEP_LOW[s]]).read_volatile();
                                let vn = ((v_hi as u32 + v_lo as u32) >> 1) as u16;
                                RAW_SAMPLE_BUF[idx] = va;
                                SNAP_BUF[idx] = vc;
                                RAW_NEUT_BUF[idx] = vb;
                                SNAP_NEUT_BUF[idx] = vn;
                                TRIP_COUNTS[s] = cnt + 1;
                            }
                        }
                    }
                } else {
                    // Async-legacy: fresh single-channel SW conversion → rolling ring.
                    let fixed = RING_CHAN_FIXED.load(Ordering::Relaxed);
                    const FLOAT_CHAN: [u32; 6] = [14, 5, 17, 14, 5, 17];
                    let chan = if fixed != 0 { fixed } else { FLOAT_CHAN[s] };
                    let adc2 = &*stm32::ADC2::ptr();
                    adc2.sqr1().write(|w| w.bits(chan << 6));
                    adc2.cr().modify(|_, w| w.adstart().set_bit());
                    while adc2.isr().read().eoc().bit_is_clear() {}
                    let raw = adc2.dr().read().rdata().bits();
                    let wr = RAW_SAMPLE_WR.load(Ordering::Relaxed);
                    RAW_SAMPLE_BUF[wr as usize & RAW_SAMPLE_MASK] = raw;
                    RAW_NEUT_BUF[wr as usize & RAW_SAMPLE_MASK] = 0;
                    RAW_SAMPLE_WR.store(wr.wrapping_add(1), Ordering::Relaxed);
                }
            }
        } else {
            // Sine: CCR = half + (val−127)/127 * amplitude/100 * half
            // At amplitude=100 the swing reaches 0..ARR (peak LL = 0.75×Vbus).
            // +4 step offset shifts A's peak from 90° → 60°, aligning with the centre
            // of the six-step FWD plateau (0°–120°, centre at 60°).
            let half = (arr / 2) as i32;
            let amp = amplitude as i32;
            let va = SINE48[(STEP + 4) as usize % 48] as i32 - 127; // peak at 60°
            let vb = SINE48[(STEP + 36) as usize % 48] as i32 - 127; // B lags A 120°
            let vc = SINE48[(STEP + 20) as usize % 48] as i32 - 127; // C lags A 240°
            let scale = 127 * 100_i32;
            let duty_a = (half + va * amp * half / scale) as u32;
            let duty_b = (half + vb * amp * half / scale) as u32;
            let duty_c = (half + vc * amp * half / scale) as u32;
            let t1 = &*stm32::TIM1::ptr();
            t1.ccr1().write(|w| w.ccr().bits(duty_a));
            t1.ccr2().write(|w| w.ccr().bits(duty_b));
            t1.ccr3().write(|w| w.ccr().bits(duty_c));
        }
    }
}

// ---------------------------------------------------------------------------

#[exception]
fn SysTick() {
    TIMER.systick_handler();
}
