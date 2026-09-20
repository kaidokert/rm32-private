//! Diagnostic CH1 mirror of filteredTI2. No CH2 flag or controller writes.
//! Serialized by PRIMASK. Prefix only; timestamp is latest capture, not IRQ origin.
use super::*;
static mut ACTIVE: bool = false;
static mut EPOCH: u32 = 0;
static mut N: usize = 0;
static mut ROWS: [[u32; 10]; 24] = [[0; 10]; 24];
pub fn invalidate() {
    cortex_m::interrupt::free(|_| unsafe {
        ACTIVE = false;
        EPOCH = EPOCH.wrapping_add(1);
        let t = &*stm32::TIM2::ptr();
        t.ccer().modify(|r, w| w.bits(r.bits() & !1));
        t.sr().write(|w| w.bits(!(1 << 1 | 1 << 9)));
    });
}
pub fn prepare() {
    cortex_m::interrupt::free(|_| unsafe {
        invalidate();
        N = 0;
        // CC1S2=indirectTI2. E324/E337 prove sharedfilter, independent acknowledgement.
        (*stm32::TIM2::ptr())
            .ccmr1_input()
            .modify(|r, w| w.bits((r.bits() & !0xff) | 2));
    });
}
/// Called only after the source validates its current phase ticket.
pub fn arm(rising: bool) {
    cortex_m::interrupt::free(|_| unsafe {
        invalidate();
        (*stm32::TIM2::ptr())
            .ccer()
            .modify(|r, w| w.bits((r.bits() & !15) | 1 | if rising { 0 } else { 2 }));
        ACTIVE = true;
    });
}
/// Exactly one actual comparator read; the returned value is never substituted.
/// Instrumentation delays that read and extends its IRQ; not timing-neutral.
pub fn read_first<F: FnOnce() -> bool>(seq: u32, step: u32, read: F) -> bool {
    cortex_m::interrupt::free(|_| unsafe {
        if !ACTIVE || N >= 24 {
            return read();
        }
        let t = &*stm32::TIM2::ptr();
        let sr = t.sr().read().bits();
        let capture = t.ccr1().read().bits();
        let before = t.cnt().read().bits();
        let raw = read();
        let after = t.cnt().read().bits();
        let pwm = (*stm32::TIM1::ptr()).cnt().read().bits();
        // Overcapture belongs only to mirror; preserve every authority flag.
        t.sr().write(|w| w.bits(!(1 << 9)));
        core::ptr::addr_of_mut!(ROWS)
            .cast::<[u32; 10]>()
            .add(N)
            .write([
                seq,
                step,
                EPOCH,
                (sr >> 1) & 1,
                (sr >> 9) & 1,
                capture,
                before,
                after,
                raw as u32,
                pwm,
            ]);
        N += 1;
        raw
    })
}
pub fn dump<W: Write>(out: &mut W) {
    let (n, active) = cortex_m::interrupt::free(|_| unsafe { (N, ACTIVE) });
    if active {
        let _ = writeln!(out, "FILTERLATENCY refused_active=1");
        return;
    }
    let _ = writeln!(
        out,
        "FILTERLATENCY n={} latest_capture=1 authority=0 tick_half_us=1 prefix=1",
        n
    );
    for i in 0..n {
        let v = unsafe { core::ptr::addr_of!(ROWS).cast::<[u32; 10]>().add(i).read() };
        let _ = writeln!(
            out,
            "FILTERLAT seq={} step={} epoch={} valid={} over={} capture={} before={} after={} raw={} pwm={}",
            v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7], v[8], v[9]
        );
    }
}
