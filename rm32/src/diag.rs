//! Diagnostic sink: the one seam through which bench observability and
//! bench fault injection reach the control path.
//!
//! Every hook defaults to an `#[inline(always)]` no-op, and `()` implements
//! the trait with nothing else, so a production build that plugs in `()`
//! carries no counter, no load and no branch for any of it. A bench build
//! plugs in its own sink (counters, cycle brackets, injection).

use crate::commutation::Commutation;
use crate::transfer::TransferActions;

/// Where an ISR (or main-loop) cycle bracket was taken.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IsrSite {
    /// The 20 kHz control tick.
    Tick,
    /// Commutation timer expiry.
    Commutation,
    /// BEMF zero-cross (comparator edge).
    ZeroCross,
    /// Input-capture DMA transfer complete.
    InputDma,
    /// Input frame processing (software-triggered after the DMA).
    InputFrame,
    /// One main-loop iteration body.
    MainLoop,
}

pub trait DiagSink {
    /// Cycle stamp at bracket entry (0 when the sink keeps no timing).
    #[inline(always)]
    fn stamp(&self) -> u32 {
        0
    }

    /// Close a bracket opened with [`stamp`](Self::stamp): record and return
    /// the elapsed cycles (0 when the sink keeps no timing).
    #[inline(always)]
    fn bracket(&self, _site: IsrSite, _start: u32) -> u32 {
        0
    }

    /// A decoded input frame: `crc_pass` is a valid DShot throttle or
    /// command; `dshot` reads whether DShot mode is active (only consulted
    /// when the sink counts failures).
    #[inline(always)]
    fn input_frame(
        &self,
        _actions: &TransferActions,
        _crc_pass: bool,
        _dshot: impl FnOnce() -> bool,
    ) {
    }

    /// A DShot command frame arrived.
    #[inline(always)]
    fn dshot_command(&self, _cmd: u8) {}

    /// Bidirectional DShot was detected.
    #[inline(always)]
    fn bidir_detected(&self) {}

    /// Commutation-timer ISR entry, before the step logic: the bench hook
    /// for fault injection (e.g. [`Commutation::inject_skip`]).
    #[inline(always)]
    fn on_com_step(&self, _commutation: &mut Commutation) {}
}

/// Production sink: every hook compiles away.
impl DiagSink for () {}
