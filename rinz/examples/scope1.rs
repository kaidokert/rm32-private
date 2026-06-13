//! ON-time BEMF scope: samples ONE time per PWM period at the valley (CNT=0).
//!
//! In center-aligned PWM mode 1 the ON pulse is centered at the valley, so a
//! TRGO at CNT=0 lands in the exact middle of the ON window for every duty:
//!   HIGH  phase: high-side ON  → divider reads ~Vbus
//!   LOW   phase: low-side ON   → divider reads ~0
//!   FLOAT phase: both FETs OFF → divider reads neutral + BEMF
//! Mid-ON sampling stays valid from ~5% duty all the way to 95%+: the budget
//! after the trigger is half the ON window (1.25 us at 5% duty) and the 3-ch
//! scan's last sample aperture ends 1.05 us after TRGO at Cycles_6_5.
//!
//! TIM1 CC4 (CCR4=1, PWM mode 1) pulses OC4REF at the valley; CR2.MMS routes
//! OC4REF to TRGO straight into ADC2 EXTSEL — no TIM3 needed. 12-bit samples,
//! 20 kHz frame rate (1 frame = ch17/PA4, ch5/PC4, ch14/PB11).
//!
//! Commands:
//!   f / v   electrical frequency  +10 / -10 Hz
//!   a / z   amplitude              +1.0 / -1.0 %
//!   + / -   amplitude              +0.1 / -0.1 %
//!   w       kill
//!   d       capture 2 electrical revs from phase zero, then dump 12-bit hex
//!   q       reset to defaults and run

#![no_std]
#![no_main]

use core::fmt::Write;

use cortex_m::peripheral::NVIC;
use cortex_m_rt::entry;
use embedded_io::{Read, ReadReady};
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
const LOGICAL_SECTORS: u32 = 72;
const ELEC_STEPS_PER_REV: u32 = 144;
const STEPS_PER_LOGICAL_SECTOR: u32 = ELEC_STEPS_PER_REV / LOGICAL_SECTORS;

// amp is in 0.1% units of the drive scale; six-step duty = amp * 2/3.
// 1425 (142.5%) maps to 95% actual PWM duty — the design ceiling.
const AMP_CAP: u32 = 1425;
const AMP_START: u32 = 80; // 8.0 % (5.3 % actual duty)
const FREQ_MIN: u32 = 1;
const FREQ_MAX: u32 = 600;
const FREQ_START: u32 = 60;

