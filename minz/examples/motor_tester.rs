//! Bench motor tester: open-loop TIM1 PWM driven by a software amplitude
//! that the host can adjust over UART.
//!
//! - **RX**: soft-UART on **PA0** via LPTIM1 + EXTI0 (mirrors
//!   `bitbang_uart_lptim1.rs`). Single-key commands from the host.
//! - **TX**: hardware USART1 on **PB6** for status messages back to host.
//!   PB7 is also claimed as USART1_RX even though we don't use it,
//!   because the HAL's `Serial::usart1` constructor wants both pins.
//! - **Motor PWM**: TIM1 driving HIN1-3 / LIN1-3 via the AM32 pin map
//!   (same as `output_pins.rs`).
//!
//! Wire to host:
//! - USB-TTL TX  → PA0 (host → ESC, key commands)
//! - PB6         → USB-TTL RX (ESC → host, status messages)
//! - GND ↔ GND
//!
//! Primary control is **electrical frequency** — that's how an open-loop
//! drive moves the rotor. Amplitude is the secondary knob and is capped
//! to a deliberately conservative ceiling (see `AMP_MAX`) because outrunners
//! melt fast when the voltage you apply isn't being cancelled by back-EMF.
//!
//! Keys (paired like vertical neighbours on QWERTY — top key adds, bottom
//! subtracts):
//! - `d`       : electrical frequency +1 Hz
//! - `c`       : electrical frequency -1 Hz
//! - `f`       : electrical frequency +10 Hz
//! - `v`       : electrical frequency -10 Hz
//! - `a` / `A` : amplitude +1 (capped, see `AMP_MAX`)
//! - `z`       : amplitude -1
//! - `S`       : amplitude +10
//! - `x`       : amplitude -10
//! - `m`       : toggle waveform (sine ↔ 6-step BLDC commutation)
//! - `r`       : reset to sine, `f = FREQ_START`, `amp = AMP_START` (re-arms if killed)
//! - `w`       : hard kill — clear `MOE` in BDTR (all FETs off)
//! - `i`       : print ADC reads (PA3 = IN8 current, PA6 = IN11 battery)

#![no_std]
#![no_main]

use core::cell::RefCell;
use core::fmt::Write;

use cortex_m::interrupt::{Mutex, free};
use cortex_m::peripheral::NVIC;
use cortex_m_rt::{entry, exception};
use fugit::HertzU32 as Hertz;
use heapless::Deque;
use heapless::spsc::Queue;
use minz::board_init::{BoardInit, configure_motor_pwm_pins, configure_systick, init};
use minz::current_adc::SenseAdc;
use minz::hal::gpio::gpioa::PA0;
use minz::hal::gpio::{Edge, ExtiPin, Input, PullUp};
use minz::hal::lptimer::{ClockSource, Event, LowPowerTimer, LowPowerTimerConfig, PreScaler};
use minz::hal::pac::{LPTIM1, USART1, interrupt};
use minz::hal::prelude::*;
use minz::hal::serial::{Config, Serial, Tx};
use minz::hal::stm32;
use minz::hal::stm32::Interrupt;
use minz::hal::time::MonoTimer;
use minz::open_loop::{self, OPEN_LOOP_STEPS_PER_REV, Waveform};
use minz::softuart::{IrqAck, Rx, SoftUart};
use minz::tim1_motor_pwm::{self, max_duty};
use minz::{SYSCLK, SYSTICK};
use portable_atomic::{AtomicU32, Ordering};
use rtt_target::rprintln;

const BAUD: Hertz = Hertz::Hz(9600);
const OVERSAMPLE: usize = 4;
const SAMPLE: Hertz = Hertz::Hz(BAUD.raw() * OVERSAMPLE as u32);
const RX_BUF_LEN: usize = 32;

/// Amplitude clamps as % of full ARR swing. **Hard-capped at 20 %** because
/// open-loop sine drive has no rotor sync and no current limit — at low
/// electrical frequency the applied voltage divides across the milliohm
/// winding resistance and turns straight into copper losses (already cost
/// us one motor on this bench). 20 % at 5 V bench supply ≈ 1 V × 1/0.05 Ω
/// ≈ 20 A peak per phase, which is already plenty. Drop this further if
/// the bench supply goes back above 5 V.
///
/// Does **not** protect against e.g. duty getting stuck high if the loop
/// hangs, or shoot-through during dead-time misconfiguration — there are
/// other ways to fry the windings that this clamp doesn't cover.
const AMP_MIN: u16 = 0;
const AMP_MAX: u16 = 20;
/// Bench observation: at 5 V supply this motor refuses to start
/// (synchronise to the commanded field) below ~15 %. Set the default at
/// the empirical floor so the user doesn't have to ramp up after boot
/// just to confirm the loop is alive.
const AMP_START: u16 = 15;

