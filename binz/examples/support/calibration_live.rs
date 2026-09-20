//! Optional singleton adapter. Epoch identity is provenance, never motor authority.
use super::*;
static mut EPOCH: calibration_epoch::Epoch = calibration_epoch::Epoch::new();

pub fn write_enable(high: bool) {
    cortex_m::interrupt::free(|_| unsafe {
        // First operation after masking IRQs is the physical write. No token
        // can be observed between that write and its provenance update.
        (*stm32::GPIOD::ptr())
            .bsrr()
            .write(|w| w.bits(if high { 2 } else { 2 << 16 }));
        (&mut *core::ptr::addr_of_mut!(EPOCH)).commanded(high);
        #[cfg(feature = "bench-average-current")]
        if !high {
            average_current_live::revoke();
        }
        #[cfg(feature = "bench-adc-latest")]
        if !high {
            adc_stream::quiesce();
        }
    });
}
pub fn begin() -> Option<calibration_epoch::Token> {
    cortex_m::interrupt::free(|_| unsafe {
        (&*core::ptr::addr_of!(EPOCH)).begin(
            get_idr(3, 1),
            get_idr(1, 14),
            powered_timer::outputs_disabled(),
        )
    })
}
pub fn matches(token: &calibration_epoch::Token) -> bool {
    cortex_m::interrupt::free(|_| unsafe {
        (&mut *core::ptr::addr_of_mut!(EPOCH)).matches(token, get_idr(3, 1), get_idr(1, 14))
    })
}
/// No gates commanded: wake only CSA, test actual PD1 routing, then disable.
/// Never restore an old tracker: this diagnostic itself revokes prior tokens.
pub fn check<W: Write>(out: &mut W) {
    if get_idr(3, 1) || !powered_timer::outputs_disabled() {
        let _ = writeln!(out, "!epochcheck disabled_only");
        return;
    }
    let mut passed = 0;
    if begin().is_none() {
        passed += 1;
    }
    set_pin(3, 1, true);
    cortex_m::asm::delay(70_400);
    let token = begin();
    if token.as_ref().is_some_and(matches) {
        passed += 1;
    }
    let mut maximum = 0;
    let mut total = 0;
    for _ in 0..256 {
        let at = t17();
        set_pin(3, 1, true);
        let cost = t17().wrapping_sub(at) as u32;
        maximum = maximum.max(cost);
        total += cost;
    }
    if token.as_ref().is_some_and(matches) {
        passed += 1;
    }
    set_pin(3, 1, false);
    set_pin(3, 1, true);
    cortex_m::asm::delay(70_400);
    if token.as_ref().is_some_and(|t| !matches(t)) {
        passed += 1;
    }
    if begin().as_ref().is_some_and(matches) {
        passed += 1;
    }
    set_pin(3, 1, false);
    let disabled = !get_idr(3, 1) && powered_timer::outputs_disabled();
    let _ = writeln!(
        out,
        "EPOCHCHECK passed={} total=5 repeat_n=256 repeat_sum_us={} repeat_max_us={} disabled={} gates_commanded=0",
        passed, total, maximum, disabled as u8
    );
}
