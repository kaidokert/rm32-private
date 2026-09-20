//! Optional characterization ONLY. No changes to gate or guard decisions.
use super::*;
static mut METER: irq_accounting::Meter = irq_accounting::Meter::new();
#[cfg(feature = "bench-scheduling-tail")]
static mut TAIL: scheduling_tail::Tail = scheduling_tail::Tail::new();
pub fn reset() {
    cortex_m::interrupt::free(|_| unsafe {
        core::ptr::addr_of_mut!(METER).write(irq_accounting::Meter::new());
        #[cfg(feature = "bench-scheduling-tail")]
        core::ptr::addr_of_mut!(TAIL).write(scheduling_tail::Tail::new());
    });
}
// Vector identity is a compile-time property, not runtime scope state. This
// removes dynamic context indexing/bounds branches from each vector's exit.
pub struct Scope<const ID: u8> {
    entered: bool,
}
#[inline(always)]
pub fn enter<const ID: u8>() -> Scope<ID> {
    enter_inner::<ID, false>()
}
#[inline(always)]
fn enter_inner<const ID: u8, const CHECK_FORCE: bool>() -> Scope<ID> {
    // Diagnostic-only upper-regime probe: keep the meter entirely out of
    // startup/seed/handoff, where even its timestamp/critical-section cost
    // can change whether the replacement motor engages. Start at the first
    // powered interrupt after an ACKed >=35% live request.
    #[cfg(feature = "bench-cpu-running-only")]
    if !CHECK_FORCE && core_bench::live_duty_current().unwrap_or(0) < 350 {
        return Scope { entered: false };
    }
    #[cfg(not(feature = "bench-cpu-running-only"))]
    let _ = CHECK_FORCE;
    let entered = cortex_m::interrupt::free(|_| unsafe {
        let m = &mut *core::ptr::addr_of_mut!(METER);
        #[cfg(feature = "bench-scheduling-tail")]
        {
            let tail = &mut *core::ptr::addr_of_mut!(TAIL);
            if ID == 1 && !m.started && powered_timer::owns() {
                let now = t17();
                m.begin(now);
                tail.begin(now);
            }
            if !m.active {
                return false;
            }
            // One current sample for both instruments, including nesting.
            let now = t17();
            let entered = m.enter(ID, now);
            if entered {
                tail.enter(ID, now);
            }
            return entered;
        }
        #[cfg(not(feature = "bench-scheduling-tail"))]
        {
            // Start at the first powered guard interrupt, not in the time-critical
            // seed arm. Acquisition and up to one guard period are excluded.
            if ID == 1 && !m.started && powered_timer::owns() {
                m.begin(t17());
            }
            #[cfg(feature = "bench-cpu-union")]
            {
                m.enter_clock(ID, t17)
            }
            #[cfg(not(feature = "bench-cpu-union"))]
            {
                m.enter(ID, t17())
            }
        }
    });
    Scope { entered }
}
impl<const ID: u8> Drop for Scope<ID> {
    #[inline(always)]
    fn drop(&mut self) {
        if self.entered {
            cortex_m::interrupt::free(|_| unsafe {
                let m = &mut *core::ptr::addr_of_mut!(METER);
                #[cfg(feature = "bench-scheduling-tail")]
                {
                    if m.active {
                        let now = t17();
                        m.leave(ID, now);
                        (&mut *core::ptr::addr_of_mut!(TAIL)).leave(ID, now);
                    }
                }
                #[cfg(not(feature = "bench-scheduling-tail"))]
                {
                    #[cfg(feature = "bench-cpu-union")]
                    m.leave_clock(ID, t17);
                    #[cfg(not(feature = "bench-cpu-union"))]
                    m.leave(ID, t17());
                }
            });
        }
    }
}
pub fn finish() {
    cortex_m::interrupt::free(|_| unsafe {
        let now = t17();
        (&mut *core::ptr::addr_of_mut!(METER)).finish(now);
        #[cfg(feature = "bench-scheduling-tail")]
        (&mut *core::ptr::addr_of_mut!(TAIL)).finish(now);
    });
}
pub fn dump<W: Write>(out: &mut W) {
    // Called only by fixture capture after powered owner has stopped.
    if powered_timer::owns() {
        return;
    }
    // Copy under serialization; never retain a shared reference while IRQ
    // wrappers could obtain a mutable one during UART formatting.
    let m = cortex_m::interrupt::free(|_| unsafe { core::ptr::addr_of!(METER).read() });
    let _ = writeln!(
        out,
        "{} elapsed_us={} started={} active={} fault={} max_gap_us={} max_depth={} stopped_depth={} contexts={} first_guard_origin=1 exception_overhead_separate=0 probe_cost_measured=0 foreground_is_idle=0",
        if cfg!(feature = "bench-cpu-union") {
            "CPUUNION"
        } else {
            "CPUMETER"
        },
        m.elapsed,
        m.started as u8,
        m.active as u8,
        m.fault,
        m.max_gap,
        m.max_depth,
        m.stopped_depth,
        irq_accounting::CONTEXTS
    );
    for id in 0..irq_accounting::CONTEXTS {
        let _ = snapshot::record(
            out,
            if cfg!(feature = "bench-cpu-union") {
                "CU85"
            } else {
                "CPU85"
            },
            &[
                id as u16,
                m.calls[id] as u16,
                (m.calls[id] >> 16) as u16,
                m.time[id] as u16,
                (m.time[id] >> 16) as u16,
            ],
        );
    }
    #[cfg(feature = "bench-cpu-roots")]
    {
        let _ = writeln!(
            out,
            "CPUROOT contexts=6 includes_nested=1 exclusive=0 extra_clock_reads=0"
        );
        for id in 1..7 {
            let _ = snapshot::record(
                out,
                "CR85",
                &[
                    id as u16,
                    m.root_time[id] as u16,
                    (m.root_time[id] >> 16) as u16,
                ],
            );
        }
    }
    #[cfg(feature = "bench-scheduling-tail")]
    dump_tail(out);
    #[cfg(feature = "bench-comp-overlap")]
    {
        let overlap = m.stopped_comp_overlap();
        let _ = writeln!(
            out,
            "COMPOVERLAP stop_in_comp={} mask={} guard_bit=2 dma_bit=64 extra_clock_reads=0 stop_context_only=1 preentry_delay_measured=0",
            overlap.is_some() as u8,
            overlap.unwrap_or(0)
        );
    }
}