/// Electrical-frequency clamps in Hz. 60 Hz × 7 pole-pairs ≈ 514 RPM mech.
/// Lower bound 1 to avoid divide-by-zero on the step-cycle compute.
const FREQ_MIN: u32 = 1;
const FREQ_MAX: u32 = 600;
const FREQ_START: u32 = 60;

/// PA6 (ADC1_IN11) → bench-supply voltage divider, scaled ×100.
/// Vimdrones L431 schematic: 30 kΩ from VBat to PA6, 3.6 kΩ from PA6
/// to GND. `V_bat = V_pa6 × (30 + 3.6) / 3.6 = V_pa6 × 9.333`.
///
/// (Bench cal under-read the schematic ratio by ~8 % at 5.39 V —
/// resistor tolerance; trust the schematic for now and add a fixed
/// trim if you actually need ±1 % accuracy.)
const VBAT_DIVIDER_X100: u32 = 933;

/// PA3 (ADC1_IN8) current-sense gain in mV per amp.
/// Vimdrones L431 uses an INA180B1 (gain **20 V/V**) on a **1.5 mΩ**
/// shunt: `V_pa3 = I × 1.5 mΩ × 20 = I × 30 mV/A`.
///
/// (Bench cal at 0.222 A read 5 mV ≈ 22.5 mV/A — 25 % lower than the
/// schematic predicts. PSU readout error / INA offset / shunt tol;
/// schematic is the honest answer until measured otherwise.)
const ISNS_MV_PER_AMP: u32 = 30;

/// DWT cycles between angle steps at a given electrical frequency.
const fn step_cycles(electrical_hz: u32) -> u32 {
    SYSCLK.raw() / (electrical_hz * OPEN_LOOP_STEPS_PER_REV)
}

type Uart = SoftUart<'static, { BAUD.raw() }, { SYSTICK.raw() }, OVERSAMPLE>;
type RxQueue = Queue<u8, RX_BUF_LEN>;

static TICKS_10US: AtomicU32 = AtomicU32::new(0);

type RxPin = PA0<Input<PullUp>>;
type RxTimer = LowPowerTimer<LPTIM1>;
type RxBundle = Rx<RxPin, RxTimer, Uart>;

static RX: Mutex<RefCell<Option<RxBundle>>> = Mutex::new(RefCell::new(None));

/// Bounded software byte queue + USART1 TX, used only from the main
/// context (no ISR access → plain `Deque`, not SPSC). `write!` macros
/// target this via the [`fmt::Write`] impl which just enqueues bytes —
/// no busy-wait on `TXE`. The motor loop calls [`service`] each idle
/// `nop` pass; that tries to shift exactly one byte from the queue head
/// to USART1's TDR via the *non-blocking* `embedded-hal` write. If TXE
/// isn't set we leave the byte and try again next pass.
///
/// Bytes are dropped if the queue is full — fine for status updates;
/// not OK if you wanted reliable delivery.
///
/// [`service`]: UartTxWriter::service
struct UartTxWriter {
    tx: Tx<USART1>,
    queue: Deque<u8, TX_QUEUE_LEN>,
}

const TX_QUEUE_LEN: usize = 64;

impl UartTxWriter {
    fn new(tx: Tx<USART1>) -> Self {
        Self {
            tx,
            queue: Deque::new(),
        }
    }

    /// Try to push one byte from the queue head to USART1.TDR. No-op if
    /// the queue is empty or TXE is not yet set.
    fn service(&mut self) {
        if let Some(&b) = self.queue.front()
            && self.tx.write(b).is_ok()
        {
            self.queue.pop_front();
        }
    }
}

impl core::fmt::Write for UartTxWriter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for b in s.bytes() {
            // Drop bytes on overflow — status messages aren't critical.
            let _ = self.queue.push_back(b);
        }
        Ok(())
    }
}

fn ticks_10us() -> u32 {
    TICKS_10US.load(Ordering::Relaxed)
}

