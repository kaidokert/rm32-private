//! Experimental BEMF-only carrier path. Not used by forced startup.
//! apply() is called ONLY inside powered_timer::commit's guard/write critical
//! section. Global bridge_clear revokes preparation and restores AF with MOE off.
use super::*;
use phase_role_sequence::Registers;
use portable_atomic::{AtomicU32, Ordering::Relaxed};
#[cfg(all(feature = "bench-reentry-pwm-stage", feature = "bench-reentry-carrier"))]
compile_error!("PWM staging and period-only recovery experiments are exclusive");
static PREPARED: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "bench-live-control")]
static LIVE_PREPARED: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "bench-reentry-pwm-stage")]
static RECOVERY_COMPARE: AtomicU32 = AtomicU32::new(0);
/// Requires caller's guard/write critical section. Never authorizes a new run.
#[cfg(feature = "bench-live-control")]
pub fn prepared_matches(duty: u32) -> bool {
    duty != 0 && PREPARED.load(Relaxed) == duty
}
#[cfg(feature = "bench-live-control")]
pub fn live_matches(duty: u32) -> bool {
    prepared_matches(duty) && LIVE_PREPARED.load(Relaxed) == duty
}
#[cfg(feature = "bench-live-control")]
pub fn publish_live(duty: u32, _cs: &cortex_m::interrupt::CriticalSection) {
    PREPARED.store(duty, Relaxed);
    LIVE_PREPARED.store(duty, Relaxed);
}
#[cfg(any(
    all(feature = "bench-reverse-32k", feature = "bench-reverse-40k"),
    all(feature = "bench-reverse-32k", feature = "bench-reverse-48k"),
    all(feature = "bench-reverse-40k", feature = "bench-reverse-48k")
))]
compile_error!("reverse 32k/40k/48k carrier A/B features are exclusive");
pub const CARRIER: carrier_profile::Carrier = if cfg!(feature = "bench-reverse-48k") {
    carrier_profile::Carrier::Khz48
} else if cfg!(feature = "bench-reverse-40k") {
    carrier_profile::Carrier::Khz40
} else if cfg!(feature = "bench-reverse-32k") {
    carrier_profile::Carrier::Khz32
} else if cfg!(feature = "bench-reverse-20k") {
    carrier_profile::Carrier::Khz20
} else if cfg!(feature = "bench-current-carrier-24k") {
    carrier_profile::Carrier::Khz24
} else if cfg!(feature = "bench-pwm-20k") {
    carrier_profile::Carrier::Khz20
} else if cfg!(feature = "bench-pwm-24k") {
    carrier_profile::Carrier::Khz24
} else {
    carrier_profile::Carrier::Khz10
};
/// Called before the new powered guard/ADC stream is started, or explicitly
/// ENABLE-low by rolecheck. Never changes a running observation's period.
pub fn prepare_carrier() {
    cortex_m::interrupt::free(|_| unsafe {
        if !powered_timer::outputs_disabled()
            || powered_timer::owns()
            || (*stm32::DMA1::ptr()).ch1().cr().read().bits() & 1 != 0
        {
            panic!("carrier preparation owner");
        }
        let t = &*stm32::TIM1::ptr();
        if t.ccr1().read().bits() != 0 || t.ccr2().read().bits() != 0 || t.ccr3().read().bits() != 0
        {
            panic!("carrier preparation compare");
        }
        PREPARED.store(0, Relaxed);
        t.arr().write(|w| w.bits(CARRIER.arr()));
        t.egr().write(|w| w.bits(1));
    });
}
/// Adopt a gate-disabled startup carrier. A legacy 10k startup may transition
/// once to the BEMF carrier here. ADC uses independent TIM15 TRGO; do not
/// restart it, refresh acquisition stamps, or rephase PWM on later COMs.
pub fn adopt_startup_carrier() -> bool {
    cortex_m::interrupt::free(|_| unsafe {
        let t = &*stm32::TIM1::ptr();
        let valid = !powered_timer::owns()
            && powered_timer::outputs_disabled()
            && t.ccr1().read().bits() == 0
            && t.ccr2().read().bits() == 0
            && t.ccr3().read().bits() == 0;
        if valid {
            if t.arr().read().bits() != CARRIER.arr() {
                // No bridge output is active. UG loads the new ARR once.
                t.arr().write(|w| w.bits(CARRIER.arr()));
                t.egr().write(|w| w.bits(1));
            }
            PREPARED.store(0, Relaxed);
        }
        valid
    })
}
struct Hardware;
/// Diagnostic entry for future recovery preparation. No output authority;
/// ordinary recovery does not call this yet. Global safing revokes both tags.
#[cfg(all(
    feature = "bench-recovery-duty-check",
    not(feature = "bench-recovery-runtime-only")
))]
pub fn prepare_recovery_duty(request: live_duty::Prepared) -> bool {
    prepare_recovery_inner(request, false)
}
#[cfg(feature = "bench-recovery-duty-check")]
fn prepare_recovery_inner(request: live_duty::Prepared, awake: bool) -> bool {
    cortex_m::interrupt::free(|cs| unsafe {
        let t = &*stm32::TIM1::ptr();
        let wake_ok = if awake {
            powered_timer::ready() && powered_timer::reason() == 8
        } else {
            !get_idr(3, 1)
        };
        if !wake_ok
            || powered_timer::owns()
            || core_bench::active()
            || driven_run::owns()
            || !powered_timer::outputs_disabled()
            || PREPARED.load(Relaxed) != 0
            || LIVE_PREPARED.load(Relaxed) != 0
            || request.ticks() != CARRIER.ticks()
            || t.arr().read().bits() != CARRIER.arr()
            || t.psc().read().bits() != 0
            || t.rcr().read().bits() != 0
            || t.cr1().read().bits() & 0xff != 0x81
            || t.dier().read().bits() != 0
            || (*stm32::DMA1::ptr()).ch1().cr().read().bits() & 1 != 0
        {
            return false;
        }
        let duty = phase_role_sequence::prepare_live_disabled(&mut Hardware, request);
        publish_live(duty, cs);
        true
    })
}

