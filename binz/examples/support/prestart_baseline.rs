//! Raw pre-drive baseline in the existing wake epoch. Not proof of zero current.
//! Never applied to motor guards; no output authority or recovery baseline reuse.
use super::*;
static mut TOKEN: Option<calibration_epoch::Token> = None;
static mut STATS: [zero_stats::Stats; 5] = [zero_stats::Stats::new(); 5];
static mut STATUS: u32 = 0;
static mut ELAPSED: u32 = 0;
static mut ENTRY: u32 = 0;
static mut RECOVERY: u32 = 0;
#[path = "average_current.rs"]
mod average_policy;

/// Offset evidence for current-limit initialization, not motor authority.
/// Caller still establishes rotor stationarity and calibrated current gain.
/// Never returns a baseline from an earlier ENABLE wake or partial capture.
pub fn current_zeros() -> Option<(i32, [u16; 3])> {
    cortex_m::interrupt::free(|_| unsafe {
        if STATUS != 2
            || !(&*core::ptr::addr_of!(TOKEN))
                .as_ref()
                .is_some_and(calibration_live::matches)
        {
            return None;
        }
        let stats = &*core::ptr::addr_of!(STATS);
        if stats[..3]
            .iter()
            .any(|s| s.n != 128 || s.min == 0 || s.max >= 4095)
        {
            return None;
        }
        let sums = [stats[0].sum, stats[1].sum, stats[2].sum];
        let average = average_policy::zero_from_128(sums)?;
        // Nearest raw code per channel. These offsets are used only by the
        // ratings-derived pulse backstop in this same ENABLE epoch.
        let phase = [
            ((sums[0] + 64) >> 7) as u16,
            ((sums[1] + 64) >> 7) as u16,
            ((sums[2] + 64) >> 7) as u16,
        ];
        Some((average, phase))
    })
}
pub fn average_zero() -> Option<i32> {
    current_zeros().map(|v| v.0)
}

/// Same-wake, bridge-off voltage reference for a relative sag guard. The ADC
/// scans are coherent, and the bus/VREF ratio avoids assuming fixed VDDA.
#[cfg(feature = "bench-fast-bus-sag")]
pub fn bus_reference() -> Option<(u16, u16)> {
    cortex_m::interrupt::free(|_| unsafe {
        if STATUS != 2
            || !(&*core::ptr::addr_of!(TOKEN))
                .as_ref()
                .is_some_and(calibration_live::matches)
        {
            return None;
        }
        let stats = &*core::ptr::addr_of!(STATS);
        if stats[3..5]
            .iter()
            .any(|s| s.n != 128 || s.min == 0 || s.max >= 4095)
        {
            return None;
        }
        let bus = ((stats[3].sum + 64) >> 7) as u16;
        let vref = ((stats[4].sum + 64) >> 7) as u16;
        // Refuse an already-collapsed baseline; never normalize it as healthy.
        let vcal = core::ptr::read_volatile(VREFINT_CAL_ADDR as *const u16) as u32;
        if (bus as u32 * vcal) < 963 * vref as u32 {
            return None;
        }
        Some((bus, vref))
    })
}

