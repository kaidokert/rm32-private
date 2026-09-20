//! Local test state only; no interrupt installation or motor authority.
use super::*;
use portable_atomic::{AtomicU32, Ordering::SeqCst};

pub fn marker<W: Write>(out: &mut W) {
    let _ = writeln!(
        out,
        "ATOMICBACKEND single_core={} explicit_cs_preserved=1",
        cfg!(feature = "bench-single-core-atomics") as u8
    );
}

#[inline(never)]
fn exercise() -> bool {
    let value = AtomicU32::new(0);
    value.store(7, SeqCst);
    let a = value.load(SeqCst) == 7;
    let b = value.swap(11, SeqCst) == 7;
    let c = value.compare_exchange(11, 19, SeqCst, SeqCst) == Ok(11);
    let d = value.compare_exchange(11, 29, SeqCst, SeqCst) == Err(19);
    let e = value.fetch_add(3, SeqCst) == 19;
    a && b && c && d && e && value.load(SeqCst) == 22
}

pub fn check<W: Write>(out: &mut W) {
    if powered_timer::owns() || !powered_timer::outputs_disabled() || get_idr(3, 1) {
        let _ = writeln!(out, "ATOMICCHECK refused=1 gate_authority=0");
        return;
    }
    let privileged = cortex_m::register::control::read().npriv().is_privileged();
    let before = cortex_m::register::primask::read().is_active();
    let mut passed = 0;
    for _ in 0..256 {
        let outer = exercise() && cortex_m::register::primask::read().is_active() == before;
        let nested = cortex_m::interrupt::free(|_| {
            let a = exercise() && !cortex_m::register::primask::read().is_active();
            let b = cortex_m::interrupt::free(|_| {
                exercise() && !cortex_m::register::primask::read().is_active()
            });
            a && b && !cortex_m::register::primask::read().is_active()
        });
        if outer && nested && cortex_m::register::primask::read().is_active() == before {
            passed += 1;
        }
    }
    marker(out);
    let _ = writeln!(
        out,
        "ATOMICCHECK passed={} total=256 privileged={} entry_unmasked={} restored={} disabled={} gate_authority=0",
        passed,
        privileged as u8,
        before as u8,
        (cortex_m::register::primask::read().is_active() == before) as u8,
        (!get_idr(3, 1) && powered_timer::outputs_disabled()) as u8
    );
}