/// Called after acquisition's initial shutdown, before any edge sampling.
#[cfg(feature = "bench-reentry-pwm-stage")]
pub fn stage_recovery(duty: u32) -> bool {
    let Some(request) = live_duty::Prepared::new(CARRIER.ticks(), duty) else {
        return false;
    };
    prepare_carrier();
    if !prepare_recovery_inner(request, true) {
        return false;
    }
    RECOVERY_COMPARE.store(request.compare(), Relaxed);
    true
}

/// Revalidate real hardware, not just cached tags. No writes or output authority.
/// No division on the fresh-edge path: comparison was computed before sensing.
#[cfg(feature = "bench-reentry-pwm-stage")]
pub fn recovery_prepared(duty: u32) -> bool {
    let compare = RECOVERY_COMPARE.load(Relaxed);
    compare != 0
        && live_matches(duty)
        && !powered_timer::owns()
        && powered_timer::outputs_disabled()
        && unsafe {
            let t = &*stm32::TIM1::ptr();
            t.arr().read().bits() == CARRIER.arr()
                && t.ccr1().read().bits() == compare
                && t.ccr2().read().bits() == compare
                && t.ccr3().read().bits() == compare
                && t.ccmr1_output().read().bits() == 0x6868
                && t.ccmr2_output().read().bits() == 0x68
                && t.ccer().read().bits() == 0x555
                && (*stm32::GPIOA::ptr()).odr().read().bits() & phase_gpio_plan::A_GATES == 0
                && (*stm32::GPIOB::ptr()).odr().read().bits() & phase_gpio_plan::B_GATES == 0
                && (*stm32::DMA1::ptr()).ch1().cr().read().bits() & 1 == 0
        }
}

