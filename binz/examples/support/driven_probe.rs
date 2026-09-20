//! Gates-disabled TIM3/TIM6 test. Optional real ADC wakes CSA,still NO PWM.
//! Must not be used as a powered runtime driver or BEMF qualification result.
use super::*;
use minz_core::am32_hal::Comparator;
use portable_atomic::{AtomicBool, Ordering::Relaxed};
static ACTIVE: AtomicBool = AtomicBool::new(false);
const PHASE_ORIGIN: u32 = 0x12345678;
const PHASE_RATE: u32 = 858993; // same Q0.32 integer rate as200eHz sine
struct State {
    guard: Option<driven_guard::Guard>,
    origin: u16,
    step: u8,
    commands: u32,
    ticks: u32,
    reason: u32,
    stop_us: u32,
    sector_max: u16,
    tick_max: u16,
    real: bool,
    scans: u32,
    max_age: u32,
    peak: u32,
    bus_min: u32,
    observer: Option<driven_observer::Observer>,
    reads: u32,
    last_read: u32,
    gap_max: u32,
    bracket_max: u32,
    steps: u32,
    phased: bool,
    deadline: u32,
    first_us: u32,
    late_max: u32,
}
static mut STATE: State = State {
    guard: None,
    origin: 0,
    step: 1,
    commands: 0,
    ticks: 0,
    reason: 0,
    stop_us: 0,
    sector_max: 0,
    tick_max: 0,
    real: false,
    scans: 0,
    max_age: 0,
    peak: 0,
    bus_min: u32::MAX,
    observer: None,
    reads: 0,
    last_read: 0,
    gap_max: 0,
    bracket_max: 0,
    steps: 0,
    phased: false,
    deadline: 0,
    first_us: 0,
    late_max: 0,
};
pub fn owns() -> bool {
    ACTIVE.load(Relaxed)
}
/// Shared gates_off must revoke this owner before physical outputs are cleared.
/// Do not call gates_off from here: that would recurse through this hook.
pub fn cancel() {
    cortex_m::interrupt::free(|_| unsafe {
        if !owns() {
            return;
        }
        let s = &mut *core::ptr::addr_of_mut!(STATE);
        let now = t17().wrapping_sub(s.origin) as u32;
        end(s, 9, now);
        set_pin(3, 1, false);
    });
}
fn disabled() -> bool {
    !get_idr(3, 1) && powered_timer::outputs_disabled()
}
fn safe(real: bool) -> bool {
    get_idr(3, 1) == real && powered_timer::outputs_disabled()
}
fn code(f: powered_guard::Fault) -> u32 {
    use powered_guard::Fault::*;
    match f {
        SegmentDeadline => 2,
        TickGap => 3,
        FeedbackStale => 4,
        Current => 5,
        Bus => 6,
        Driver => 7,
        HostAbort => 9,
        _ => 10,
    }
}
// All callers serialize with both IRQs; timer shutdown does not touch PWM.
unsafe fn end(s: &mut State, reason: u32, now: u32) {
    unsafe {
        if s.reason == 0 {
            s.reason = reason;
            s.stop_us = now;
        }
        if let Some(g) = s.guard.as_mut() {
            g.stop();
        }
        ACTIVE.store(false, Relaxed);
        (*stm32::TIM3::ptr()).dier().write(|w| w.bits(0));
        (*stm32::TIM3::ptr()).cr1().write(|w| w.bits(0));
        (*stm32::TIM6::ptr()).dier().write(|w| w.bits(0));
        (*stm32::TIM6::ptr()).cr1().write(|w| w.bits(0));
        cortex_m::peripheral::NVIC::mask(stm32::Interrupt::TIM3);
        cortex_m::peripheral::NVIC::mask(stm32::Interrupt::TIM6_DAC_LPTIM1);
    }
}
pub fn tick() {
    let began = t17();
    cortex_m::interrupt::free(|_| unsafe {
        (*stm32::TIM6::ptr()).sr().write(|w| w.bits(0));
        if !owns() {
            return;
        }
        let s = &mut *core::ptr::addr_of_mut!(STATE);
        let now = t17().wrapping_sub(s.origin) as u32;
        s.ticks += 1;
        if !safe(s.real) {
            end(s, 11, now);
            return;
        }
        if let Err(f) = s.guard.as_mut().unwrap().tick(now, get_idr(1, 14), false) {
            end(s, code(f), now);
            return;
        }
        if let Some(o) = s.observer.as_mut() {
            // Both timer IRQs serialized: mux/epoch cannot change across read.
            let before = t17().wrapping_sub(s.origin) as u32;
            let level = core::ptr::read_volatile(COMP2_CSR) & (1 << 30) == 0;
            let after = t17().wrapping_sub(s.origin) as u32;
            s.bracket_max = s.bracket_max.max(after.wrapping_sub(before));
            if s.reads != 0 {
                s.gap_max = s.gap_max.max(before.wrapping_sub(s.last_read));
            }
            s.last_read = before;
            s.reads += 1;
            s.steps |= 1 << (s.step - 1);
            let _ = o.sample(s.commands, s.commands, before, after, level);
        }
        s.tick_max = s.tick_max.max(t17().wrapping_sub(began));
    });
}
pub fn sector() {
    let began = t17();
    cortex_m::interrupt::free(|_| unsafe {
        (*stm32::TIM3::ptr()).sr().write(|w| w.bits(0));
        if !owns() {
            return;
        }
        let s = &mut *core::ptr::addr_of_mut!(STATE);
        let now = t17().wrapping_sub(s.origin) as u32;
        if !safe(s.real) {
            end(s, 11, now);
            return;
        }
        let next = s.step % 6 + 1;
        if s.phased {
            let late = now.wrapping_sub(s.deadline);
            if late > 50 {
                end(s, 15, now);
                return;
            }
            s.late_max = s.late_max.max(late);
        }
        match s.guard.as_mut().unwrap().authorize(now, next) {
            Ok(()) => {
                s.step = next;
                s.commands += 1; // Deliberately no physical gate write.
                if let Some(o) = s.observer.as_mut() {
                    comp_input::Input.set_step(next, next & 1 == 0);
                    comp_input::Input.change_input();
                    let changed = t17().wrapping_sub(s.origin) as u32;
                    if !o.command(s.commands, next, changed) {
                        end(s, 14, changed);
                    }
                }
            }
            Err(f) => end(s, code(f), now),
        }
        if owns() && s.phased {
            let theta = PHASE_ORIGIN.wrapping_add(PHASE_RATE.wrapping_mul(s.deadline));
            let Some(boundary) = phase_schedule::next(theta, PHASE_RATE) else {
                end(s, 16, now);
                return;
            };
            if boundary.step != s.step {
                end(s, 16, now);
                return;
            }
            // Timer already reset itself at update. Changing ARR without UG
            // retains elapsed ISR time; ISR execution never shifts the epoch.
            (*stm32::TIM3::ptr())
                .arr()
                .write(|w| w.bits(boundary.delay_us as u32 - 1));
            s.deadline += boundary.delay_us as u32;
        }
        s.sector_max = s.sector_max.max(t17().wrapping_sub(began));
    });
}
pub fn run<W: Write>(
    out: &mut W,
    real: bool,
    comp: bool,
    cancel_test: bool,
    phased: bool,
    vcal: u32,
) {
    if owns() || powered_timer::owns() || core_bench::active() || !disabled() {
        let _ = writeln!(out, "DRIVENCHECK refused=1 gate_authority=0");
        return;
    }
    gates_off();
    set_pin(3, 1, false);
    // TIM3 may be owned by ADC-trigger diagnostics. Refuse rather than steal.
    if unsafe { (*stm32::TIM3::ptr()).cr1().read().bits() & 1 != 0 } {
        let _ = writeln!(out, "DRIVENCHECK refused=2 gate_authority=0");
        return;
    }
    if real && !powered_timer::wake(&mut || false) {
        let _ = writeln!(out, "DRIVENCHECK refused=3 gate_authority=0");
        return;
    }
    let initial_at = t17();
    let baseline = if real {
        powered_timer::sample_feedback(vcal)
    } else {
        Some(powered_guard::Feedback {
            phase: [2048; 3],
            bus_mv: 11700,
            vref: 1500,
        })
    };
    let boundary = phase_schedule::next(PHASE_ORIGIN, PHASE_RATE).unwrap();
    let first_step = if phased { boundary.step } else { 1 };
    let first_us = if phased {
        boundary.delay_us as u32
    } else {
        833
    };
    let guard =
        baseline.and_then(|sample| driven_guard::Guard::new(0, 20_000, first_step, sample).ok());
    if guard.is_none() {
        gates_off();
        set_pin(3, 1, false);
        let _ = writeln!(out, "DRIVENCHECK refused=4 gate_authority=0");
        return;
    }
    let sample = baseline.unwrap();
    let saved_comp = unsafe { core::ptr::read_volatile(COMP2_CSR) };
    if comp {
        comp_input::stop();
        comp_input::Input.set_step(first_step, first_step & 1 == 0);
        comp_input::Input.change_input();
    }
    cortex_m::interrupt::free(|_| unsafe {
        (*stm32::RCC::ptr())
            .apbenr1()
            .modify(|r, w| w.bits(r.bits() | 2 | (1 << 4)));
        let s = &mut *core::ptr::addr_of_mut!(STATE);
        *s = State {
            guard,
            origin: 0,
            step: first_step,
            commands: 0,
            ticks: 0,
            reason: 0,
            stop_us: 0,
            sector_max: 0,
            tick_max: 0,
            real,
            scans: 0,
            max_age: 0,
            peak: 0,
            bus_min: sample.bus_mv,
            observer: if comp {
                Some(driven_observer::Observer::new())
            } else {
                None
            },
            reads: 0,
            last_read: 0,
            gap_max: 0,
            bracket_max: 0,
            steps: 0,
            phased,
            deadline: first_us,
            first_us,
            late_max: 0,
        };
        // URS prevents software UG from generating a startup interrupt.
        // Keep it set through start; only counter overflow requests updates.
        let t = &*stm32::TIM3::ptr();
        t.cr1().write(|w| w.bits(4));
        t.dier().write(|w| w.bits(0));
        t.cr2().write(|w| w.bits(0));
        t.ccer().write(|w| w.bits(0));
        t.psc().write(|w| w.bits(63));
        t.arr().write(|w| w.bits(first_us - 1));
        t.egr().write(|w| w.bits(1));
        t.sr().write(|w| w.bits(0));
        let t6 = &*stm32::TIM6::ptr();
        t6.cr1().write(|w| w.bits(4));
        t6.dier().write(|w| w.bits(0));
        t6.psc().write(|w| w.bits(63));
        t6.arr().write(|w| w.bits(if comp { 49 } else { 99 }));
        t6.egr().write(|w| w.bits(1));
        t6.sr().write(|w| w.bits(0));
        let mut p = cortex_m::Peripherals::steal();
        p.NVIC.set_priority(stm32::Interrupt::TIM6_DAC_LPTIM1, 0);
        p.NVIC.set_priority(stm32::Interrupt::TIM3, 0x80);
        cortex_m::peripheral::NVIC::unpend(stm32::Interrupt::TIM3);
        cortex_m::peripheral::NVIC::unpend(stm32::Interrupt::TIM6_DAC_LPTIM1);
        s.origin = t17();
        if let Some(o) = s.observer.as_mut() {
            o.command(0, first_step, 0);
        }
        let age = if real {
            s.origin.wrapping_sub(initial_at) as u32
        } else {
            0
        };
        if s.guard.as_mut().unwrap().age_initial_feedback(age).is_err() {
            end(s, 4, 0);
            return;
        }
        ACTIVE.store(true, Relaxed);
        t6.dier().write(|w| w.bits(1));
        t6.cr1().write(|w| w.bits(5));
        t.dier().write(|w| w.bits(1));
        t.cr1().write(|w| w.bits(5));
        cortex_m::peripheral::NVIC::unmask(stm32::Interrupt::TIM6_DAC_LPTIM1);
        cortex_m::peripheral::NVIC::unmask(stm32::Interrupt::TIM3);
    });
    while owns() {
        if cancel_test
            && unsafe { t17().wrapping_sub((*core::ptr::addr_of!(STATE)).origin) >= 5000 }
        {
            // Exercise the actual shared safing entry, not a local fake stop.
            gates_off();
            set_pin(3, 1, false);
            break;
        }
        let acquired = t17();
        // ADC/voltage conversion stays outside critical sections; guard IRQ
        // remains live. Acquisition age uses scan START conservatively.
        let reading = if real {
            powered_timer::sample_feedback(vcal)
        } else {
            Some(sample)
        };
        cortex_m::interrupt::free(|_| unsafe {
            if !owns() {
                return;
            }
            let s = &mut *core::ptr::addr_of_mut!(STATE);
            let now = t17().wrapping_sub(s.origin) as u32;
            if !safe(s.real) {
                end(s, 11, now);
                return;
            }
            if now > 22_000 {
                end(s, 12, now);
                return;
            }
            let Some(reading) = reading else {
                end(s, 13, now);
                return;
            };
            let age = if real {
                t17().wrapping_sub(acquired) as u32
            } else {
                0
            };
            if let Err(f) = s.guard.as_mut().unwrap().feedback(now, age, reading) {
                end(s, code(f), now);
                return;
            }
            if real {
                s.scans += 1;
                s.max_age = s.max_age.max(age);
                s.bus_min = s.bus_min.min(reading.bus_mv);
                for raw in reading.phase {
                    s.peak = s.peak.max((raw as i32 - 2048).unsigned_abs());
                }
            }
        });
    }
    cortex_m::peripheral::NVIC::unpend(stm32::Interrupt::TIM3);
    cortex_m::peripheral::NVIC::unpend(stm32::Interrupt::TIM6_DAC_LPTIM1);
    gates_off();
    set_pin(3, 1, false);
    if comp {
        comp_input::stop();
        unsafe {
            core::ptr::write_volatile(COMP2_CSR, saved_comp);
        }
    }
    if cancel_test {
        let commands = unsafe { (*core::ptr::addr_of!(STATE)).commands };
        // Simulate delivery of already-pending callbacks AFTER shared stop.
        sector();
        tick();
        let unchanged = unsafe { (*core::ptr::addr_of!(STATE)).commands == commands };
        let timers_off = unsafe {
            ((*stm32::TIM3::ptr()).cr1().read().bits() | (*stm32::TIM6::ptr()).cr1().read().bits())
                & 1
                == 0
        };
        let _ = writeln!(
            out,
            "DRIVENCANCEL shared_gates_off=1 late_callbacks=2 commands_unchanged={} timers_off={} owner_off={} disabled={} gate_authority=0",
            unchanged as u8,
            timers_off as u8,
            (!owns()) as u8,
            disabled() as u8
        );
    }
    let s = unsafe { &mut *core::ptr::addr_of_mut!(STATE) };
    let refused = s
        .guard
        .as_mut()
        .unwrap()
        .authorize(s.stop_us + 1, s.step % 6 + 1)
        .is_err();
    let _ = writeln!(
        out,
        "DRIVENCHECK reason={} stop_us={} ticks={} commands={} tick_max_us={} sector_max_us={} poststop_refused={} disabled={} synthetic_feedback={} gate_authority=0 sector_period_us={}",
        s.reason,
        s.stop_us,
        s.ticks,
        s.commands,
        s.tick_max,
        s.sector_max,
        refused as u8,
        disabled() as u8,
        (!real) as u8,
        if phased { 0 } else { 833 }
    );
    if phased {
        let _ = writeln!(
            out,
            "DRIVENPHASE theta={} rate={} first_us={} next_deadline_us={} late_max_us={} synthetic_phase=1 gate_authority=0",
            PHASE_ORIGIN, PHASE_RATE, s.first_us, s.deadline, s.late_max
        );
    }
    if real {
        let _ = writeln!(
            out,
            "DRIVENADC scans={} max_age_us={} peak_abs_raw={} bus_min_mv={} scan_start_aged=1 gates_disabled=1",
            s.scans, s.max_age, s.peak, s.bus_min
        );
    }
    if let Some(o) = s.observer.as_ref() {
        let _ = writeln!(
            out,
            "DRIVENCOMP reads={} gap_max_us={} bracket_max_us={} steps_mask={} candidates={} missed={} reject_none={} reject_epoch={} reject_blank={} reject_bracket={} reject_gap={} period_us=50 synthetic_sectors=1 gate_authority=0",
            s.reads,
            s.gap_max,
            s.bracket_max,
            s.steps,
            o.candidates,
            o.missed,
            o.rejected[0],
            o.rejected[1],
            o.rejected[2],
            o.rejected[3],
            o.rejected[4]
        );
    }
}
