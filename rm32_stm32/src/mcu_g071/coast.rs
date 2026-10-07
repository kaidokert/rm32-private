//! binz bench: coast-down rotor speed (`C` key). After the bridge is off,
//! time the free-running rotor's BEMF crossings on one phase against the
//! star point: one phase crosses twice per electrical period, so a
//! half-period gives the TRUE rotor speed, independent of the commutation
//! loop's own interval estimate (which an early-accept bias can move).
//! Model: firmware50's `COASTTIMING` (`trans=`, `iv_us=`, `ehz_first=`).
//!
//! Runs in the main loop inside one critical section of at most `max_ms`.
//! `IsrAction::AllOff` alone is not enough: it does not stop the
//! commutation timer, and a commutation still armed from the last
//! zero-cross would re-drive the bridge (and re-select the comparator
//! input). So `measure` masks the commutation IRQ (TIM14) and clears its
//! pending bit, forces all phases off itself, and masks COMP's EXTI line.
//! TIM14 STAYS masked: the caller unmasks it with `release_com` only once
//! the control has stopped (`running == false` after the zero throttle).
//! Bench only. Timestamps: the free-running SysTick (24-bit down-counter
//! at the CPU clock, armed by `isr_handlers::tick_gap_init`).

use crate::comp_hal::CompOps;
use crate::mcu_g071::comparator::G071Comp;

/// Half-periods recorded (firmware50 records 32).
pub const N_IV: usize = 32;
/// A new comparator level must hold this many consecutive polls (~4 us at
/// ~15 cycles per poll) before it counts as a crossing: rejects the
/// chatter around each crossing without PWM present.
const STABLE: u32 = 16;

/// Result: crossings seen, cycles from the bridge-off request to the first
/// crossing, and up to `N_IV` half-periods in CPU cycles.
pub struct CoastTiming {
    pub trans: u32,
    pub first_cyc: u32,
    pub iv_cyc: [u32; N_IV],
}

fn now() -> u32 {
    // SAFETY: read-only access to the free-running SysTick counter.
    unsafe { (*cortex_m::peripheral::SYST::PTR).cvr.read() }
}

/// `t_off`: SysTick value taken when the bridge-off was requested.
/// `phase_inmsel`: the board's packed comparator input for the phase.
pub fn measure(t_off: u32, phase_inmsel: u32, max_ms: u32) -> CoastTiming {
    use crate::mcu::ChipConfig as _;
    let cpu_mhz = crate::mcu::Chip::CPU_FREQUENCY_MHZ;
    let max_cyc = max_ms * 1000 * cpu_mhz; // < 2^24 for max_ms <= 262 at 64 MHz
    let comp = G071Comp;
    let mut out = CoastTiming {
        trans: 0,
        first_cyc: 0,
        iv_cyc: [0; N_IV],
    };
    cortex_m::interrupt::free(|_| {
        use rm32::hal::PhaseOutput as _;
        cortex_m::peripheral::NVIC::mask(crate::pac::Interrupt::TIM14);
        cortex_m::peripheral::NVIC::unpend(crate::pac::Interrupt::TIM14);
        crate::phase::G0APhaseDriver::new(false).all_off();
        // SAFETY: EXTI line 18 (COMP2) mask + pending clear, as
        // `G071Exti::mask_and_clear`; the next commutation start re-arms it.
        unsafe {
            let exti = &*crate::pac::EXTI::ptr();
            exti.imr1().modify(|r, w| w.bits(r.bits() & !(1 << 18)));
            exti.rpr1().write(|w| w.bits(1 << 18));
            exti.fpr1().write(|w| w.bits(1 << 18));
        }
        comp.set_inmsel(phase_inmsel);
        let start = now();
        let mut level = comp.output();
        let mut run = 0u32;
        let mut t_cand = start;
        let mut t_last: Option<u32> = None;
        loop {
            let t = now();
            if (start.wrapping_sub(t) & 0x00FF_FFFF) > max_cyc {
                break;
            }
            if comp.output() != level {
                if run == 0 {
                    t_cand = t;
                }
                run += 1;
                if run >= STABLE {
                    level = !level;
                    run = 0;
                    match t_last {
                        None => out.first_cyc = t_off.wrapping_sub(t_cand) & 0x00FF_FFFF,
                        Some(tl) => {
                            let k = (out.trans - 1) as usize;
                            out.iv_cyc[k] = tl.wrapping_sub(t_cand) & 0x00FF_FFFF;
                        }
                    }
                    t_last = Some(t_cand);
                    out.trans += 1;
                    if out.trans as usize > N_IV {
                        break;
                    }
                }
            } else {
                run = 0;
            }
        }
    });
    out
}

/// Re-enable commutation after a coast measurement. Call only once the
/// control has stopped (`running == false`); any stale pending update is
/// dropped first.
pub fn release_com() {
    cortex_m::peripheral::NVIC::unpend(crate::pac::Interrupt::TIM14);
    // SAFETY: restores the unmask `mcu_g071::init` performed at boot.
    unsafe { cortex_m::peripheral::NVIC::unmask(crate::pac::Interrupt::TIM14) };
}