#[cfg(all(
    feature = "bench-recovery-duty-check",
    not(feature = "bench-recovery-runtime-only")
))]
pub fn recovery_duty_check<W: Write>(out: &mut W) {
    if get_idr(3, 1)
        || powered_timer::owns()
        || core_bench::active()
        || !powered_timer::outputs_disabled()
    {
        let _ = writeln!(out, "!recoverpwm busy");
        return;
    }
    let mut passed = 0;
    let mut max_us = 0;
    for duty in [duty_envelope::MIN, 100, 220, duty_envelope::MAX] {
        prepare_sine();
        prepare_carrier();
        let request = live_duty::Prepared::new(CARRIER.ticks(), duty).unwrap();
        let start = t17();
        let ok = prepare_recovery_duty(request);
        max_us = max_us.max(t17().wrapping_sub(start));
        let t = unsafe { &*stm32::TIM1::ptr() };
        let mut valid = ok
            && !get_idr(3, 1)
            && t.bdtr().read().bits() & (1 << 15) == 0
            && t.ccr1().read().bits() == request.compare()
            && t.ccr2().read().bits() == request.compare()
            && t.ccr3().read().bits() == request.compare()
            && live_matches(duty)
            && !prepare_recovery_duty(request);
        #[cfg(feature = "bench-reentry-pwm-stage")]
        {
            RECOVERY_COMPARE.store(request.compare(), Relaxed);
            valid &= recovery_prepared(duty) && !recovery_prepared(duty + 1);
            unsafe {
                t.ccr2().write(|w| w.bits(request.compare() + 1));
            }
            valid &= !recovery_prepared(duty);
        }
        gates_off();
        set_pin(3, 1, false);
        #[cfg(feature = "bench-reentry-pwm-stage")]
        {
            valid &= RECOVERY_COMPARE.load(Relaxed) == 0 && !recovery_prepared(duty);
        }
        if !valid || !powered_timer::outputs_disabled() || live_matches(duty) {
            break;
        }
        passed += 1;
    }
    let _ = writeln!(
        out,
        "RECOVERPWM passed={} expected=4 max_us={} disabled={} gate_authority=0",
        passed,
        max_us,
        (!get_idr(3, 1) && powered_timer::outputs_disabled()) as u8
    );
}
impl Registers for Hardware {
    fn blank(&mut self) {
        pwm_moe(false);
        bsrr_a(0, phase_gpio_plan::A_GATES);
        bsrr_b(0, phase_gpio_plan::B_GATES);
    }
    fn prepare_equal(&mut self, compare: u32) {
        unsafe {
            let t = &*stm32::TIM1::ptr();
            t.ccmr1_output().write(|w| w.bits(0x6868));
            t.ccmr2_output().write(|w| w.bits(0x68));
            t.ccer().write(|w| w.bits(0x555));
            t.ccr1().write(|w| w.bits(compare));
            t.ccr2().write(|w| w.bits(compare));
            t.ccr3().write(|w| w.bits(compare));
            t.egr().write(|w| w.bits(1)); // Once per prepared segment, never per COM.
        }
    }
    fn modes(&mut self, p: &phase_gpio_plan::Plan) {
        unsafe {
            (*stm32::GPIOA::ptr())
                .moder()
                .modify(|r, w| w.bits((r.bits() & !phase_gpio_plan::A_MODER_MASK) | p.a_moder));
            (*stm32::GPIOB::ptr())
                .moder()
                .modify(|r, w| w.bits((r.bits() & !phase_gpio_plan::B_MODER_MASK) | p.b_moder));
        }
    }
    fn sink(&mut self, p: &phase_gpio_plan::Plan) {
        unsafe {
            (*stm32::GPIOA::ptr()).bsrr().write(|w| w.bits(p.a_bsrr));
            (*stm32::GPIOB::ptr()).bsrr().write(|w| w.bits(p.b_bsrr));
        }
    }
    fn mux(&mut self, floating: u8) {
        unsafe {
            let old = core::ptr::read_volatile(COMP2_CSR);
            core::ptr::write_volatile(
                COMP2_CSR,
                (old & !(0xF << 4 | 0x3 << 8)) | ((6 + floating as u32) << 4) | (2 << 8),
            );
        }
    }
    fn enable(&mut self) {
        pwm_moe(true);
    }
}
pub fn apply(step: u8, duty: u32) {
    let step = crate::phase_direction::physical_step(step);
    #[cfg(feature = "bench-live-control")]
    if live_matches(duty) {
        if unsafe { (*stm32::TIM1::ptr()).arr().read().bits() } != CARRIER.arr()
            || phase_role_sequence::apply_live_prepared(
                &mut Hardware,
                PREPARED.load(Relaxed),
                step,
                duty,
            )
            .is_err()
        {
            panic!("live role PWM refusal");
        }
        return;
    }
    #[cfg(feature = "bench-pwm-24k")]
    if unsafe { (*stm32::TIM1::ptr()).arr().read().bits() } != CARRIER.arr() {
        panic!("carrier period changed");
    }
    match phase_role_sequence::apply_carrier(
        &mut Hardware,
        PREPARED.load(Relaxed),
        step,
        duty,
        CARRIER,
    ) {
        Ok(prepared) => PREPARED.store(prepared, Relaxed),
        // Local panic handler disables ENABLE and clears GPIO as well as MOE.
        Err(_) => panic!("role PWM refusal"),
    }
}
/// Only AFTER bridge_clear has cleared MOE, CCRs and BOTH GPIO latches.
pub fn revoked_restore_af() {
    revoked_restore_af_inner::<false>();
}
pub fn revoked_restore_af_inner<const KEEP_CARRIER: bool>() {
    #[cfg(feature = "bench-reentry-pwm-stage")]
    RECOVERY_COMPARE.store(0, Relaxed);
    #[cfg(feature = "bench-live-control")]
    LIVE_PREPARED.store(0, Relaxed);
    PREPARED.store(0, Relaxed);
    unsafe {
        #[cfg(feature = "bench-pwm-24k")]
        {
            let t = &*stm32::TIM1::ptr();
            if !KEEP_CARRIER && t.arr().read().bits() != PWM_ARR {
                t.arr().write(|w| w.bits(PWM_ARR));
                t.egr().write(|w| w.bits(1));
            }
        }
        (*stm32::GPIOA::ptr()).moder().modify(|r, w| {
            w.bits(
                (r.bits() & !phase_gpio_plan::A_MODER_MASK)
                    | (2 << 14)
                    | (2 << 16)
                    | (2 << 18)
                    | (2 << 20),
            )
        });
        (*stm32::GPIOB::ptr())
            .moder()
            .modify(|r, w| w.bits((r.bits() & !phase_gpio_plan::B_MODER_MASK) | 10));
    }
}