#[entry]
fn main() -> ! {
    static mut RX_QUEUE: RxQueue = Queue::new();
    let cp = cortex_m::Peripherals::take().unwrap();
    let mut dp = stm32::Peripherals::take().unwrap();
    let BoardInit {
        mut cp,
        clocks,
        mut ahb2,
        mut apb1r1,
        mut apb2,
        mut ccipr,
        ..
    } = init(cp, dp.FLASH, dp.RCC, dp.PWR);

    rprintln!(
        "motor_tester: clocks sysclk={} pclk1={}",
        clocks.sysclk().raw(),
        clocks.pclk1().raw(),
    );

    configure_systick(cp.SYST, &clocks, SYSTICK);

    let mut gpioa = dp.GPIOA.split(&mut ahb2);
    let mut gpiob = dp.GPIOB.split(&mut ahb2);

    // Soft-UART RX on PA0: pull-up input + EXTI0 falling edge.
    let mut rx_pin = gpioa
        .pa0
        .into_pull_up_input(&mut gpioa.moder, &mut gpioa.pupdr);
    rx_pin.make_interrupt_source(&mut dp.SYSCFG, &mut apb2);
    rx_pin.trigger_on_edge(&mut dp.EXTI, Edge::Falling);
    rx_pin.enable_interrupt(&mut dp.EXTI);

    // Bench instrumentation: PA3 = ADC1_IN8 (supply current),
    // PA6 = ADC1_IN11 (battery voltage). Oneshot HAL reads, ~16 µs each.
    let pa3 = gpioa.pa3.into_analog(&mut gpioa.moder, &mut gpioa.pupdr);
    let pa6 = gpioa.pa6.into_analog(&mut gpioa.moder, &mut gpioa.pupdr);
    let mut sense_adc = SenseAdc::new(
        dp.ADC1,
        dp.ADC_COMMON,
        pa3,
        pa6,
        &mut ahb2,
        &mut ccipr,
        clocks,
    );

    // Motor PWM pins → TIM1 AF1 (AM32 pin map).
    configure_motor_pwm_pins(
        gpioa.pa7,
        gpioa.pa8,
        gpioa.pa9,
        gpioa.pa10,
        gpiob.pb0,
        gpiob.pb1,
        &mut gpioa.moder,
        &mut gpioa.otyper,
        &mut gpioa.afrl,
        &mut gpioa.afrh,
        &mut gpiob.moder,
        &mut gpiob.otyper,
        &mut gpiob.afrl,
    );

    // USART1 in half-duplex mode: PB6 only, open-drain alternate
    // function. The HAL's `Serial::usart1` accepts a 1-tuple `(tx,)`
    // when `tx` implements `TxHalfDuplexPin` (= AF + OpenDrain), so
    // PB7 stays unclaimed and is available for the COMP2 INM input.
    //
    // Internal pull-up is enabled because half-duplex relies on the
    // line idling high — the L431 only drives it low for start bits.
    // Most USB-TTL adapters also have an internal pull-up on RX, but
    // this keeps it working without that assumption.
    let mut usart_tx = gpiob.pb6.into_alternate_open_drain::<7>(
        &mut gpiob.moder,
        &mut gpiob.otyper,
        &mut gpiob.afrl,
    );
    usart_tx.internal_pull_up(&mut gpiob.pupdr, true);
    let serial = Serial::usart1(
        dp.USART1,
        (usart_tx,),
        Config::default().baudrate(BAUD.raw().bps()),
        clocks,
        &mut apb2,
    );
    let (mut tx, _) = serial.split();

    // TIM1 motor PWM init.
    tim1_motor_pwm::init(dp.TIM1, &mut apb2);

    // LPTIM1 for soft-UART sample rate (= OVERSAMPLE × BAUD).
    let lptim_ticks_per_sample = clocks.pclk1().raw() / SAMPLE.raw();
    let arr = (lptim_ticks_per_sample - 1) as u16;
    let cmp = arr / 2;
    let lptim_cfg = LowPowerTimerConfig::default()
        .clock_source(ClockSource::PCLK)
        .prescaler(PreScaler::U1)
        .arr_value(arr)
        .compare_value(cmp);
    let mut timer = LowPowerTimer::lptim1(dp.LPTIM1, lptim_cfg, &mut apb1r1, &mut ccipr, clocks);
    // `listen` toggles ENABLE to write IER, wiping ARR/CMP — re-program.
    timer.listen(Event::CompareMatch);
    timer.set_autoreload(arr);
    timer.set_compare_match(cmp);

    let (producer, mut consumer) = RX_QUEUE.split();

    free(|cs| {
        RX.borrow(cs)
            .replace(Some(Rx::new(rx_pin, timer, SoftUart::new(producer))));
    });

    // DWT for the open-loop step timing.
    cp.DCB.enable_trace();
    let mono = MonoTimer::new(cp.DWT, clocks);

    unsafe {
        NVIC::unmask(Interrupt::LPTIM1);
        NVIC::unmask(Interrupt::EXTI0);
        cortex_m::interrupt::enable();
    }

    // Greet the host — done BEFORE entering the motor loop so the
    // multi-line message doesn't stall PWM updates.
    // Boot help — sent via blocking `writeln!` while the motor isn't
    // running yet, so the per-byte stall doesn't matter. Once the motor
    // loop starts we switch to the non-blocking `UartTxWriter` so status
    // messages don't freeze PWM updates.
    writeln!(&mut tx, "\r\n=== Hello, this is motor tester ===\r").ok();
    writeln!(
        &mut tx,
        "Frequency (Hz):       d   +1, c -1, f +10, v -10\r"
    )
    .ok();
    writeln!(
        &mut tx,
        "Amplitude (% of ARR): a/A +1, z -1, S +10, x -10\r"
    )
    .ok();
    writeln!(&mut tx, "Mode toggle:          m   (sine <-> six-step)\r").ok();
    writeln!(
        &mut tx,
        "Panic reset:          r   (sine, f=start, amp=start)\r"
    )
    .ok();
    writeln!(
        &mut tx,
        "Kill output:          w   (clear MOE, all FETs off)\r"
    )
    .ok();
    writeln!(
        &mut tx,
        "Print sense:          i   (PA3 IN8 isns, PA6 IN11 vbat)\r"
    )
    .ok();
    writeln!(
        &mut tx,
        "Starting at f = {} Hz ({}..{}), amp = {} % (hard cap {})\r",
        FREQ_START, FREQ_MIN, FREQ_MAX, AMP_START, AMP_MAX,
    )
    .ok();

    let mut tx_writer = UartTxWriter::new(tx);

    let pwm_arr = max_duty();
    let mut waveform = Waveform::Sine;
    let mut angle: u16 = 0;
    let mut amplitude_pct: u16 = AMP_START;
    let mut electrical_hz: u32 = FREQ_START;
    let mut cycles_per_step: u32 = step_cycles(electrical_hz);
    // `w` clears this and `MOE` in BDTR; `r` re-arms. While false the
    // outer loop skips PWM writes (cheap optimisation — MOE=0 already
    // blocks the outputs at the timer regardless).
    let mut output_enabled = true;

    loop {
        if output_enabled {
            match waveform {
                Waveform::Sine | Waveform::Trapezoid => {
                    let (c1, c2, c3) = open_loop::duties(waveform, angle, pwm_arr, amplitude_pct);
                    tim1_motor_pwm::set_duties(c1, c2, c3);
                }
                Waveform::SixStep => {
                    // 6 sectors × 60° → angle/60. CCER changes only when
                    // the sector flips; CCRs are constant within a sector.
                    // Cheap enough to just rewrite per angle step.
                    let sector = open_loop::six_step_sector(angle);
                    let duty = open_loop::six_step_duty(pwm_arr, amplitude_pct);
                    tim1_motor_pwm::set_six_step(sector, duty);
                }
            }
        }

        let start = mono.now();
        while start.elapsed() < cycles_per_step {
            while let Some(b) = consumer.dequeue() {
                let prev_amp = amplitude_pct;
                let prev_hz = electrical_hz;
                let prev_mode = waveform;
                match b {
                    b'a' | b'A' => amplitude_pct = clamp_amp(amplitude_pct as i32 + 1),
                    b'z' => amplitude_pct = clamp_amp(amplitude_pct as i32 - 1),
                    b'S' => amplitude_pct = clamp_amp(amplitude_pct as i32 + 10),
                    b'x' => amplitude_pct = clamp_amp(amplitude_pct as i32 - 10),
                    b'd' => electrical_hz = clamp_hz(electrical_hz as i32 + 1),
                    b'c' => electrical_hz = clamp_hz(electrical_hz as i32 - 1),
                    b'f' => electrical_hz = clamp_hz(electrical_hz as i32 + 10),
                    b'v' => electrical_hz = clamp_hz(electrical_hz as i32 - 10),
                    b'm' => {
                        waveform = match waveform {
                            Waveform::Sine => Waveform::SixStep,
                            _ => Waveform::Sine,
                        };
                    }
                    b'r' => {
                        // Panic reset: back to a known-good idle config.
                        // The existing prev_amp / prev_hz / prev_mode delta
                        // checks below pick this up and print the changes.
                        waveform = Waveform::Sine;
                        amplitude_pct = AMP_START;
                        electrical_hz = FREQ_START;
                        if !output_enabled {
                            output_enabled = true;
                            tim1_motor_pwm::arm_output();
                            write!(&mut tx_writer, "armed\r\n").ok();
                        }
                    }
                    b'w' => {
                        // Hard kill: clear MOE → all FETs off immediately.
                        // CCRs / CCER are preserved so `r` resumes from
                        // the same waveform state.
                        if output_enabled {
                            output_enabled = false;
                            tim1_motor_pwm::all_off();
                            write!(&mut tx_writer, "off\r\n").ok();
                        }
                    }
                    b'i' => {
                        // Oneshot reads on both instrumentation pins
                        // (~16 µs each). Convert to engineering units
                        // using the empirical bench calibration above.
                        let i_raw = sense_adc.isns_raw();
                        let i_mv = sense_adc.adc_to_mv(i_raw);
                        let i_ma = i_mv as u32 * 1000 / ISNS_MV_PER_AMP;
                        let v_raw = sense_adc.vbat_raw();
                        let v_mv = sense_adc.adc_to_mv(v_raw);
                        let v_supply_mv = v_mv as u32 * VBAT_DIVIDER_X100 / 100;
                        write!(
                            &mut tx_writer,
                            "vbat={}.{:03}V isns={}.{:03}A (raw v={} i={})\r\n",
                            v_supply_mv / 1000,
                            v_supply_mv % 1000,
                            i_ma / 1000,
                            i_ma % 1000,
                            v_raw,
                            i_raw,
                        )
                        .ok();
                    }
                    _ => {}
                }
                // Non-blocking: bytes go into the writer's queue and
                // drain one-per-`service()` from the busy-wait below.
                if amplitude_pct != prev_amp {
                    write!(&mut tx_writer, "amp={}\r\n", amplitude_pct).ok();
                }
                if electrical_hz != prev_hz {
                    cycles_per_step = step_cycles(electrical_hz);
                    write!(&mut tx_writer, "f={}\r\n", electrical_hz).ok();
                }
                if waveform != prev_mode {
                    // Leaving 6-step: re-enable the floating phase so the
                    // next continuous-drive write isn't stuck Hi-Z.
                    if matches!(prev_mode, Waveform::SixStep) {
                        tim1_motor_pwm::enable_all_phases();
                    }
                    write!(&mut tx_writer, "mode={}\r\n", mode_name(waveform)).ok();
                }
            }
            tx_writer.service();
            cortex_m::asm::nop();
        }

        angle = open_loop::advance_angle(angle, true);
    }
}

