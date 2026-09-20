//! Experimental polled sidecar; no IRQ, DMA, comparator or gate authority.
//! First driven handoff only; the reference controller retains all authority.
use super::*;
static mut EPOCH: filter_epoch::Epoch = filter_epoch::Epoch::new();
// All callers below hold PRIMASK. No references escape a critical section.
unsafe fn epoch() -> &'static mut filter_epoch::Epoch {
    unsafe { &mut *core::ptr::addr_of_mut!(EPOCH) }
}
// samples, captures, overcaptures, minimum lag, maximum lag
static mut STATS: [u32; 5] = [0, 0, 0, u32::MAX, 0];
static mut SECTORS: [[u32; 2]; 6] = [[0; 2]; 6];
static mut MISSES: [[u32; 6]; 8] = [[0; 6]; 8];
static mut MISS_N: usize = 0;
static mut ARMED: [u32; 3] = [0; 3]; // step, raw rising, counter at arm
#[cfg(feature = "bench-filter-raw")]
static mut RAW_READY: bool = false;
#[cfg(feature = "bench-filter-raw")]
static mut RAW_MISSES: [[u32; 5]; 8] = [[0; 5]; 8]; // ready,flag,over,CCR,CNT

#[cfg(feature = "bench-filter-raw")]
pub fn raw_started() -> bool {
    cortex_m::interrupt::free(|_| unsafe {
        if !epoch().active() {
            return true;
        }
        RAW_READY = raw_capture::prepare();
        RAW_READY
    })
}
#[cfg(feature = "bench-filter-raw")]
pub fn raw_stopped() {
    cortex_m::interrupt::free(|_| unsafe {
        RAW_READY = false;
        raw_capture::stop();
    });
}

/// Call with reference timer stopped and all gate owners inactive.
/// ENABLE may remain awake during the validated driven ownership transfer.
pub fn prepare() -> bool {
    cortex_m::interrupt::free(|_| unsafe {
        stop();
        let t = &*stm32::TIM2::ptr();
        if !powered_timer::outputs_disabled()
            || driven_run::owns()
            || powered_timer::owns()
            || core_bench::active()
            || t.cr1().read().bits() & 1 != 0
            || t.dier().read().bits() != 0
        {
            return false;
        }
        let p = capture_filter::plan(
            2,
            if cfg!(feature = "bench-filter-short") {
                12
            } else {
                15
            },
        )
        .unwrap();
        // Preserve counter, period, prescaler, slave mode and unrelated bits.
        t.ccer().modify(|r, w| w.bits(r.bits() & !(15 << 4)));
        t.tisel()
            .modify(|r, w| w.bits((r.bits() & !(15 << 8)) | p.tisel));
        t.ccmr1_input()
            .modify(|r, w| w.bits((r.bits() & !0xff00) | p.ccmr1));
        t.cr1()
            .modify(|r, w| w.bits((r.bits() & !(3 << 8)) | p.ckd));
        t.sr().write(|w| w.bits(!(1 << 2 | 1 << 10)));
        core::ptr::addr_of_mut!(STATS).write([0, 0, 0, u32::MAX, 0]);
        core::ptr::addr_of_mut!(SECTORS).write([[0; 2]; 6]);
        MISS_N = 0;
        epoch().prepare();
        true
    })
}

/// Disable capture before changing comparator mux; never clear EXTI pending.
pub fn before_mux() -> Option<filter_epoch::Ticket> {
    cortex_m::interrupt::free(|_| unsafe {
        let ticket = epoch().before_mux();
        if ticket.is_some() {
            #[cfg(feature = "bench-filter-raw")]
            if RAW_READY {
                raw_capture::stop();
            }
            (*stm32::TIM2::ptr())
                .ccer()
                .modify(|r, w| w.bits(r.bits() & !(1 << 4)));
        }
        ticket
    })
}

/// Immediate arming deliberately includes mux/filter-settling artifacts.
/// Observations are NOT qualified BEMF events or a qzc metric.
pub fn after_mux(ticket: filter_epoch::Ticket, raw_rising: bool, step: u32) {
    cortex_m::interrupt::free(|_| unsafe {
        if !epoch().after_mux(ticket) {
            return;
        }
        #[cfg(feature = "bench-filter-raw")]
        if RAW_READY {
            raw_capture::arm(raw_rising);
        }
        let t = &*stm32::TIM2::ptr();
        core::ptr::addr_of_mut!(ARMED).write([step, raw_rising as u32, t.cnt().read().bits()]);
        t.sr().write(|w| w.bits(!(1 << 2 | 1 << 10)));
        t.ccer().modify(|r, w| {
            w.bits((r.bits() & !(15 << 4)) | (1 << 4) | if raw_rising { 0 } else { 1 << 5 })
        });
    });
}