/// Live validation, not a cached authorization flag. Called before guard start.
#[cfg(feature = "bench-reentry-carrier")]
pub fn recovery_carrier_ready() -> bool {
    !powered_timer::owns()
        && powered_timer::outputs_disabled()
        && PREPARED.load(Relaxed) == 0
        && unsafe {
            (*stm32::TIM1::ptr()).arr().read().bits() == CARRIER.arr()
                && (*stm32::TIM1::ptr()).ccr1().read().bits() == 0
                && (*stm32::TIM1::ptr()).ccr2().read().bits() == 0
                && (*stm32::TIM1::ptr()).ccr3().read().bits() == 0
                && (*stm32::DMA1::ptr()).ch1().cr().read().bits() & 1 == 0
        }
}

// Run only inside rolecheck's ENABLE-low admission. Uses the actual register
// writes and role latch, not a duplicate model of cleanup.
#[cfg(feature = "bench-reentry-carrier")]
fn recovery_carrier_check() -> u32 {
    let mut passed = 0;
    bridge_clear();
    if !recovery_carrier_ready() {
        passed += 1;
    }
    prepare_carrier();
    if recovery_carrier_ready() {
        passed += 1;
    }
    apply(1, 54); // MCU pins only: rolecheck keeps driver ENABLE low.
    if !recovery_carrier_ready() {
        passed += 1;
    }
    bridge_clear_inner::<true>();
    let af = unsafe {
        ((*stm32::GPIOA::ptr()).moder().read().bits() & phase_gpio_plan::A_MODER_MASK)
            == ((2 << 14) | (2 << 16) | (2 << 18) | (2 << 20))
            && ((*stm32::GPIOB::ptr()).moder().read().bits() & phase_gpio_plan::B_MODER_MASK) == 10
            && (*stm32::GPIOA::ptr()).odr().read().bits() & phase_gpio_plan::A_GATES == 0
            && (*stm32::GPIOB::ptr()).odr().read().bits() & phase_gpio_plan::B_GATES == 0
    };
    if recovery_carrier_ready() && af && !get_idr(3, 1) {
        passed += 1;
    }
    bridge_clear();
    if !recovery_carrier_ready()
        && unsafe { (*stm32::TIM1::ptr()).arr().read().bits() == PWM_ARR }
        && powered_timer::outputs_disabled()
        && !get_idr(3, 1)
    {
        passed += 1;
    }
    passed
}

