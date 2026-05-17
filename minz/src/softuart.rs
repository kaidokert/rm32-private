//! Soft-UART RX: state machine, byte FIFO, and the small HAL adapter traits
//! / wrapper struct used by binaries to plug it into stm32l4xx-hal.
//!
//! The pure state machine (`SoftUart<...>`, `FrameStep`) has no HAL deps —
//! it's a hardware-agnostic decoder.
//!
//! The L4-specific glue (`SoftUartPin`, `IrqAck`, `RxHw`) lives in the
//! same module for convenience since the rest of `minz` is already
//! stm32l4xx-hal-bound. Binaries pick the concrete `P` and `T` via type
//! aliases and the wiring code in `main`.
//!
//! The driver is split into two callback entry points and a poll API:
//!
//! - [`SoftUart::on_falling_edge`] — call from the RX-line edge ISR. Decides
//!   whether the falling edge qualifies as the start bit of a fresh frame.
//! - [`SoftUart::on_sample`] — call from the bit-sample timer ISR with the
//!   pin level just read. Returns whether the frame has just completed.
//! - [`SoftUart::peek`] / [`SoftUart::pop`] — read decoded bytes from the
//!   internal FIFO, the way a real hardware UART's RDR works.
//!
//! ## Generic parameters
//!
//! - `BAUD` — RX bit rate in Hz (e.g. `9600`).
//! - `TICK_HZ` — frequency of the timestamp source used for inter-frame
//!   idle detection. Only used in [`Self::FRAME_GAP_TICKS`]; needs ≥ ~3×
//!   `BAUD` so 12 bit times round to a non-trivial integer.
//! - `OVERSAMPLE` — samples per bit. 4 or 8 are typical. Affects voting
//!   threshold.
//!
//! ## What this crate is NOT responsible for
//!
//! - Owning the GPIO pin, EXTI config, or sample timer. The caller wires
//!   the ISRs, reads the pin, and starts/stops the sample timer.
//! - **Owning the RX byte FIFO.** [`SoftUart::on_sample`] hands a decoded
//!   byte back via [`FrameStep::Byte`]; the caller pushes it to whatever
//!   queue/storage it prefers (e.g. `heapless::spsc::Queue` for
//!   lock-free producer/consumer split between ISR and main loop).
//! - Picking the sample timer phase. Half a sample period is typical; the
//!   caller computes the timer's initial CNT value.

use core::convert::Infallible;

use heapless::spsc::Producer;

use crate::hal::gpio::ExtiPin;
// `stm32l4xx_hal::hal` is its re-export of `embedded-hal` 0.2 (see
// `stm32l4xx-hal/src/lib.rs`: `pub use embedded_hal as hal;`). Going via
// the alias keeps the version pinned to whatever the HAL itself uses,
// which is the v2 `InputPin` trait — not the unrelated `embedded-hal` 1.0
// we depend on directly.
use crate::hal::hal::digital::v2::InputPin;

// ---------------------------------------------------------------------------
// HAL adapter traits + struct
// ---------------------------------------------------------------------------

/// What the soft-UART RX driver needs from a GPIO pin: digital-input level
/// reads (`embedded_hal::digital::v2::InputPin`) and per-pin EXTI control
/// (`stm32l4xx_hal::gpio::ExtiPin`). Any pin implementing both fits.
pub trait SoftUartPin: InputPin<Error = Infallible> + ExtiPin {}
impl<T> SoftUartPin for T where T: InputPin<Error = Infallible> + ExtiPin {}

/// Tiny "ack the peripheral's pending IRQ flag" trait.
///
/// Neither `embedded-hal` nor `stm32l4xx-hal` defines a generic version of
/// this even though every interrupt handler needs it — the register and
/// bit are wildly per-peripheral, but the *operation* (clear the flag so
/// the ISR doesn't immediately re-fire) is universal.
///
/// In this crate the soft-UART RX path uses it on its bit-sample timer:
/// binaries wire that timer as always-running at `OVERSAMPLE × BAUD`,
/// gate "are we currently receiving" with a software flag, and the ISR
/// just needs to ack the timer flag on every entry. Constant WCET, no
/// surprises when traffic starts/stops, and the trait stays portable
/// across timer types that don't support pausing / CNT-phase tricks
/// (e.g. LPTIM).
pub trait IrqAck {
    /// Clear the peripheral's pending interrupt flag.
    fn ack(&mut self);
}

/// Pin + bit-sample timer pair owned by the binary and shared between the
/// EXTI and TIM ISRs. Wrap in `Mutex<RefCell<Option<RxHw<P, T>>>>` for
/// cross-ISR access.
pub struct RxHw<P: SoftUartPin, T: IrqAck> {
    pub pin: P,
    pub timer: T,
}

impl<P: SoftUartPin, T: IrqAck> RxHw<P, T> {
    pub const fn new(pin: P, timer: T) -> Self {
        Self { pin, timer }
    }
}

// ---------------------------------------------------------------------------
// Pure decoder state machine
// ---------------------------------------------------------------------------

