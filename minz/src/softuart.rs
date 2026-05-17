//! Soft-UART RX: state machine, byte FIFO, and the small HAL adapter traits
//! / wrapper struct used by binaries to plug it into stm32l4xx-hal.
//!
//! The pure state machine (`SoftUart<...>`, `FrameStep`) has no HAL deps —
//! it's a hardware-agnostic decoder.
//!
//! The L4-specific glue (`SoftUartPin`, `BitSampleTimer`, `RxHw`) lives in
//! the same module for convenience since the rest of `minz` is already
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
//! - `RX_BUF_LEN` — depth of the RX byte FIFO.
//!
//! ## What this crate is NOT responsible for
//!
//! - Owning the GPIO pin, EXTI config, or sample timer. The caller wires
//!   the ISRs, reads the pin, and starts/stops the sample timer.
//! - Picking the sample timer phase. Half a sample period is typical; the
//!   caller computes the timer's initial CNT value.
//! - Tracking framing errors *per byte*. Errors bump a counter; the byte
//!   in flight is dropped and the decoder resyncs on the next start bit.

use core::convert::Infallible;

use heapless::Deque;

use crate::hal::gpio::ExtiPin;
use crate::hal::prelude::*; // brings `InputPin` (digital::v2) into scope

// ---------------------------------------------------------------------------
// HAL adapter traits + struct
// ---------------------------------------------------------------------------

/// What the soft-UART RX driver needs from a GPIO pin: digital-input level
/// reads (`embedded_hal::digital::v2::InputPin`) and per-pin EXTI control
/// (`stm32l4xx_hal::gpio::ExtiPin`). Any pin implementing both fits.
pub trait SoftUartPin: InputPin<Error = Infallible> + ExtiPin {}
impl<T> SoftUartPin for T where T: InputPin<Error = Infallible> + ExtiPin {}

/// What the soft-UART RX driver needs from its bit-sample timer. Binaries
/// pick which `Timer<TIMx>` (or other type) plays this role; the only impl
/// shipped in `minz` is for `Timer<TIM2>` (see `softuart_tim2.rs`).
pub trait BitSampleTimer {
    /// Re-arm the timer with `cnt` as the initial counter value. Impl is
    /// expected to do whatever is needed atomically from the caller's POV —
    /// typically pause + set CNT + clear pending flags + resume — so the
    /// next update event fires `(ARR - cnt + 1)` ticks later. Used by the
    /// EXTI ISR to seed half-period phase at a qualifying start bit.
    fn reload(&mut self, cnt: u32);

    /// Stop the counter until the next `reload`. Used by the bit-sample ISR
    /// at the end of a frame.
    fn pause(&mut self);

    /// Clear the timer's update-event pending flag. Called at the top of the
    /// bit-sample ISR. Concrete `Timer<TIM2>` exposes this as an inherent
    /// HAL method which wins method resolution; generic `T: BitSampleTimer`
    /// consumers reach the same operation through this trait method.
    #[allow(dead_code)]
    fn clear_update_interrupt_flag(&mut self);
}

/// Pin + bit-sample timer pair owned by the binary and shared between the
/// EXTI and TIM ISRs. Wrap in `Mutex<RefCell<Option<RxHw<P, T>>>>` for
/// cross-ISR access.
pub struct RxHw<P: SoftUartPin, T: BitSampleTimer> {
    pub pin: P,
    pub timer: T,
}

impl<P: SoftUartPin, T: BitSampleTimer> RxHw<P, T> {
    pub const fn new(pin: P, timer: T) -> Self {
        Self { pin, timer }
    }
}

// ---------------------------------------------------------------------------
// Pure decoder state machine + FIFO
// ---------------------------------------------------------------------------

/// Result of feeding one sample into the decoder.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum FrameStep {
    /// More samples needed for this frame. Caller should keep the sample
    /// timer running.
    Continue,
    /// Frame ended this sample — either a byte was pushed to the FIFO or
    /// a framing error was recorded. Caller should pause the sample timer
    /// until the next start edge.
    Complete,
}

pub struct SoftUart<
    const BAUD: u32,
    const TICK_HZ: u32,
    const OVERSAMPLE: usize,
    const RX_BUF_LEN: usize,