// TIM1_TRGO-triggered ADC2 scan of the three BEMF phases:
// ch17/PA4, ch5/PC4, ch14/PB11 — one 3-channel scan per PWM period at the valley.
// ADC clock = synchronous HCLK/4 = 42.5 MHz (proven on this board, always present
// — async-from-SYSCLK doesn't deliver a live kernel clock). Each channel conversion
// at the 6.5-cycle sample time is 6.5 + 12.5 = 19 cyc = 447 ns; the 3rd channel's
// sampling aperture ends 2*447 + 153 = 1047 ns after TRGO, inside the half-ON
// window (1250 ns) down to 5% duty. 12-bit resolution: bench BEMF swings are a
// few tens of mV at the pin, far below 8-bit LSB.
const ADC_CHANNELS: usize = 3;
const ADC_SAMPLES_PER_PWM: u32 = 1;
const ADC_FRAME_HZ: u32 = PWM_HZ * ADC_SAMPLES_PER_PWM;
// Buffer must hold this many revolutions at the lowest supported electrical freq.
const CAPTURE_REVS: u32 = 2;
const CAPTURE_MIN_HZ: u32 = 60;
// +1 to round up the partial frame; total buffer fits comfortably in 32 KB SRAM.
const ADC_FRAME_COUNT: usize = (ADC_FRAME_HZ * CAPTURE_REVS / CAPTURE_MIN_HZ + 1) as usize;
const ADC_BUF_LEN: usize = ADC_FRAME_COUNT * ADC_CHANNELS;

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
static CAPTURE_REQUEST: AtomicBool = AtomicBool::new(false);
static CAPTURE_DONE: AtomicBool = AtomicBool::new(false);
static CAPTURE_FRAMES: AtomicU32 = AtomicU32::new(0);
static CAPTURE_TICKS_TARGET: AtomicU32 = AtomicU32::new(0);
static CAPTURE_BUF_ADDR: AtomicU32 = AtomicU32::new(0);
// The other buffer: where the ISR re-points the DMA the instant the capture window
// closes, so ADC->DMA keeps streaming while main drains the frozen buffer over UART.
static CAPTURE_BUF_ALT: AtomicU32 = AtomicU32::new(0);
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
        "scope1 ready 1x ADC valley 12-bit  f/v=±10Hz a/z=±1% +/-=±0.1% w=off d=dump q=reset\r"
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
    writeln!(tx, "dbg: claim adc2 (vreg+calib)\r").ok();
    let mut adc = adc12_common.claim(dp.ADC2, &mut delay);

    writeln!(tx, "dbg: configure channel\r").ok();
    adc.set_resolution(Resolution::Twelve);
    adc.set_continuous(Continuous::Single);
    adc.set_external_trigger((TriggerMode::RisingEdge, ExternalTrigger12::Tim_1_trgo));
    adc.reset_sequence();
    adc.configure_channel(&pa4, Sequence::One, SampleTime::Cycles_6_5);
    adc.configure_channel(&pc4, Sequence::Two, SampleTime::Cycles_6_5);
    adc.configure_channel(&pb11, Sequence::Three, SampleTime::Cycles_6_5);

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

    rinz::tim7_drive::init(dp.TIM7, DRIVE_HZ, &clocks);
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

    loop {
        let mut buf = [0u8; 1];
        if rx.read(&mut buf).is_ok() {
            if buf[0] == b'd' {
                // Flip to the other buffer; the previous capture stays intact in the
                // one we just left. The ISR aims the DMA at CAPTURE_BUF_ADDR for this
                // capture, then flips it onto CAPTURE_BUF_ALT when the window closes.
                let sel = !BUF_SEL.load(Ordering::Relaxed);
                BUF_SEL.store(sel, Ordering::Relaxed);
                let cap_ptr = if sel { buf_ptr1 } else { buf_ptr0 };
                let alt_ptr = if sel { buf_ptr0 } else { buf_ptr1 };
                CAPTURE_BUF_ADDR.store(cap_ptr as u32, Ordering::Relaxed);
                CAPTURE_BUF_ALT.store(alt_ptr as u32, Ordering::Relaxed);

                CAPTURE_DONE.store(false, Ordering::Relaxed);
                CAPTURE_FRAMES.store(0, Ordering::Relaxed);
                CAPTURE_TICKS_TARGET.store(0, Ordering::Relaxed);
                CAPTURE_REQUEST.store(true, Ordering::Relaxed);
                RUNNING.store(true, Ordering::Relaxed);
                writeln!(tx, "capture: wait zero, 2 electrical revs\r").ok();

                while !CAPTURE_DONE.load(Ordering::Relaxed) {
                    cortex_m::asm::nop();
                }

                let frames = CAPTURE_FRAMES.load(Ordering::Relaxed) as usize;
                writeln!(
                    tx,
                    "debug: hz={} amp={} tim7={} six_ticks={} dma_tc={} dma_ht={} dma_te={}\r",
                    DBG_CAPTURE_HZ.load(Ordering::Relaxed),
                    DBG_CAPTURE_AMP.load(Ordering::Relaxed),
                    DBG_TIM7_TICKS.load(Ordering::Relaxed),
                    DBG_SIX_STEP_TICKS.load(Ordering::Relaxed),
                    DBG_DMA_TC.load(Ordering::Relaxed),
                    DBG_DMA_HT.load(Ordering::Relaxed),
                    DBG_DMA_TE.load(Ordering::Relaxed)
                )
                .ok();
                dump_debug_registers(&mut tx);
                // No motor kill, no DMA pause: the DMA is already streaming into the
                // alt buffer (flipped by the ISR), so dump the frozen buffer in place
                // while capture and commutation keep running.
                dump_buffer(&mut tx, cap_ptr, frames);
                // Reject re-triggers: drain any keys (notably another 'd') that landed
                // in the RDR during the long dump. read_ready() keeps this non-blocking
                // (rx.read() itself blocks until a byte arrives).
                let mut drain = [0u8; 1];
                while rx.read_ready().unwrap_or(false) && rx.read(&mut drain).is_ok() {}
            } else {
                handle_command(buf[0], &mut tx, &mut c1, &mut c2, &mut c3, half as u16);
            }
        }
    }
}

/// Dump the (paused) ADC capture buffer as 12-bit hex (4-digit). One line is
/// one ch17/ch5/ch14 frame.
fn dump_buffer<TX: Write>(tx: &mut TX, ptr: *const u16, frames: usize) {
    writeln!(
        tx,
        "dump3: {} frames x 3 channels (ch17 ch5 ch14, 12-bit ADC, {} Hz)\r",
        frames, ADC_FRAME_HZ
    )
    .ok();
    for frame in 0..frames {
        for ch in 0..ADC_CHANNELS {
            let i = frame * ADC_CHANNELS + ch;
            let v = unsafe { core::ptr::read_volatile(ptr.add(i)) };
            write!(tx, "{:04x}", v).ok();
            if ch + 1 < ADC_CHANNELS {
                write!(tx, " ").ok();
            }
        }
        writeln!(tx, "\r").ok();
    }
    writeln!(tx, "\rend\r").ok();
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
        if CAPTURE_ACTIVE {
            DBG_SIX_STEP_TICKS.fetch_add(1, Ordering::Relaxed);
        }
        let sector = (STEP / STEPS_PER_LOGICAL_SECTOR) as u8;
        let duty = arr * amplitude * 2 / (1000 * 3);
        set_six_step(sector, duty);

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
            CAPTURE_FRAMES.store(frames, Ordering::Relaxed);
            CAPTURE_TICKS_TARGET.store(ticks, Ordering::Relaxed);
            CAPTURE_TICKS = 0;
            restart_capture_dma(CAPTURE_BUF_ADDR.load(Ordering::Relaxed));
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
                CAPTURE_ACTIVE = false;
                CAPTURE_DONE.store(true, Ordering::Relaxed);
            }
        }
    }
}
