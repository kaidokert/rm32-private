//! Minimal open-loop motor driver for bench/scope work.
//!
//! Keeps only the control-path pieces from `motor_tester`:
//!   f / v   electrical frequency  +10 / -10 Hz
//!   a / z   amplitude              +1.0 / -1.0 %
//!   + / -   amplitude              +0.1 / -0.1 %
//!   m       toggle commutation mode (six-step / sine)
//!   0 / 1 / 2 / 3  notch off / A / B / C
//!   w       kill
//!   d       capture 2 electrical revs from phase zero, then dump 8-bit hex (sample >> 4)
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
    Adc, AdcClaim, AdcCommonExt, DMA as AdcDmaStatus, Instance,
    config::{
        ClockMode, Continuous, Dma as AdcDma, ExternalTrigger12, Resolution, SampleTime, Sequence,
        TriggerMode,
    },
};
use rinz::hal::dma::{
    PeripheralToMemory, TransferExt, channel::DMAExt, config::DmaConfig, traits::TargetAddress,
};
use rinz::hal::prelude::*;
use rinz::hal::pwm::PwmAdvExt;
use rinz::hal::pwr::{PwrExt, VoltageScale};
use rinz::hal::rcc::{PllConfig, PllMDiv, PllNMul, PllRDiv, PllSrc};
use rinz::hal::serial::FullConfig;
use rinz::hal::time::{ExtU32, Hertz, RateExtU32};
use rinz::hal::{rcc, stm32};

const PWM_HZ: u32 = 20_000;
const DRIVE_HZ: u32 = PWM_HZ;
const LOGICAL_SECTORS: u32 = 24;
const ELEC_STEPS_PER_REV: u32 = 48;
const STEPS_PER_LOGICAL_SECTOR: u32 = ELEC_STEPS_PER_REV / LOGICAL_SECTORS;
const MODE_SIX_STEP: u32 = 0;
const MODE_SINE: u32 = 1;
const SINE_SCALE: i32 = 1000;
const NOTCH_OFF: u32 = 0;
const NOTCH_A: u32 = 1;
const NOTCH_B: u32 = 2;
const NOTCH_C: u32 = 3;
// Positive-side diagnostic notch width: 5 table slots * 7.5° = 37.5°.
const SINE_NOTCH_HALF_WIDTH_STEPS: usize = 2;
const SINE_TABLE: [i16; ELEC_STEPS_PER_REV as usize] = [
    0, 131, 259, 383, 500, 609, 707, 793, 866, 924, 966, 991, 1000, 991, 966, 924, 866, 793, 707,
    609, 500, 383, 259, 131, 0, -131, -259, -383, -500, -609, -707, -793, -866, -924, -966, -991,
    -1000, -991, -966, -924, -866, -793, -707, -609, -500, -383, -259, -131,
];

const AMP_CAP: u32 = 300; // 30.0 %
const AMP_START: u32 = 80; // 8.0 %
const FREQ_MIN: u32 = 1;
const FREQ_MAX: u32 = 600;
const FREQ_START: u32 = 60;

// TIM1_TRGO-triggered ADC2 scan of the three BEMF phases:
// ch17/PA4, ch5/PC4, ch14/PB11. One 3-channel scan is captured per PWM period.
// ADC clock = synchronous HCLK/4 = 42.5 MHz (proven on this board, always present
// — async-from-SYSCLK doesn't deliver a live kernel clock). Each channel conversion
// at the 6.5-cycle sample time is 6.5 + 12.5 = 19 cyc; a full 3-channel scan is
// ~1.34 us, safely inside the 20 kHz PWM period. ADC resolution is 8-bit for
// direct dump/plot comparison with the previous 12-bit >> 4 display path.
const ADC_CHANNELS: usize = 3;
const ADC_FRAME_HZ: u32 = PWM_HZ;
// Buffer must hold this many revolutions at the lowest supported electrical freq.
const CAPTURE_REVS: u32 = 2;
const CAPTURE_MIN_HZ: u32 = 60;
// +1 to round up the partial frame; total buffer fits comfortably in 32 KB SRAM.
const ADC_FRAME_COUNT: usize = (ADC_FRAME_HZ * CAPTURE_REVS / CAPTURE_MIN_HZ + 1) as usize;
const ADC_BUF_LEN: usize = ADC_FRAME_COUNT * ADC_CHANNELS;

