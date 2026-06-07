//! Minimal open-loop motor driver for bench/scope work.
//!
//! Keeps only the control-path pieces from `motor_tester`:
//!   f / v   electrical frequency  +10 / -10 Hz
//!   a / z   amplitude              +1.0 / -1.0 %
//!   + / -   amplitude              +0.1 / -0.1 %
//!   w       kill
//!   q       reset to defaults and run

#![no_std]
#![no_main]

use core::fmt::Write;

use cortex_m::peripheral::NVIC;
use cortex_m_rt::entry;
use embedded_io::Read;
use portable_atomic::{AtomicBool, AtomicU32, Ordering};
use rtt_target::rprintln;

use rinz::hal;
use rinz::hal::adc::{
    AdcClaim, AdcCommonExt,
    config::{ClockMode, Continuous, Dma as AdcDma, SampleTime, Sequence},
};
use rinz::hal::dma::{TransferExt, channel::DMAExt, config::DmaConfig};
use rinz::hal::prelude::*;
use rinz::hal::pwm::PwmAdvExt;
use rinz::hal::pwr::{PwrExt, VoltageScale};
use rinz::hal::rcc::{PllConfig, PllMDiv, PllNMul, PllRDiv, PllSrc};
use rinz::hal::serial::FullConfig;
use rinz::hal::time::{ExtU32, Hertz, RateExtU32};
use rinz::hal::{rcc, stm32};

const DRIVE_HZ: u32 = 96_000;
const LOGICAL_SECTORS: u32 = 24;
const ELEC_STEPS_PER_REV: u32 = 48;
const STEPS_PER_LOGICAL_SECTOR: u32 = ELEC_STEPS_PER_REV / LOGICAL_SECTORS;

const AMP_CAP: u32 = 300; // 30.0 %
const AMP_START: u32 = 80; // 8.0 %
const FREQ_MIN: u32 = 1;
const FREQ_MAX: u32 = 600;
const FREQ_START: u32 = 60;

// Free-running ADC2 sampling of one BEMF phase.
// ADC clock = synchronous HCLK/4 = 42.5 MHz (proven on this board, always present
// — async-from-SYSCLK doesn't deliver a live kernel clock). Conversion at the
// 640.5-cycle sample time = 640.5 + 12.5 = 653 cyc → ~65 kSa/s (~15.4 µs each).
const ADC_SAMPLE_HZ: u32 = 42_500_000 / 653; // ≈ 65_084
// Buffer must hold this many revolutions at the lowest supported electrical freq.
const CAPTURE_REVS: u32 = 2;
const CAPTURE_MIN_HZ: u32 = 60;
// +1 to round up the partial sample; fits comfortably in 32 KB SRAM (~7.3 KB).
const ADC_BUF_LEN: usize = (ADC_SAMPLE_HZ * CAPTURE_REVS / CAPTURE_MIN_HZ + 1) as usize;

static RUNNING: AtomicBool = AtomicBool::new(false);
static AMPLITUDE: AtomicU32 = AtomicU32::new(AMP_START);
static ELECTRICAL_HZ: AtomicU32 = AtomicU32::new(FREQ_START);

/// Driven-high / driven-low phase index per logical sector (A=0, B=1, C=2).
/// Each physical six-step state is repeated 4 times.
const SIX_STEP_HIGH: [usize; LOGICAL_SECTORS as usize] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2,
];
const SIX_STEP_LOW: [usize; LOGICAL_SECTORS as usize] = [
    1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1,
];

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

