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

// ---------------------------------------------------------------------------
// Raw BEMF sample ring — 96 kHz, enabled when electrical_hz > 100 ('l' dump)
// ---------------------------------------------------------------------------

/// 4096 × (1/96000 s) ≈ 42.7 ms ≈ 4.3 revs at the 100 Hz floor.
const RAW_SAMPLE_LEN: usize = 4096;
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

/// Circular DMA destination for ADC2 continuous scan: [ch5/B, ch14/C, ch17/A].
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

/// Snapshot buffer for 'l'/'m' dumps. Filled under TIM7 mask, printed after unmask.
/// Sized for worst-case 2 revs at the 101 Hz floor: 2×96000/101 ≈ 1901 samples.
const SNAP_LEN: usize = 2048;
static mut SNAP_BUF: [u16; SNAP_LEN] = [0u16; SNAP_LEN];

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

        // 3-channel sequence: SQ1=ch5(B) SQ2=ch14(C) SQ3=ch17(A), L=2 (3 conversions)
        adc2.sqr1()
            .write(|w| w.bits((2u32 << 0) | (5u32 << 6) | (14u32 << 12) | (17u32 << 18)));

        // Sample time 24.5 cycles (value 3): ch5 SMPR1[17:15], ch14/ch17 SMPR2[14:12]/[23:21]
        adc2.smpr1()
            .modify(|r, w| w.bits((r.bits() & !(7 << 15)) | (3 << 15)));
        adc2.smpr2()
            .modify(|r, w| w.bits((r.bits() & !(7 << 12 | 7 << 21)) | (3 << 12) | (3 << 21)));

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
    unsafe { start_adc2_scan_dma() };

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
                            let f_hz = ELECTRICAL_HZ.load(Ordering::Relaxed);
                            if f_hz <= 100 {
                                writeln!(tx, "l: disabled below 100 Hz (current {} Hz)\r", f_hz)
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
                                    writeln!(tx, "l: only {} complete revs (need 3)\r", hist_count)
                                        .ok();
                                } else {
                                    let idx0 = hist_head;
                                    let idx1 = (hist_head + 1) % 3;
                                    let idx2 = (hist_head + 2) % 3;
                                    let win_start = starts[idx0][0];
                                    let win_len = starts[idx2][0].wrapping_sub(win_start) as usize;
                                    if win_len == 0 || win_len > SNAP_LEN {
                                        unsafe { NVIC::unmask(stm32::Interrupt::TIM7) };
                                        writeln!(tx, "l: window {} out of range\r", win_len).ok();
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
                                            let rel = row_start.wrapping_sub(win_start) as usize;
                                            write!(tx, "[r{} s{}]: ", rev, sec).ok();
                                            if n == 0 || rel + n > win_len {
                                                writeln!(tx, "(invalid)\r").ok();
                                                continue;
                                            }
                                            unsafe {
                                                for i in 0..n {
                                                    let nibble =
                                                        (SNAP_BUF[rel + i] >> 8) as u8 & 0xF;
                                                    write!(
                                                        tx,
                                                        "{}",
                                                        b"0123456789abcdef"[nibble as usize]
                                                            as char
                                                    )
                                                    .ok();
                                                }
                                            }
                                            writeln!(tx, "\r").ok();
                                        }
                                    }
                                }
                            }
                        }
                        b'm' => {
                            let f_hz = ELECTRICAL_HZ.load(Ordering::Relaxed);
                            if f_hz <= 100 {
                                writeln!(tx, "m: disabled below 100 Hz (current {} Hz)\r", f_hz)
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
                                    writeln!(tx, "m: only {} complete revs (need 3)\r", hist_count)
                                        .ok();
                                } else {
                                    let idx0 = hist_head;
                                    let idx1 = (hist_head + 1) % 3;
                                    let idx2 = (hist_head + 2) % 3;
                                    let win_start = starts[idx0][0];
                                    let win_len = starts[idx2][0].wrapping_sub(win_start) as usize;
                                    if win_len == 0 || win_len > SNAP_LEN {
                                        unsafe { NVIC::unmask(stm32::Interrupt::TIM7) };
                                        writeln!(tx, "m: window {} out of range\r", win_len).ok();
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
                                            let rel = row_start.wrapping_sub(win_start) as usize;
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
                                                        "{}{}{}{} ",
                                                        hex[((v >> 12) & 0xF) as usize] as char,
                                                        hex[((v >> 8) & 0xF) as usize] as char,
                                                        hex[((v >> 4) & 0xF) as usize] as char,
                                                        hex[(v & 0xF) as usize] as char,
                                                    )
                                                    .ok();
                                                }
                                            }
                                            writeln!(tx, "\r").ok();
                                        }
                                    }
                                }
                            }
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
            }

            // ADC sampling for raw ring ('l'/'m' dumps).
            if electrical_hz > 100 {
                const FLOAT_CHAN: [u32; 6] = [14, 5, 17, 14, 5, 17];
                let s = (sector % 6) as usize;
                let fixed = RING_CHAN_FIXED.load(Ordering::Relaxed);
                let chan = if fixed != 0 { fixed } else { FLOAT_CHAN[s] };
                let idx = match chan {
                    5 => 0usize,
                    14 => 1,
                    _ => 2,
                };
                let raw = core::ptr::addr_of!(ADC2_DMA_BUF)
                    .cast::<u16>()
                    .add(idx)
                    .read_volatile();
                let wr = RAW_SAMPLE_WR.load(Ordering::Relaxed);
                RAW_SAMPLE_BUF[wr as usize & RAW_SAMPLE_MASK] = raw;
                RAW_SAMPLE_WR.store(wr.wrapping_add(1), Ordering::Relaxed);
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