struct AdcDma8<ADC: Instance>(Adc<ADC, AdcDmaStatus>);

impl<ADC: Instance> AdcDma8<ADC> {
    fn start_conversion(&mut self) {
        self.0.start_conversion();
    }

    fn cancel_conversion(&mut self) {
        self.0.cancel_conversion();
    }

    fn clear_overrun_flag(&mut self) {
        self.0.clear_overrun_flag();
    }
}

unsafe impl<ADC: Instance> TargetAddress<PeripheralToMemory> for AdcDma8<ADC> {
    #[inline(always)]
    fn address(&self) -> u32 {
        <Adc<ADC, AdcDmaStatus> as TargetAddress<PeripheralToMemory>>::address(&self.0)
    }

    type MemSize = u8;

    const REQUEST_LINE: Option<u8> =
        <Adc<ADC, AdcDmaStatus> as TargetAddress<PeripheralToMemory>>::REQUEST_LINE;
}

static RUNNING: AtomicBool = AtomicBool::new(false);
static AMPLITUDE: AtomicU32 = AtomicU32::new(AMP_START);
static ELECTRICAL_HZ: AtomicU32 = AtomicU32::new(FREQ_START);
static COMMUTATION_MODE: AtomicU32 = AtomicU32::new(MODE_SIX_STEP);
static NOTCH_PHASE: AtomicU32 = AtomicU32::new(NOTCH_OFF);
static CAPTURE_REQUEST: AtomicBool = AtomicBool::new(false);
static CAPTURE_DONE: AtomicBool = AtomicBool::new(false);
static CAPTURE_FRAMES: AtomicU32 = AtomicU32::new(0);
static CAPTURE_BUF_ADDR: AtomicU32 = AtomicU32::new(0);