/// Called after normal startup wake, before any phase drive. Caller disables
/// on false. At most128 scans/50ms; no printing or PWM commands in this loop.
pub fn acquire(abort: &mut dyn FnMut() -> bool) -> bool {
    unsafe {
        TOKEN = None;
        STATS = [zero_stats::Stats::new(); 5];
        STATUS = 1;
        ELAPSED = 0;
        ENTRY = 0;
        RECOVERY = 0;
    }
    let Some(token) = calibration_live::begin() else {
        return false;
    };
    let started = t17();
    #[cfg(feature = "bench-prestart-dma")]
    let good = adc_stream::prestart_collect(
        &mut || {
            !abort() && t17().wrapping_sub(started) < 50_000 && calibration_live::matches(&token)
        },
        unsafe { &mut *core::ptr::addr_of_mut!(STATS) },
    );
    #[cfg(not(feature = "bench-prestart-dma"))]
    let mut good = true;
    #[cfg(not(feature = "bench-prestart-dma"))]
    'scan: for _ in 0..128 {
        for (i, ch) in [4, 1, 0, 6, 13].into_iter().enumerate() {
            if abort()
                || t17().wrapping_sub(started) >= 50_000
                || !powered_timer::outputs_disabled()
                || !calibration_live::matches(&token)
            {
                good = false;
                break 'scan;
            }
            let Some(raw) = powered_timer::read_channel(ch) else {
                good = false;
                break 'scan;
            };
            if !unsafe {
                (&mut *core::ptr::addr_of_mut!(STATS))
                    .get_mut(i)
                    .unwrap()
                    .push(raw)
            } {
                good = false;
                break 'scan;
            }
        }
    }
    let good = good
        && t17().wrapping_sub(started) < 50_000
        && powered_timer::outputs_disabled()
        && calibration_live::matches(&token);
    unsafe {
        ELAPSED = t17().wrapping_sub(started) as u32;
        STATUS = if good { 2 } else { 1 };
        if good {
            TOKEN = Some(token);
        }
    }
    good
}
/// Evidence at the pre-transfer staging boundary, NOT a fresh-seed timestamp.
/// It cannot authorize handoff, apply offsets, or keep a driver awake.
pub fn entry_check() {
    unsafe {
        ENTRY = if (&*core::ptr::addr_of!(TOKEN))
            .as_ref()
            .is_some_and(calibration_live::matches)
        {
            1
        } else {
            2
        };
    }
}
/// Called after recovery wake but BEFORE fresh-edge acquisition. Keep initial
/// evidence separate; never apply or reacquire a zero from the moving rotor.
pub fn recovery_check() {
    unsafe {
        RECOVERY = if (&*core::ptr::addr_of!(TOKEN))
            .as_ref()
            .is_some_and(calibration_live::matches)
        {
            1
        } else {
            2
        };
    }
}
pub fn dump<W: Write>(out: &mut W) {
    #[cfg(all(feature = "bench-adc-tim15", not(feature = "bench-startup-adc")))]
    let _ = writeln!(
        out,
        "ADCTRIGGER timer=15 extsel=4 continuous_startup=0 backend_only=1"
    );
    #[cfg(feature = "bench-startup-adc")]
    let _ = writeln!(
        out,
        "ADCTRIGGER timer=15 extsel=4 continuous_startup=1 backend_only=0 startup_vsenc_valid=0 startup_neutral_valid=0"
    );
    #[cfg(feature = "bench-prestart-dma")]
    let _ = writeln!(
        out,
        "BASEACQ dma=1 trigger_us={} first_us={} channels=0,1,4,6,13 irq_consumed=0 offsets_applied=0",
        adc_stream::PERIOD_US,
        adc_stream::FIRST_TRIGGER_US
    );
    let (status, elapsed, entry) = unsafe { (STATUS, ELAPSED, ENTRY) };
    let _ = writeln!(
        out,
        "PREBASE status={} n_target=128 elapsed_us={} entry_same_epoch={} stationary_verified=0 offsets_applied=0 guard_settings_unchanged=1",
        status, elapsed, entry
    );
    let recovery = unsafe { RECOVERY };
    let _ = writeln!(
        out,
        "BASEEPOCH recovery_checked={} recovery_matches={} initial_records_only=1",
        (recovery != 0) as u8,
        (recovery == 1) as u8
    );
    for i in 0..5 {
        let words = unsafe { (&*core::ptr::addr_of!(STATS))[i].words([4, 1, 0, 6, 13][i]) };
        let _ = snapshot::record(out, "BZ85", &words);
    }
}
/// Idle-only physical scan tests. ENABLE may wake CSA; no gate writes.
pub fn check<W: Write>(out: &mut W) {
    if get_idr(3, 1) || !powered_timer::outputs_disabled() {
        let _ = writeln!(out, "!basecheck disabled_only");
        return;
    }
    for case in 0..4 {
        if case != 0 {
            set_pin(3, 1, true);
            cortex_m::asm::delay(70_400);
        }
        let mut calls = 0;
        let result = acquire(&mut || {
            calls += 1;
            if case == 2 && calls == 11 {
                set_pin(3, 1, false);
            }
            case == 1 && calls == 11
        });
        let passed = if case == 3 {
            entry_check();
            let first = unsafe { ENTRY == 1 };
            set_pin(3, 1, false);
            set_pin(3, 1, true);
            cortex_m::asm::delay(70_400);
            entry_check();
            result && first && unsafe { ENTRY == 2 }
        } else {
            !result
        };
        set_pin(3, 1, false);
        let _ = writeln!(
            out,
            "BASECHECK case={} passed={} disabled={} gates_commanded=0",
            case,
            passed as u8,
            (!get_idr(3, 1) && powered_timer::outputs_disabled()) as u8
        );
        dump(out);
    }
}