/// Explicit idle diagnostic. Driver ENABLE stays low even while MCU PWM pins
/// exercise roles. Prints only AFTER gates/ENABLE are cleared. Not a motor test.
pub fn check<W: Write>(out: &mut W) {
    check_duty(out, 54);
}
pub fn check_duty<W: Write>(out: &mut W, duty: u32) {
    if !(40..=duty_split::MAX).contains(&duty) {
        let _ = writeln!(out, "!rolecheck invalid_duty");
        return;
    }
    if get_idr(3, 1) || powered_timer::owns() || !powered_timer::outputs_disabled() {
        let _ = writeln!(out, "!rolecheck disabled_only");
        return;
    }
    gates_off();
    set_pin(3, 1, false);
    #[cfg(feature = "bench-reentry-carrier")]
    {
        let passed = recovery_carrier_check();
        let _ = writeln!(
            out,
            "REENTRYCARRIERCHECK passed={} total=5 enable=0",
            passed
        );
        if passed != 5 {
            gates_off();
            set_pin(3, 1, false);
            return;
        }
    }
    #[cfg(feature = "bench-pwm-24k")]
    prepare_carrier();
    let period = CARRIER.ticks();
    let compare = CARRIER.compare(duty).unwrap();
    let mut rows = [[0u32; 5]; 6];
    for step in 1..=6 {
        let result = cortex_m::interrupt::free(|_| unsafe {
            if get_idr(3, 1) {
                return [0, 0, 0, 0, 0];
            }
            // First call initializes; measured second call must NOT issue UG.
            apply(step, duty);
            let t = &*stm32::TIM1::ptr();
            t.cnt().write(|w| w.bits(period / 2));
            let began = t17();
            let before = t.cnt().read().bits();
            apply(step, duty);
            let after = t.cnt().read().bits();
            let elapsed = t17().wrapping_sub(began) as u32;
            let delta = (after + period - before) % period;
            let expected = (elapsed * 64) % period;
            let error = (delta + period - expected) % period;
            // TIM17 quantization plus the two CNT read boundaries; NOT a CPU
            // utilization estimate. <70us disambiguates a full carrier wrap.
            let continuous = CARRIER.continuous(elapsed, delta);
            let p = phase_gpio_plan::plan(crate::phase_direction::physical_step(step)).unwrap();
            let a = &*stm32::GPIOA::ptr();
            let b = &*stm32::GPIOB::ptr();
            let modes = (a.moder().read().bits() & phase_gpio_plan::A_MODER_MASK) == p.a_moder
                && (b.moder().read().bits() & phase_gpio_plan::B_MODER_MASK) == p.b_moder;
            let latches = (a.odr().read().bits() & phase_gpio_plan::A_GATES) == (p.a_bsrr & 65535)
                && (b.odr().read().bits() & phase_gpio_plan::B_GATES) == (p.b_bsrr & 65535);
            // Source AF pins toggle; verify only the GPIO sink/floating pins.
            let source_a = (1 << (10 - p.source)) | if p.source == 2 { 1 << 7 } else { 0 };
            let source_b = if p.source < 2 { 1 << (1 - p.source) } else { 0 };
            let pins = (a.idr().read().bits() & (phase_gpio_plan::A_GATES & !source_a))
                == (p.a_bsrr & 65535)
                && (b.idr().read().bits() & (phase_gpio_plan::B_GATES & !source_b))
                    == (p.b_bsrr & 65535);
            let timer = t.arr().read().bits() == CARRIER.arr()
                && t.ccr1().read().bits() == compare
                && t.ccr2().read().bits() == compare
                && t.ccr3().read().bits() == compare
                && t.ccmr1_output().read().bits() == 0x6868
                && t.ccmr2_output().read().bits() == 0x68
                && t.ccer().read().bits() == 0x555
                && t.bdtr().read().bits() & (1 << 15) != 0;
            let mux = (core::ptr::read_volatile(COMP2_CSR) & (15 << 4 | 3 << 8))
                == ((6 + p.floating as u32) << 4 | 2 << 8);
            let flags = (modes as u32)
                | ((latches as u32) << 1)
                | ((pins as u32) << 2)
                | ((timer as u32) << 3)
                | ((mux as u32) << 4)
                | ((continuous as u32) << 5)
                | ((!get_idr(3, 1) as u32) << 6);
            [step as u32, flags, elapsed, delta, error]
        });
        rows[(step - 1) as usize] = result;
        if result[1] != 127 {
            break;
        }
    }
    gates_off();
    set_pin(3, 1, false);
    let restored = unsafe {
        let a = (*stm32::GPIOA::ptr()).moder().read().bits();
        let b = (*stm32::GPIOB::ptr()).moder().read().bits();
        a & phase_gpio_plan::A_MODER_MASK == ((2 << 14) | (2 << 16) | (2 << 18) | (2 << 20))
            && b & phase_gpio_plan::B_MODER_MASK == 10
            && PREPARED.load(Relaxed) == 0
            && (*stm32::TIM1::ptr()).arr().read().bits() == PWM_ARR
    };
    let safe = !get_idr(3, 1) && powered_timer::outputs_disabled();
    let _ = writeln!(
        out,
        "ROLECHECKPERIOD ticks={} timer_hz=64000000 restored_ticks=6400",
        period
    );
    let _ = writeln!(
        out,
        "ROLECHECKDUTY tenths={} compare={} all_three_checked=1",
        duty, compare
    );
    for r in rows {
        let _ = writeln!(
            out,
            "ROLECHECKPART step={} flags={} elapsed_us={} delta_ticks={} error_mod={}",
            r[0], r[1], r[2], r[3], r[4]
        );
    }
    let _ = writeln!(
        out,
        "ROLECHECK passed={} expected=6 restored={} disabled={} motor_authority=0",
        rows.iter().filter(|r| r[1] == 127).count(),
        restored as u8,
        safe as u8
    );
}