#[inline]
fn mode_name(w: Waveform) -> &'static str {
    match w {
        Waveform::Sine => "sine",
        Waveform::Trapezoid => "trapezoid",
        Waveform::SixStep => "six-step",
    }
}

#[inline]
fn clamp_amp(v: i32) -> u16 {
    v.clamp(AMP_MIN as i32, AMP_MAX as i32) as u16
}

#[inline]
fn clamp_hz(v: i32) -> u32 {
    v.clamp(FREQ_MIN as i32, FREQ_MAX as i32) as u32
}

#[exception]
fn SysTick() {
    TICKS_10US.fetch_add(1, Ordering::Relaxed);
}

#[interrupt]
fn EXTI0() {
    free(|cs| {
        let mut rx_borrow = RX.borrow(cs).borrow_mut();
        let Some(rx) = rx_borrow.as_mut() else { return };
        rx.pin.clear_interrupt_pending_bit();
        rx.uart.on_falling_edge(ticks_10us());
    });
}

#[interrupt]
fn LPTIM1() {
    free(|cs| {
        let mut rx_borrow = RX.borrow(cs).borrow_mut();
        let Some(rx) = rx_borrow.as_mut() else { return };
        rx.timer.ack();
        rx.uart.on_sample(rx.pin.is_high());
    });
}