#[cfg(feature = "bench-scheduling-tail")]
#[inline(never)]
fn dump_tail<W: Write>(out: &mut W) {
    // Post-stop only; copy scalars/one row, never a 544-byte stack temporary.
    let (started, active, fault, count, elapsed, gap) = cortex_m::interrupt::free(|_| unsafe {
        let t = &*core::ptr::addr_of!(TAIL);
        (
            t.started,
            t.active,
            t.fault,
            t.count,
            t.elapsed_us,
            t.max_gap_us,
        )
    });
    if active {
        return;
    }
    let n = count.min(scheduling_tail::CAPACITY as u32);
    let _ = writeln!(
        out,
        "SCHEDTAIL count={} retained={} omitted={} elapsed_us={} max_gap_us={} started={} active=0 fault={} capacity=64 row_words=4 first_guard_origin=1 shared_cpu_clock=1 physical_edges=0 blackout_watchdog=0",
        count,
        n,
        count.saturating_sub(64),
        elapsed,
        gap,
        started as u8,
        fault
    );
    for i in 0..n {
        let row =
            cortex_m::interrupt::free(|_| unsafe { (&*core::ptr::addr_of!(TAIL)).row(i as usize) });
        if let Some(r) = row {
            let _ = snapshot::record(
                out,
                "ST85",
                &[
                    r.elapsed_us as u16,
                    (r.elapsed_us >> 16) as u16,
                    r.packed as u16,
                    (r.packed >> 16) as u16,
                ],
            );
        }
    }
}

/// Disabled-board software probe cost; not NVIC exception cost or a motor run.
#[inline(never)]
pub fn check<W: Write>(out: &mut W) {
    if powered_timer::owns() || !powered_timer::outputs_disabled() || get_idr(3, 1) {
        let _ = writeln!(out, "CPUCHECK refused=1 gate_authority=0");
        return;
    }
    let mut results = [[0u32; 3]; 3];
    for mode in 0..3 {
        reset();
        if mode != 0 {
            cortex_m::interrupt::free(|_| unsafe {
                let now = t17();
                (&mut *core::ptr::addr_of_mut!(METER)).begin(now);
                #[cfg(feature = "bench-scheduling-tail")]
                (&mut *core::ptr::addr_of_mut!(TAIL)).begin(now);
            });
        }
        let mut sum = 0;
        let mut maximum = 0;
        for _ in 0..256 {
            let before = t17();
            {
                // Exercise the active ISR meter while the bridge is known
                // disabled, even when the running-only duty gate is shut.
                let _outer = enter_inner::<2, true>();
                if mode == 2 {
                    let _inner = enter_inner::<1, true>();
                }
            }
            let us = t17().wrapping_sub(before) as u32;
            sum += us;
            maximum = maximum.max(us);
        }
        finish();
        let fault = cortex_m::interrupt::free(|_| unsafe { (*core::ptr::addr_of!(METER)).fault });
        #[cfg(feature = "bench-scheduling-tail")]
        let fault = fault.max(cortex_m::interrupt::free(|_| unsafe {
            (*core::ptr::addr_of!(TAIL)).fault
        }));
        results[mode] = [sum, maximum, fault as u32];
    }
    reset(); // Synthetic accounting never survives into a motor capture.
    let _ = writeln!(
        out,
        "CPUCHECKTYPE union={}",
        cfg!(feature = "bench-cpu-union") as u8
    );
    #[cfg(feature = "bench-comp-overlap")]
    let _ = writeln!(
        out,
        "CPUCHECKOVERLAP enabled=1 nested_exercised=1 extra_clock_reads=0"
    );
    #[cfg(feature = "bench-scheduling-tail")]
    let _ = writeln!(
        out,
        "CPUCHECKTAIL enabled=1 nested_exercised=1 fault_checked=1"
    );
    for (mode, r) in results.iter().enumerate() {
        let _ = writeln!(
            out,
            "CPUCHECK mode={} n=256 sum_us={} max_us={} fault={} software_pairs_only=1 gate_authority=0 disabled={}",
            mode,
            r[0],
            r[1],
            r[2],
            (!get_idr(3, 1) && powered_timer::outputs_disabled()) as u8
        );
    }
}
