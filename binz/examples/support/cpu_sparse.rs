//! Diagnostic-only sparse IRQ bracketing. It never owns gate/control state.
//! Every 17th entry of each vector is timed; the other 16 do no clock read.
//! Each vector owns one row, so nested different-priority IRQs do not race
//! over a row. Brackets are inclusive of preemption; sums are NOT IRQ union.
use super::*;

const ROOTS: usize = 4; // TIM6 guard, ADC_COMP, TIM16 COM, DMA1_CH1
const PERIOD: u8 = 17; // coprime with six electrical sectors
static mut CALLS: [u32; ROOTS] = [0; ROOTS];
static mut COUNTDOWN: [u8; ROOTS] = [0; ROOTS];
static mut SAMPLES: [u32; ROOTS] = [0; ROOTS];
static mut SUM_US: [u32; ROOTS] = [0; ROOTS];
static mut MAX_US: [u16; ROOTS] = [0; ROOTS];
static mut FIRST_US: u32 = u32::MAX;

pub fn reset() {
    // Called before ownership transfer, with the powered IRQs disabled.
    unsafe {
        core::ptr::addr_of_mut!(CALLS).write([0; ROOTS]);
        core::ptr::addr_of_mut!(COUNTDOWN).write([0; ROOTS]);
        core::ptr::addr_of_mut!(SAMPLES).write([0; ROOTS]);
        core::ptr::addr_of_mut!(SUM_US).write([0; ROOTS]);
        core::ptr::addr_of_mut!(MAX_US).write([0; ROOTS]);
        core::ptr::addr_of_mut!(FIRST_US).write(u32::MAX);
    }
}

pub struct Scope<const ID: usize> {
    start: u16,
    sampled: bool,
}

#[inline(always)]
pub fn enter<const ID: usize>() -> Scope<ID> {
    if !powered_timer::owns() || core_bench::live_duty_current().unwrap_or(0) < 350 {
        return Scope {
            start: 0,
            sampled: false,
        };
    }
    unsafe {
        let calls = core::ptr::addr_of_mut!(CALLS).cast::<u32>().add(ID);
        calls.write(calls.read().wrapping_add(1));
        let countdown = core::ptr::addr_of_mut!(COUNTDOWN).cast::<u8>().add(ID);
        let n = countdown.read();
        if n != 0 {
            countdown.write(n - 1);
            return Scope {
                start: 0,
                sampled: false,
            };
        }
        countdown.write(PERIOD - 1);
        let samples = core::ptr::addr_of_mut!(SAMPLES).cast::<u32>().add(ID);
        samples.write(samples.read().wrapping_add(1));
        if core::ptr::addr_of!(FIRST_US).read() == u32::MAX {
            // One extended timestamp at the first sample only. The ordinary
            // guard keeps this clock fed; POWERPATH stop_us is the end stamp.
            core::ptr::addr_of_mut!(FIRST_US).write(powered_timer::stream_now());
        }
    }
    Scope {
        start: t17(),
        sampled: true,
    }
}

impl<const ID: usize> Drop for Scope<ID> {
    #[inline(always)]
    fn drop(&mut self) {
        if !self.sampled {
            return;
        }
        let duration = t17().wrapping_sub(self.start);
        unsafe {
            let sum = core::ptr::addr_of_mut!(SUM_US).cast::<u32>().add(ID);
            sum.write(sum.read().saturating_add(duration as u32));
            let max = core::ptr::addr_of_mut!(MAX_US).cast::<u16>().add(ID);
            max.write(max.read().max(duration));
        }
    }
}

pub fn dump<W: Write>(out: &mut W) {
    if powered_timer::owns() || !powered_timer::outputs_disabled() {
        return;
    }
    let first = unsafe { core::ptr::addr_of!(FIRST_US).read() };
    let _ = writeln!(
        out,
        "SPARSECPU first_us={} period={} roots=4 inclusive=1 union=0 diagnostic_only=1",
        first, PERIOD
    );
    for id in 0..ROOTS {
        let calls = unsafe { core::ptr::addr_of!(CALLS).cast::<u32>().add(id).read() };
        let samples = unsafe { core::ptr::addr_of!(SAMPLES).cast::<u32>().add(id).read() };
        let sum = unsafe { core::ptr::addr_of!(SUM_US).cast::<u32>().add(id).read() };
        let max = unsafe { core::ptr::addr_of!(MAX_US).cast::<u16>().add(id).read() };
        let _ = writeln!(
            out,
            "SPARSEROW id={} calls={} sampled={} sum_us={} max_us={}",
            id, calls, samples, sum, max
        );
    }
}