> {
    bit_index: u32,
    oversample_idx: u32,
    accum: u32,
    byte_build: u8,
    last_edge_ticks: u32,
    rx_buf: Deque<u8, RX_BUF_LEN>,
    framing_errors: u32,
    frame_count: u32,
    overrun_count: u32,
}

impl<const BAUD: u32, const TICK_HZ: u32, const OVERSAMPLE: usize, const RX_BUF_LEN: usize>
    SoftUart<BAUD, TICK_HZ, OVERSAMPLE, RX_BUF_LEN>
{
    /// 12 bit times in ticks of `TICK_HZ`. A falling edge that arrives at
    /// least this long after the previous falling edge is treated as the
    /// start bit of a new frame; otherwise it's a mid-frame transition.
    ///
    /// 12 bit times is comfortably past the worst-case 9-bit-times in-frame
    /// no-edge stretch (e.g. data `0xFF`), with margin.
    pub const FRAME_GAP_TICKS: u32 = (12 * TICK_HZ).div_ceil(BAUD);

    pub const fn new() -> Self {
        Self {
            bit_index: 0,
            oversample_idx: 0,
            accum: 0,
            byte_build: 0,
            last_edge_ticks: 0,
            rx_buf: Deque::new(),
            framing_errors: 0,
            frame_count: 0,
            overrun_count: 0,
        }
    }

    /// Feed a falling-edge event into the decoder. Returns `true` iff this
    /// edge qualifies as a frame start — the caller should then arm the
    /// bit-sample timer (with a half-period CNT seed to land samples near
    /// bit centres).
    pub fn on_falling_edge(&mut self, now_ticks: u32) -> bool {
        let elapsed = now_ticks.wrapping_sub(self.last_edge_ticks);
        self.last_edge_ticks = now_ticks;
        if elapsed >= Self::FRAME_GAP_TICKS {
            self.frame_count = self.frame_count.wrapping_add(1);
            self.reset_decoder();
            true
        } else {
            false
        }
    }

    /// Feed one bit-sample-time pin read into the decoder. Caller pauses
    /// the sample timer when this returns [`FrameStep::Complete`].
    pub fn on_sample(&mut self, pin_high: bool) -> FrameStep {
        self.accum += pin_high as u32;
        self.oversample_idx += 1;
        if (self.oversample_idx as usize) < OVERSAMPLE {
            return FrameStep::Continue;
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
                    return FrameStep::Complete;
                }
            }
            1..=8 => {
                self.byte_build |= bit << (self.bit_index - 1);
            }
            _ => {
                if bit == 1 {
                    if self.rx_buf.push_back(self.byte_build).is_err() {
                        self.overrun_count = self.overrun_count.wrapping_add(1);
                    }
                } else {
                    self.framing_errors = self.framing_errors.wrapping_add(1);
                }
                self.reset_decoder();
                return FrameStep::Complete;
            }
        }
        self.bit_index += 1;
        FrameStep::Continue
    }

    fn reset_decoder(&mut self) {
        self.bit_index = 0;
        self.oversample_idx = 0;
        self.accum = 0;
        self.byte_build = 0;
    }

    /// Look at the oldest pending byte without removing it.
    pub fn peek(&self) -> Option<u8> {
        self.rx_buf.front().copied()
    }

    /// Remove and return the oldest pending byte.
    pub fn pop(&mut self) -> Option<u8> {
        self.rx_buf.pop_front()
    }

    pub fn bytes_available(&self) -> usize {
        self.rx_buf.len()
    }

    pub fn framing_errors(&self) -> u32 {
        self.framing_errors
    }

    pub fn frame_count(&self) -> u32 {
        self.frame_count
    }

    /// Count of bytes dropped because the FIFO was full when they decoded.
    pub fn overrun_count(&self) -> u32 {
        self.overrun_count
    }
}

impl<const BAUD: u32, const TICK_HZ: u32, const OVERSAMPLE: usize, const RX_BUF_LEN: usize> Default
    for SoftUart<BAUD, TICK_HZ, OVERSAMPLE, RX_BUF_LEN>
{
    fn default() -> Self {
        Self::new()
    }
}
