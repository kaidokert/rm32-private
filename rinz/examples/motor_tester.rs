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
//!   a / z   amplitude              +1 /  -1  (0..20)
//!   w       kill — duties to midpoint, no current
//!   q       reset — re-enable at defaults (60 Hz, amp 15)
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

/// TIM7 ISR rate. At 6 kHz the sine table tracks up to 600 Hz electrical
/// with fractional accumulation (600×48 = 28800 steps/s ÷ 6000 = 4.8/tick).
const DRIVE_HZ: u32 = 6_000;

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

const AMP_MAX: u32 = 20;
const AMP_START: u32 = 15;
const FREQ_MIN: u32 = 1;
const FREQ_MAX: u32 = 600;
const FREQ_START: u32 = 60;

// Shared state written by main, read by TIM7 ISR.
static RUNNING: AtomicBool = AtomicBool::new(false);
static AMPLITUDE: AtomicU32 = AtomicU32::new(AMP_START);
static ELECTRICAL_HZ: AtomicU32 = AtomicU32::new(FREQ_START);

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
    // VBUS:    PA0 → ADC1 IN1  (M1_BUS_VOLTAGE; divider ratio 0.09626)
    // Phase U: PA1 → OPAMP1 non-inv, PGA gain 16, internal output → ADC1
    //          shunt 3 mΩ → I_mA = V_opamp_mV / 48
    // Temp:    PB14 → ADC1 IN5  (NTC: V@25°C=1400mV, dV/dT=+19mV/°C)
    let pa0_vbus = gpioa.pa0.into_analog();
    let pa1_isns = gpioa.pa1.into_analog();
    let _pb14_ntc = gpiob.pb14.into_analog(); // sets MODER=analog; channel read via PAC
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
    rinz::tim7_drive::init(dp.TIM7, DRIVE_HZ, SYSCLK_HZ);
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
                            let amp = (AMPLITUDE.load(Ordering::Relaxed) + 1).min(AMP_MAX);
                            AMPLITUDE.store(amp, Ordering::Relaxed);
                            writeln!(tx, "amp={}\r", amp).ok();
                        }
                        b'z' => {
                            let amp = AMPLITUDE.load(Ordering::Relaxed).saturating_sub(1);
                            AMPLITUDE.store(amp, Ordering::Relaxed);
                            writeln!(tx, "amp={}\r", amp).ok();
                        }
                        b'w' => {
                            RUNNING.store(false, Ordering::Relaxed);
                            let _ = c1.set_duty_cycle(half as u16);
                            let _ = c2.set_duty_cycle(half as u16);
                            let _ = c3.set_duty_cycle(half as u16);
                            writeln!(tx, "kill\r").ok();
                            rprintln!("KILL");
                        }
                        b'q' => {
                            ELECTRICAL_HZ.store(FREQ_START, Ordering::Relaxed);
                            AMPLITUDE.store(AMP_START, Ordering::Relaxed);
                            RUNNING.store(true, Ordering::Relaxed);
                            writeln!(tx, "reset: freq={}Hz amp={}\r", FREQ_START, AMP_START).ok();
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
                            // NTC: V@25°C=1400mV, dV/dT=+19mV/°C → T = 25 + (V_mV - 1400) / 19
                            let temp_c = 25 + (temp_mv - 1400) / 19;

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
// TIM7 ISR — sinewave commutation heartbeat at DRIVE_HZ
// ---------------------------------------------------------------------------

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

    // Fractional phase accumulator: advances electrical_hz×48 sub-steps per
    // tick, draining DRIVE_HZ sub-steps per integer step.
    unsafe {
        PHASE_FRAC += electrical_hz * 48;
        let advance = PHASE_FRAC / DRIVE_HZ;
        PHASE_FRAC %= DRIVE_HZ;
        STEP = (STEP + advance) % 48;

        let va = SINE48[STEP as usize] as u32;
        let vb = SINE48[(STEP + 16) as usize % 48] as u32;
        let vc = SINE48[(STEP + 32) as usize % 48] as u32;

        // Read ARR from TIM1 to get half-duty (avoids a shared static).
        let arr = (*stm32::TIM1::ptr()).arr().read().arr().bits() as u32;
        let half = arr / 2;

        let duty_a = half + va * amplitude * half / (255 * 128);
        let duty_b = half + vb * amplitude * half / (255 * 128);
        let duty_c = half + vc * amplitude * half / (255 * 128);

        let t1 = &*stm32::TIM1::ptr();
        t1.ccr1().write(|w| w.ccr().bits(duty_a));
        t1.ccr2().write(|w| w.ccr().bits(duty_b));
        t1.ccr3().write(|w| w.ccr().bits(duty_c));
    }
}

// ---------------------------------------------------------------------------

#[exception]
fn SysTick() {
    TIMER.systick_handler();
}