/// 6-step commutation: two phases driven, one floating.
unsafe fn set_six_step(sector: u8, duty: u32) {
    let s = (sector as usize) % LOGICAL_SECTORS as usize;
    let hi = SIX_STEP_HIGH[s];
    let lo = SIX_STEP_LOW[s];
    let t1 = unsafe { &*stm32::TIM1::ptr() };

    t1.ccr1()
        .write(|w| unsafe { w.ccr().bits(if hi == 0 { duty } else { 0 }) });
    t1.ccr2()
        .write(|w| unsafe { w.ccr().bits(if hi == 1 { duty } else { 0 }) });
    t1.ccr3()
        .write(|w| unsafe { w.ccr().bits(if hi == 2 { duty } else { 0 }) });
    unsafe { set_phase_modes(hi != 0 && lo != 0, hi != 1 && lo != 1, hi != 2 && lo != 2) };
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

/// Handle one serial command byte. Generic over the USART tx and the three PWM
/// channels.
fn handle_command<TX, C1, C2, C3>(
    cmd: u8,
    tx: &mut TX,
    c1: &mut C1,
    c2: &mut C2,
    c3: &mut C3,
    half: u16,
) where
    TX: Write,
    C1: SetDutyCycle,
    C2: SetDutyCycle,
    C3: SetDutyCycle,
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
        b'w' => {
            RUNNING.store(false, Ordering::Relaxed);
            restore_all_af();
            let _ = c1.set_duty_cycle(half);
            let _ = c2.set_duty_cycle(half);
            let _ = c3.set_duty_cycle(half);
            writeln!(tx, "kill\r").ok();
        }
        b'q' => {
            restore_all_af();
            ELECTRICAL_HZ.store(FREQ_START, Ordering::Relaxed);
            AMPLITUDE.store(AMP_START, Ordering::Relaxed);
            RUNNING.store(true, Ordering::Relaxed);
            writeln!(
                tx,
                "reset: freq={}Hz amp={}.{}% mode=six-step\r",
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

    let cp = cortex_m::Peripherals::take().unwrap();
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
        "scope ready  f/v=±10Hz a/z=±1% +/-=±0.1% w=off q=reset\r"
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
        .frequency(20_000u32.Hz())
        .with_deadtime(100u32.nanos())
        .center_aligned()
        .finalize();

    let mut c1 = c1.into_complementary(gpioc.pc13.into_alternate::<4>());
    let mut c2 = c2.into_complementary(gpioa.pa12.into_alternate::<6>());
    let mut c3 = c3.into_complementary(gpiob.pb15.into_alternate::<4>());

    let half = c1.max_duty_cycle() as u32 / 2;
    c1.enable();
    c2.enable();
    c3.enable();
    let _ = c1.set_duty_cycle(half as u16);
    let _ = c2.set_duty_cycle(half as u16);
    let _ = c3.set_duty_cycle(half as u16);

    // ADC2 ch17 (PA4) — continuous, circular DMA (mirrors HAL adc-continious-dma example)
    writeln!(tx, "dbg: pwm ok, adc setup\r").ok();
    let pa4 = gpioa.pa4.into_analog();
    let dma_channels = dp.DMA1.split(&rcc);
    let dma_config = DmaConfig::default()
        .transfer_complete_interrupt(false)
        .circular_buffer(true)
        .memory_increment(true);

    let mut delay = cp.SYST.delay(&clocks);
    // Synchronous HCLK/4 = 42.5 MHz: derived directly from AHB so the kernel clock is
    // always live (calibration won't hang), and within the ADC's 60 MHz max. The
    // async-from-SYSCLK path does not deliver a running kernel clock on this board.
    let adc_clock = ClockMode::AdcHclkDiv4;
    writeln!(tx, "dbg: claim common\r").ok();
    let adc12_common = dp.ADC12_COMMON.claim(adc_clock, &mut rcc);
    writeln!(tx, "dbg: claim adc2 (vreg+calib)\r").ok();
    let mut adc = adc12_common.claim(dp.ADC2, &mut delay);

    writeln!(tx, "dbg: configure channel\r").ok();
    adc.set_continuous(Continuous::Continuous);
    adc.reset_sequence();
    adc.configure_channel(&pa4, Sequence::One, SampleTime::Cycles_640_5);

    writeln!(tx, "dbg: dma transfer ({} samples)\r", ADC_BUF_LEN).ok();
    let adc_buffer = cortex_m::singleton!(: [u16; ADC_BUF_LEN] = [0; ADC_BUF_LEN]).unwrap();
    let mut adc_transfer = dma_channels.ch1.into_circ_peripheral_to_memory_transfer(
        adc.enable_dma(AdcDma::Continuous),
        &mut adc_buffer[..],
        dma_config,
    );
    writeln!(tx, "dbg: adc start\r").ok();
    adc_transfer.start(|adc| adc.start_conversion());
    writeln!(tx, "dbg: adc ok\r").ok();

    rinz::tim7_drive::init(dp.TIM7, DRIVE_HZ, &clocks);
    unsafe { NVIC::unmask(stm32::Interrupt::TIM7) };

    RUNNING.store(true, Ordering::Relaxed);
    writeln!(
        tx,
        "reset: freq={}Hz amp={}.{}% mode=six-step\r",
        FREQ_START,
        AMP_START / 10,
        AMP_START % 10
    )
    .ok();

    loop {
        let mut buf = [0u8; 1];
        if rx.read(&mut buf).is_ok() {
            handle_command(buf[0], &mut tx, &mut c1, &mut c2, &mut c3, half as u16);
        }
    }
}

#[allow(non_snake_case)]
#[unsafe(no_mangle)]
extern "C" fn TIM7() {
    rinz::tim7_drive::clear_update_flag();

    static mut STEP: u32 = 0;
    static mut PHASE_FRAC: u32 = 0;

    if !RUNNING.load(Ordering::Relaxed) {
        return;
    }

    let electrical_hz = ELECTRICAL_HZ.load(Ordering::Relaxed);
    let amplitude = AMPLITUDE.load(Ordering::Relaxed);

    unsafe {
        PHASE_FRAC += electrical_hz * ELEC_STEPS_PER_REV;
        let advance = PHASE_FRAC / DRIVE_HZ;
        PHASE_FRAC %= DRIVE_HZ;
        STEP = (STEP + advance) % ELEC_STEPS_PER_REV;

        let arr = (*stm32::TIM1::ptr()).arr().read().arr().bits() as u32;
        let sector = (STEP / STEPS_PER_LOGICAL_SECTOR) as u8;
        let duty = arr * amplitude * 2 / (1000 * 3);
        set_six_step(sector, duty);
    }
}
