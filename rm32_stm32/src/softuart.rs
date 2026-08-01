//! Soft-UART RX: pure decoder state machine + byte FIFO producer.
//!
//! Resurrected from the minz bench era (git 8d2ff8c^, where it ran
//! validated on this exact wiring: USB-TTL TX -> PA0, 9600 8N1). The
//! HAL adapter traits from that version are dropped — rm32's L431 glue
//! (`mcu_l431::softuart_rx`) drives the decoder with direct PAC reads
//! from its EXTI0 / LPTIM1 ISRs instead.
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

use heapless::spsc::Producer;

/// RX byte-queue capacity (heapless 0.8 carries it in the type).
pub const RX_QUEUE_LEN: usize = 64;

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
    /// True when the previous frame ended in a valid stop bit. A clean
    /// end means the very next falling edge IS a legitimate start bit —
    /// back-to-back bytes in a continuous stream have sub-gap spacing
    /// between the last data edge and the next start. After a framing
    /// error this goes false and start qualification falls back to the
    /// idle-gap heuristic (noise resync). Without this, only the FIRST
    /// byte of every burst decodes (observed: 7 frames for 19 sent —
    /// the minz era masked it by pacing 1 byte per 2 s).
    last_frame_clean: bool,
    /// Producer half of the caller's SPSC RX queue. SoftUart pushes
    /// decoded bytes here; the consumer half lives in `main` and is
    /// drained lock-free.
    producer: Producer<'a, u8, RX_QUEUE_LEN>,
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
    pub fn new(producer: Producer<'a, u8, RX_QUEUE_LEN>) -> Self {
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
            last_frame_clean: true,
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
        let qualifies = if self.is_receiving {
            // Mid-frame data edge — never a start.
            false
        } else if self.last_frame_clean {
            // Previous frame closed on a valid stop bit: the line is in
            // a known state and this edge IS the next start bit, however
            // tight the spacing (back-to-back stream support).
            true
        } else {
            // Post-error resync: require a real idle gap.
            elapsed >= Self::FRAME_GAP_TICKS
        };
        if qualifies {
            self.frame_count = self.frame_count.wrapping_add(1);
            self.reset_decoder();
            self.is_receiving = true;
        }
        qualifies
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
        // The STOP bit is decided EARLY, at half the samples: the frame
        // must close before the next byte's start edge in a
        // back-to-back stream, or that edge lands while `is_receiving`
        // and is misclassified as a mid-frame transition (the burst-
        // corruption mode). Data/start bits use the full window.
        let needed = if self.bit_index > 8 {
            OVERSAMPLE / 2
        } else {
            OVERSAMPLE
        };
        if (self.oversample_idx as usize) < needed {
            return;
        }
        // Bit complete — majority vote across the sampled window.
        let bit: u8 = if (self.accum as usize) * 2 >= needed {
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
                    self.last_frame_clean = false;
                    return;
                }
            }
            1..=8 => {
                self.byte_build |= bit << (self.bit_index - 1);
            }
            _ => {
                let byte = self.byte_build;
                let stop_ok = bit == 1;
                // (bit was decided early — see the oversample gate above)
                self.reset_decoder();
                self.is_receiving = false;
                self.last_frame_clean = stop_ok;
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