/// SoftUart owns the RX-byte queue producer half (the consumer stays in
/// `main` for lock-free `dequeue`). The `'a` lifetime is the lifetime of
/// the underlying queue's storage — usually `'static` when the queue is
/// in a `static mut`. Lifting it as a parameter avoids hardcoding to one
/// queue layout while keeping the API trivial: callers feed every sample
/// blindly into [`on_sample`] and SoftUart enqueues internally when a
/// byte is ready.
pub struct SoftUart<'a, const BAUD: u32, const TICK_HZ: u32, const OVERSAMPLE: usize> {
    bit_index: u32,
    oversample_idx: u32,
    accum: u32,
    byte_build: u8,
    last_edge_ticks: u32,
    framing_errors: u32,
    frame_count: u32,
    overrun_count: u32,
    /// See [`Self::on_falling_edge`] / [`Self::on_sample`]. When false the
    /// timer ISR's blind `on_sample` calls return immediately.
    is_receiving: bool,
    /// Producer half of the caller's SPSC RX queue. SoftUart pushes
    /// decoded bytes here; the consumer half lives in `main` and is
    /// drained lock-free.
    producer: Producer<'a, u8>,
}

impl<'a, const BAUD: u32, const TICK_HZ: u32, const OVERSAMPLE: usize>
    SoftUart<'a, BAUD, TICK_HZ, OVERSAMPLE>
{
    /// 12 bit times in ticks of `TICK_HZ`. A falling edge that arrives at
    /// least this long after the previous falling edge is treated as the
    /// start bit of a new frame; otherwise it's a mid-frame transition.
    ///
    /// 12 bit times is comfortably past the worst-case 9-bit-times in-frame
    /// no-edge stretch (e.g. data `0xFF`), with margin.
    pub const FRAME_GAP_TICKS: u32 = (12 * TICK_HZ).div_ceil(BAUD);

    /// Build a SoftUart from the producer half of a caller-owned SPSC
    /// queue. Not `const fn` because `producer` is constructed at runtime
    /// from `Queue::split()` — store the result in
    /// `Mutex<RefCell<Option<SoftUart<...>>>>` and initialize once in main.
    pub fn new(producer: Producer<'a, u8>) -> Self {
        Self {
            bit_index: 0,
            oversample_idx: 0,
            accum: 0,
            byte_build: 0,
            last_edge_ticks: 0,
            framing_errors: 0,
            frame_count: 0,
            overrun_count: 0,
            is_receiving: false,
            producer,
        }
    }

    /// Feed a falling-edge event into the decoder. Returns `true` iff this
    /// edge qualifies as a frame start; in that case `is_receiving` is also
    /// flipped on internally so subsequent [`on_sample`] calls do real work
    /// instead of returning `None`.
    pub fn on_falling_edge(&mut self, now_ticks: u32) -> bool {
        let elapsed = now_ticks.wrapping_sub(self.last_edge_ticks);
        self.last_edge_ticks = now_ticks;
        if elapsed >= Self::FRAME_GAP_TICKS {
            self.frame_count = self.frame_count.wrapping_add(1);
            self.reset_decoder();
            self.is_receiving = true;
            true
        } else {
            false
        }
    }

    /// Feed one bit-sample-time pin read into the decoder. Cheap no-op
    /// until `on_falling_edge` qualifies a start, after which it walks
    /// the bit-shift state machine and pushes the decoded byte to the
    /// caller's RX queue on success. Bumps `framing_errors` on a bad
    /// start/stop bit and `overrun_count` if the queue is full.
    pub fn on_sample(&mut self, pin_high: bool) {
        if !self.is_receiving {
            return;
        }
        self.accum += pin_high as u32;
        self.oversample_idx += 1;
        if (self.oversample_idx as usize) < OVERSAMPLE {
            return;
        }
        // Bit complete — majority vote across OVERSAMPLE samples.
        let bit: u8 = if (self.accum as usize) * 2 >= OVERSAMPLE {
            1
        } else {
            0
        };
        self.accum = 0;
        self.oversample_idx = 0;

        match self.bit_index {
            0 => {
                if bit != 0 {
                    self.framing_errors = self.framing_errors.wrapping_add(1);
                    self.reset_decoder();
                    self.is_receiving = false;
                    return;
                }
            }
            1..=8 => {
                self.byte_build |= bit << (self.bit_index - 1);
            }
            _ => {
                let byte = self.byte_build;
                let stop_ok = bit == 1;
                self.reset_decoder();
                self.is_receiving = false;
                if stop_ok {
                    if self.producer.enqueue(byte).is_err() {
                        self.overrun_count = self.overrun_count.wrapping_add(1);
                    }
                } else {
                    self.framing_errors = self.framing_errors.wrapping_add(1);
                }
                return;
            }
        }
        self.bit_index += 1;
    }

    fn reset_decoder(&mut self) {
        self.bit_index = 0;
        self.oversample_idx = 0;
        self.accum = 0;
        self.byte_build = 0;
    }

    pub fn framing_errors(&self) -> u32 {
        self.framing_errors
    }

    pub fn frame_count(&self) -> u32 {
        self.frame_count
    }

    /// Bytes the decoder produced but couldn't push because the caller's
    /// RX queue was full.
    pub fn overrun_count(&self) -> u32 {
        self.overrun_count
    }
}
