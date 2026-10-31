//! Busy-wait delays on the free-running SysTick (core clock, 24-bit down
//! counter, interrupt off).
//!
//! AM32 `delayMicros` (`Src/functions.c`) waits on a 1 us utility timer.
//! `cortex_m::asm::delay(n)` spins `n` loop passes of about 3 (M4) to 4 (M0)
//! cycles, so the former `asm::delay(us * MHz)` waited 3-4x too long, which
//! stretched sine-start steps and tones on F051/G071/G431.

const SYST_MASK: u32 = 0x00FF_FFFF;

/// Start SysTick free-running at the core clock with its interrupt off,
/// unless it already runs (the bench tick-gap probe and recorder read the
/// same counter in the same configuration).
pub fn systick_free_run() {
    // SAFETY: SysTick has no handler in this firmware; this only starts the
    // counter, and only when it is stopped.
    unsafe {
        let st = &*cortex_m::peripheral::SYST::PTR;
        if st.csr.read() & 1 == 0 {
            st.rvr.write(SYST_MASK);
            st.cvr.write(0);
            st.csr.write(0b101); // CLKSOURCE = core, TICKINT = 0, ENABLE
        }
    }
}

/// Wait at least `us` microseconds at the active chip's core clock. Elapsed
/// cycles accumulate between reads, so any length works as long as the loop
/// is not preempted for a whole SysTick period (2^24 cycles, >= 98 ms).
pub fn delay_us(us: u32) {
    use crate::mcu::{Chip, ChipConfig};
    systick_free_run();
    // SAFETY: read-only access to the free-running SysTick counter.
    let st = unsafe { &*cortex_m::peripheral::SYST::PTR };
    let mut remaining = us.saturating_mul(Chip::CPU_FREQUENCY_MHZ);
    let mut prev = st.cvr.read();
    while remaining > 0 {
        let now = st.cvr.read();
        remaining = remaining.saturating_sub(prev.wrapping_sub(now) & SYST_MASK);
        prev = now;
    }
}
