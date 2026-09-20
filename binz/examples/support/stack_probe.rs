//! Boot-only paint below current MSP, never live stack frames. Diagnostic
//! watermark, not an MPU or proof against every untested interrupt nesting.
use core::fmt::Write;
unsafe extern "C" {
    static _stack_end: u32;
    static _stack_start: u32;
}
const MARK: u32 = 0xa57ac39d;
static mut PAINT_END: usize = 0;
#[inline(never)]
pub fn paint() {
    cortex_m::interrupt::free(|_| unsafe {
        let low = core::ptr::addr_of!(_stack_end) as usize;
        let high = core::ptr::addr_of!(_stack_start) as usize;
        let end = (cortex_m::register::msp::read() as usize).saturating_sub(64) & !3;
        if end < low || end > high {
            return;
        }
        for address in (low..end).step_by(4) {
            (address as *mut u32).write_volatile(MARK);
        }
        PAINT_END = end;
    });
}
pub fn report<W: Write>(out: &mut W) {
    let (span, painted, untouched) = cortex_m::interrupt::free(|_| unsafe {
        let low = core::ptr::addr_of!(_stack_end) as usize;
        let high = core::ptr::addr_of!(_stack_start) as usize;
        let end = PAINT_END;
        let mut next = low;
        while next < end && (next as *const u32).read_volatile() == MARK {
            next += 4;
        }
        (high - low, end.saturating_sub(low), next - low)
    });
    let _ = writeln!(
        out,
        "STACK span={} painted={} untouched={} diagnostic_only=1",
        span, painted, untouched
    );
}