/// Sample BEFORE the reference resets CNT. CCR holds latest, not first edge.
/// No timestamp is allowed to escape its counter epoch; disable until next mux.
pub fn before_counter_reset(record: bool) {
    cortex_m::interrupt::free(|_| unsafe {
        if !epoch().before_reset() {
            return;
        }
        let t = &*stm32::TIM2::ptr();
        t.ccer().modify(|r, w| w.bits(r.bits() & !(1 << 4)));
        if !record {
            t.sr().write(|w| w.bits(!(1 << 2 | 1 << 10)));
            return;
        }
        let sr = t.sr().read().bits();
        #[cfg(feature = "bench-filter-raw")]
        let raw = if RAW_READY {
            raw_capture::snapshot()
        } else {
            [0; 5]
        };
        let ccr = t.ccr2().read().bits();
        let now = t.cnt().read().bits();
        let mut s = core::ptr::addr_of!(STATS).read();
        s[0] += 1;
        let arm = core::ptr::addr_of!(ARMED).read();
        if (1..=6).contains(&arm[0]) {
            let row = core::ptr::addr_of_mut!(SECTORS)
                .cast::<[u32; 2]>()
                .add(arm[0] as usize - 1);
            let mut v = row.read();
            v[0] += 1;
            v[1] += ((sr >> 2) & 1);
            row.write(v);
        }
        if sr & (1 << 10) != 0 {
            s[2] += 1;
        }
        if sr & (1 << 2) != 0 {
            s[1] += 1;
            let lag = now.wrapping_sub(ccr) & 65535;
            s[3] = s[3].min(lag);
            s[4] = s[4].max(lag);
        } else if MISS_N < 8 {
            #[cfg(feature = "bench-filter-raw")]
            core::ptr::addr_of_mut!(RAW_MISSES)
                .cast::<[u32; 5]>()
                .add(MISS_N)
                .write(raw);
            // Bounded prefix only. This read is after the acceptance decision,
            // not the level sampled by the controller's persistence loop.
            core::ptr::addr_of_mut!(MISSES)
                .cast::<[u32; 6]>()
                .add(MISS_N)
                .write([
                    arm[0],
                    arm[1],
                    core::ptr::read_volatile(COMP2_CSR) >> 30 & 1,
                    now,
                    now.wrapping_sub(arm[2]) & 65535,
                    (*stm32::TIM1::ptr()).cnt().read().bits(),
                ]);
            MISS_N += 1;
        }
        core::ptr::addr_of_mut!(STATS).write(s);
        t.sr().write(|w| w.bits(!(1 << 2 | 1 << 10)));
    });
}

pub fn stop() {
    cortex_m::interrupt::free(|_| unsafe {
        let active = epoch().active();
        epoch().stop();
        #[cfg(feature = "bench-filter-raw")]
        if RAW_READY {
            RAW_READY = false;
            raw_capture::stop();
        }
        if active {
            let t = &*stm32::TIM2::ptr();
            t.ccer().modify(|r, w| w.bits(r.bits() & !(1 << 4)));
            t.sr().write(|w| w.bits(!(1 << 2 | 1 << 10)));
        }
    });
}

pub fn dump<W: Write>(out: &mut W) {
    if core_bench::active() || powered_timer::owns() {
        return;
    }
    let _ = writeln!(
        out,
        "FILTERCONFIG code={} ckd=2 observer_only=1",
        if cfg!(feature = "bench-filter-short") {
            12
        } else {
            15
        }
    );
    let (s, active) = cortex_m::interrupt::free(|_| unsafe {
        (core::ptr::addr_of!(STATS).read(), epoch().active())
    });
    let _ = writeln!(
        out,
        "FILTEROBS samples={} captures={} overcapture={} lag_min_ticks={} lag_max_ticks={} active={} authority=0 latest_capture=1 settling_excluded=0",
        s[0], s[1], s[2], s[3], s[4], active as u8
    );
    let (sectors, misses, n) = cortex_m::interrupt::free(|_| unsafe {
        (
            core::ptr::addr_of!(SECTORS).read(),
            core::ptr::addr_of!(MISSES).read(),
            MISS_N,
        )
    });
    for (i, r) in sectors.iter().enumerate() {
        let _ = writeln!(
            out,
            "FILTERSECTOR step={} samples={} captures={}",
            i + 1,
            r[0],
            r[1]
        );
    }
    for (i, r) in misses[..n].iter().enumerate() {
        let _ = writeln!(
            out,
            "FILTERMISS index={} step={} rising={} raw_after={} counter={} arm_age={} pwm={}",
            i, r[0], r[1], r[2], r[3], r[4], r[5]
        );
    }
    #[cfg(feature = "bench-filter-raw")]
    {
        let raw = cortex_m::interrupt::free(|_| unsafe { core::ptr::addr_of!(RAW_MISSES).read() });
        for (i, r) in raw[..n].iter().enumerate() {
            let _ = writeln!(
                out,
                "FILTERRAW index={} ready={} captured={} over={} ccr={} counter={} period_us=201 age_modulo_only=1",
                i, r[0], r[1], r[2], r[3], r[4]
            );
        }
    }
}