/// Driven-high / driven-low phase index per logical sector (A=0, B=1, C=2).
/// Each physical six-step state is repeated 4 times.
const SIX_STEP_HIGH: [usize; LOGICAL_SECTORS as usize] = [
    /*0*/ 0, 0, 0, 0, /*1*/ 0, 0, 0, 0, /*2*/ 1, 1, 1, 1, /*3*/ 1, 1, 1, 1,
    /*4*/ 2, 2, 2, 2, /*5*/ 2, 2, 2, 2,
];
const SIX_STEP_LOW: [usize; LOGICAL_SECTORS as usize] = [
    /*0*/ 1, 1, 1, 1, /*1*/ 2, 2, 2, 2, /*2*/ 2, 2, 2, 2, /*3*/ 0, 0, 2, 0,
    /*4*/ 0, 0, 0, 0, /*5*/ 1, 1, 1, 1,
];
/// Per-logical-sector marker mode:
///   0 = none
///   1 = hit the high-driven side by forcing the nominal high phase LOW
///   2 = hit the low-driven side by floating the nominal low phase
const MARKER_MODE: [u8; LOGICAL_SECTORS as usize] = [
    /*0*/ 0, 0, 0, 0, /*1*/ 0, 0, 0, 0, /*2*/ 0, 0, 0, 0, /*3*/ 0, 0, 0, 0,
    /*4*/ 0, 0, 0, 0, /*5*/ 0, 0, 0, 0,
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

/// TIM1_TRGO = OC4REF. CCR4=ARR/2 puts the sample point in the middle of the
/// center-aligned PWM period. CCR4=1 samples at CNT=0, where sine mode has all
/// three phases "on" because every duty is nonzero, producing all-rail ADC reads.
fn configure_tim1_adc_trgo() {
    let t1 = unsafe { &*stm32::TIM1::ptr() };
    let mid = t1.arr().read().arr().bits() as u32 / 2;
    t1.ccr4().write(|w| unsafe { w.ccr().bits(mid) });
    t1.ccmr2_output()
        .modify(|r, w| unsafe { w.bits((r.bits() & !(0x7000 | 0x300)) | (0b110u32 << 12)) });
    t1.cr2()
        .modify(|r, w| unsafe { w.bits((r.bits() & !0x70) | (0b111u32 << 4)) });
}

fn two_rev_frame_count(electrical_hz: u32) -> u32 {
    let hz = electrical_hz.max(1);
    let frames = (PWM_HZ * CAPTURE_REVS).div_ceil(hz);
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

unsafe fn pause_capture_dma() {
    let dma = unsafe { &*stm32::DMA1::ptr() };
    let ch = dma.ch1();
    let cr = ch.cr().read().bits();
    ch.cr().write(|w| unsafe { w.bits(cr & !1) });
}

/// Logical-sector commutation: two phases driven, one floating.
unsafe fn set_six_step(sector: u8, duty: u32) {
    let s = (sector as usize) % LOGICAL_SECTORS as usize;
    let hi = SIX_STEP_HIGH[s];
    let lo = SIX_STEP_LOW[s];
    let marker_mode = MARKER_MODE[s];
    let marker_hit_hi = marker_mode == 1;
    let marker_hit_lo = marker_mode == 2;
    let t1 = unsafe { &*stm32::TIM1::ptr() };

    t1.ccr1().write(|w| unsafe {
        w.ccr()
            .bits(if hi == 0 && !marker_hit_hi { duty } else { 0 })
    });
    t1.ccr2().write(|w| unsafe {
        w.ccr()
            .bits(if hi == 1 && !marker_hit_hi { duty } else { 0 })
    });
    t1.ccr3().write(|w| unsafe {
        w.ccr()
            .bits(if hi == 2 && !marker_hit_hi { duty } else { 0 })
    });

    // Normally one phase is high, one is low, one floats.
    // Marker mode 1: take the nominal high phase out of AF and drive it low.
    // Marker mode 2: float the nominal low phase instead of actively pulling it low.
    let float_a = (hi != 0 && lo != 0) || (marker_hit_hi && hi == 0) || (marker_hit_lo && lo == 0);
    let float_b = (hi != 1 && lo != 1) || (marker_hit_hi && hi == 1) || (marker_hit_lo && lo == 1);
    let float_c = (hi != 2 && lo != 2) || (marker_hit_hi && hi == 2) || (marker_hit_lo && lo == 2);
    unsafe { set_phase_modes(float_a, float_b, float_c) };
}

unsafe fn set_sine(step: u32, arr: u32, amplitude: u32) {
    let phase = (step as usize) % SINE_TABLE.len();
    let amp = arr as i32 * amplitude as i32 / (1000 * 2);
    let center = arr as i32 / 2;

    let notch = NOTCH_PHASE.load(Ordering::Relaxed);
    let fault_center = match notch {
        NOTCH_A => Some(12),
        NOTCH_B => Some(44),
        NOTCH_C => Some(28),
        _ => None,
    };
    let fault_active = fault_center
        .map(|center_step: usize| phase.abs_diff(center_step) <= SINE_NOTCH_HALF_WIDTH_STEPS)
        .unwrap_or(false);

    let duty_a = if notch == NOTCH_A && fault_active {
        center - amp
    } else {
        center + amp * SINE_TABLE[phase] as i32 / SINE_SCALE
    };
    let duty_b = if notch == NOTCH_B && fault_active {
        center - amp
    } else {
        center + amp * SINE_TABLE[(phase + 16) % SINE_TABLE.len()] as i32 / SINE_SCALE
    };
    let duty_c = if notch == NOTCH_C && fault_active {
        center - amp
    } else {
        center + amp * SINE_TABLE[(phase + 32) % SINE_TABLE.len()] as i32 / SINE_SCALE
    };
    let clamp = |d: i32| d.clamp(0, arr as i32) as u32;

    let t1 = unsafe { &*stm32::TIM1::ptr() };
    t1.ccr1().write(|w| unsafe { w.ccr().bits(clamp(duty_a)) });
    t1.ccr2().write(|w| unsafe { w.ccr().bits(clamp(duty_b)) });
    t1.ccr3().write(|w| unsafe { w.ccr().bits(clamp(duty_c)) });
    restore_all_af();
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
        b'm' => {
            let next = if COMMUTATION_MODE.load(Ordering::Relaxed) == MODE_SIX_STEP {
                MODE_SINE
            } else {
                MODE_SIX_STEP
            };
            COMMUTATION_MODE.store(next, Ordering::Relaxed);
            let name = if next == MODE_SINE {
                "sine"
            } else {
                "six-step"
            };
            writeln!(tx, "mode={}\r", name).ok();
        }
        b'0' => {
            NOTCH_PHASE.store(NOTCH_OFF, Ordering::Relaxed);
            writeln!(tx, "notch=off\r").ok();
        }
        b'1' => {
            NOTCH_PHASE.store(NOTCH_A, Ordering::Relaxed);
            writeln!(tx, "notch=A\r").ok();
        }
        b'2' => {
            NOTCH_PHASE.store(NOTCH_B, Ordering::Relaxed);
            writeln!(tx, "notch=B\r").ok();
        }
        b'3' => {
            NOTCH_PHASE.store(NOTCH_C, Ordering::Relaxed);
            writeln!(tx, "notch=C\r").ok();
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
            COMMUTATION_MODE.store(MODE_SIX_STEP, Ordering::Relaxed);
            NOTCH_PHASE.store(NOTCH_OFF, Ordering::Relaxed);
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
        "scope ready  f/v=±10Hz a/z=±1% +/-=±0.1% m=mode 0/1/2/3=notch w=off d=dump q=reset\r"
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
    c1.enable();
    c2.enable();
    c3.enable();
    let _ = c1.set_duty_cycle(half as u16);
    let _ = c2.set_duty_cycle(half as u16);
    let _ = c3.set_duty_cycle(half as u16);
    configure_tim1_adc_trgo();

    // ADC2 ch17/ch5/ch14 (PA4/PC4/PB11) — TIM1_TRGO-triggered circular DMA scan.
    writeln!(tx, "dbg: pwm ok, adc setup\r").ok();
    let pa4 = gpioa.pa4.into_analog();
    let pc4 = gpioc.pc4.into_analog();
    let pb11 = gpiob.pb11.into_analog();
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
    adc.set_resolution(Resolution::Eight);
    adc.set_continuous(Continuous::Single);
    adc.set_external_trigger((TriggerMode::RisingEdge, ExternalTrigger12::Tim_1_trgo));
    adc.reset_sequence();
    adc.configure_channel(&pa4, Sequence::One, SampleTime::Cycles_6_5);
    adc.configure_channel(&pc4, Sequence::Two, SampleTime::Cycles_6_5);
    adc.configure_channel(&pb11, Sequence::Three, SampleTime::Cycles_6_5);

    writeln!(tx, "dbg: dma transfer ({} samples)\r", ADC_BUF_LEN).ok();
    let adc_buffer = cortex_m::singleton!(: [u8; ADC_BUF_LEN] = [0; ADC_BUF_LEN]).unwrap();
    // Raw pointer to the capture buffer for the 'd' dump. The buffer itself is moved
    // into the DMA transfer below; we only read it (volatile) after pausing the DMA.
    let buf_ptr: *const u8 = adc_buffer.as_ptr();
    CAPTURE_BUF_ADDR.store(buf_ptr as u32, Ordering::Relaxed);
    let mut adc_transfer = dma_channels.ch1.into_circ_peripheral_to_memory_transfer(
        AdcDma8(adc.enable_dma(AdcDma::Continuous)),
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
            if buf[0] == b'd' {
                CAPTURE_DONE.store(false, Ordering::Relaxed);
                CAPTURE_FRAMES.store(0, Ordering::Relaxed);
                CAPTURE_REQUEST.store(true, Ordering::Relaxed);
                RUNNING.store(true, Ordering::Relaxed);
                writeln!(tx, "capture: wait zero, 2 electrical revs\r").ok();

                while !CAPTURE_DONE.load(Ordering::Relaxed) {
                    cortex_m::asm::nop();
                }

                let frames = CAPTURE_FRAMES.load(Ordering::Relaxed) as usize;
                handle_command(b'w', &mut tx, &mut c1, &mut c2, &mut c3, half as u16);
                adc_transfer.pause(|adc| adc.cancel_conversion());
                dump_buffer(&mut tx, buf_ptr, frames);
                adc_transfer.start(|adc| {
                    adc.clear_overrun_flag();
                    adc.start_conversion();
                });
            } else {
                handle_command(buf[0], &mut tx, &mut c1, &mut c2, &mut c3, half as u16);
            }
        }
    }
}

/// Dump the (paused) ADC capture buffer as 8-bit hex. The ADC and DMA are both
/// configured for 8-bit samples. One line is one ch17/ch5/ch14 frame.
fn dump_buffer<TX: Write>(tx: &mut TX, ptr: *const u8, frames: usize) {
    writeln!(
        tx,
        "dump3: {} frames x 3 channels (ch17 ch5 ch14, 8-bit ADC)\r",
        frames
    )
    .ok();
    for frame in 0..frames {
        for ch in 0..ADC_CHANNELS {
            let i = frame * ADC_CHANNELS + ch;
            let v = unsafe { core::ptr::read_volatile(ptr.add(i)) };
            write!(tx, "{:02x}", v).ok();
            if ch + 1 < ADC_CHANNELS {
                write!(tx, " ").ok();
            }
        }
        writeln!(tx, "\r").ok();
    }
    writeln!(tx, "\rend\r").ok();
}

#[allow(non_snake_case)]
#[unsafe(no_mangle)]
extern "C" fn TIM7() {
    rinz::tim7_drive::clear_update_flag();

    static mut STEP: u32 = 0;
    static mut PHASE_FRAC: u32 = 0;
    static mut CAPTURE_WAIT_ZERO: bool = false;
    static mut CAPTURE_ACTIVE: bool = false;
    static mut CAPTURE_TICKS: u32 = 0;

    if !RUNNING.load(Ordering::Relaxed) {
        return;
    }

    let electrical_hz = ELECTRICAL_HZ.load(Ordering::Relaxed);
    let amplitude = AMPLITUDE.load(Ordering::Relaxed);

    unsafe {
        if CAPTURE_REQUEST.swap(false, Ordering::Relaxed) {
            CAPTURE_WAIT_ZERO = true;
            CAPTURE_ACTIVE = false;
            CAPTURE_TICKS = 0;
        }

        let prev_step = STEP;
        PHASE_FRAC += electrical_hz * ELEC_STEPS_PER_REV;
        let advance = PHASE_FRAC / DRIVE_HZ;
        PHASE_FRAC %= DRIVE_HZ;
        STEP = (STEP + advance) % ELEC_STEPS_PER_REV;
        let wrapped_to_zero = advance != 0 && STEP < prev_step;

        let arr = (*stm32::TIM1::ptr()).arr().read().arr().bits() as u32;
        if COMMUTATION_MODE.load(Ordering::Relaxed) == MODE_SINE {
            set_sine(STEP, arr, amplitude);
        } else {
            let sector = (STEP / STEPS_PER_LOGICAL_SECTOR) as u8;
            let duty = arr * amplitude * 2 / (1000 * 3);
            set_six_step(sector, duty);
        }

        let mut capture_started = false;
        if CAPTURE_WAIT_ZERO && wrapped_to_zero {
            let frames = two_rev_frame_count(electrical_hz);
            CAPTURE_FRAMES.store(frames, Ordering::Relaxed);
            CAPTURE_TICKS = 0;
            restart_capture_dma(CAPTURE_BUF_ADDR.load(Ordering::Relaxed));
            CAPTURE_WAIT_ZERO = false;
            CAPTURE_ACTIVE = true;
            capture_started = true;
        }

        if CAPTURE_ACTIVE && !capture_started {
            CAPTURE_TICKS += 1;
            if CAPTURE_TICKS >= CAPTURE_FRAMES.load(Ordering::Relaxed) {
                pause_capture_dma();
                RUNNING.store(false, Ordering::Relaxed);
                CAPTURE_ACTIVE = false;
                CAPTURE_DONE.store(true, Ordering::Relaxed);
            }
        }
    }
}
