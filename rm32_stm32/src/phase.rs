//! Motor phase output control (6-step commutation via GPIO mode switching).
//!
//! PhaseDriver is generic over 6 pin types — all pin identity is resolved
//! at compile time. Runtime methods contain no branches on port or pin number.
//!
//! Pin assignments for HARDWARE_GROUP_G0_A:
//!   Phase A: high=PA10, low=PB1
//!   Phase B: high=PA9,  low=PB0
//!   Phase C: high=PA8,  low=PA7

use crate::gpio_pin::GpioPin;
use crate::gpio_regs::GpioPort;
use core::marker::PhantomData;
use rm32::hal::PhaseOutput;

/// GPIO MODER values.
const MODE_OUTPUT: u32 = 0b01;
const MODE_ALTERNATE: u32 = 0b10;

/// Bench live drive-mode override ('D' command): 0 = follow config,
/// 1 = force diode (comp off), 2 = force complementary. Lets the bench
/// flip drive physics mid-run to separate steady-state comp behavior
/// from the spin-up churn. Read once per com_step — negligible.
#[cfg(feature = "benchuart")]
pub static COMP_PWM_LIVE: core::sync::atomic::AtomicU8 = core::sync::atomic::AtomicU8::new(0);

/// Bench live commutation-writer toggle ('E' command): 0 = sequential
/// per-pin (AM32 phaseouts order), 1 = atomic (clone set_phase_roles:
/// one BSRR write per port then one MODER write per port — final pin
/// levels snap simultaneously, no intermediate bridge states).
///
/// DEFAULT: ATOMIC (1). Deaf-window A/B at 70%/comp (07-26): sequential
/// ~1 per 1-2.5k windows, atomic 4 per 89k (10-20x), diode 1 per 90k,
/// clone 0 per 281k. The sequential interleave's transient bridge
/// states during phase handover are the dominant source of the
/// deaf-window/orbit-entry class under complementary drive. (The old
/// "atomic = null" verdict was measured on camp-storm entries — a
/// metric blind to deaf windows.)
#[cfg(all(feature = "stm32l431", feature = "benchuart"))]
pub static PHASE_ATOMIC_LIVE: core::sync::atomic::AtomicU8 = core::sync::atomic::AtomicU8::new(1);

/// L431 atomic com_step, ported from the clone's proven
/// `set_phase_roles` (minz/src/tim1_motor_pwm.rs) onto rm32's AM32
/// pin naming: A = PA10/PB1, B = PA9/PB0, C = PA8/PA7 (hi/lo).
/// BSRR first (one write per port), MODER second (one modify per
/// port). The sequential writer's six ordered pin ops span ~1-2 µs of
/// mixed old/new bridge states per commutation; this path has none.
#[cfg(feature = "stm32l431")]
pub fn l431_atomic_com_step(step: u8, comp: bool) {
    const AF: u32 = 0b10;
    const OUT: u32 = 0b01;
    // (hi_is_gpioa always true; lo: A,B on GPIOB pins 1,0; C on GPIOA pin 7)
    // phase index 0=A,1=B,2=C → (hi_pin@GPIOA, lo_pin, lo_on_gpioa)
    const PINS: [(u32, u32, bool); 3] = [(10, 1, false), (9, 0, false), (8, 7, true)];
    // per step (1-6): (pwm_phase, low_phase, float_phase)
    const ROLES: [(usize, usize, usize); 6] = [
        (0, 1, 2),
        (2, 1, 0),
        (2, 0, 1),
        (1, 0, 2),
        (1, 2, 0),
        (0, 2, 1),
    ];
    let Some(&(pwm, low, fl)) = ROLES.get((step as usize).wrapping_sub(1)) else {
        return;
    };

    let mut bsrr_a = 0u32;
    let mut bsrr_b = 0u32;
    let mut moder_a_val = 0u32;
    let mut moder_a_mask = 0u32;
    let mut moder_b_val = 0u32;
    let mut moder_b_mask = 0u32;
    let mut set_a = |pin: u32, mode: u32| {
        moder_a_mask |= 0b11 << (pin * 2);
        moder_a_val |= mode << (pin * 2);
    };
    let mut set_b = |pin: u32, mode: u32| {
        moder_b_mask |= 0b11 << (pin * 2);
        moder_b_val |= mode << (pin * 2);
    };

    for (idx, &(hi, lo, lo_a)) in PINS.iter().enumerate() {
        let (hi_mode, lo_mode, hi_lvl, lo_lvl) = if idx == pwm {
            // driven leg: hi AF; lo AF (complementary) or OUTPUT-low (diode)
            (
                AF,
                if comp { AF } else { OUT },
                None,
                if comp { None } else { Some(false) },
            )
        } else if idx == low {
            // low leg: hi off, low FET solid on
            (OUT, OUT, Some(false), Some(true))
        } else {
            debug_assert_eq!(idx, fl);
            // floating leg: both off
            (OUT, OUT, Some(false), Some(false))
        };
        set_a(hi, hi_mode);
        if let Some(l) = hi_lvl {
            bsrr_a |= 1 << (hi + if l { 0 } else { 16 });
        }
        if lo_a {
            set_a(lo, lo_mode);
            if let Some(l) = lo_lvl {
                bsrr_a |= 1 << (lo + if l { 0 } else { 16 });
            }
        } else {
            set_b(lo, lo_mode);
            if let Some(l) = lo_lvl {
                bsrr_b |= 1 << (lo + if l { 0 } else { 16 });
            }
        }
    }

    const GPIOA: u32 = 0x4800_0000;
    const GPIOB: u32 = 0x4800_0400;
    unsafe {
        if bsrr_a != 0 {
            core::ptr::write_volatile((GPIOA + 0x18) as *mut u32, bsrr_a);
        }
        if bsrr_b != 0 {
            core::ptr::write_volatile((GPIOB + 0x18) as *mut u32, bsrr_b);
        }
        let ma = (GPIOA) as *mut u32;
        core::ptr::write_volatile(
            ma,
            (core::ptr::read_volatile(ma) & !moder_a_mask) | moder_a_val,
        );
        let mb = (GPIOB) as *mut u32;
        core::ptr::write_volatile(
            mb,
            (core::ptr::read_volatile(mb) & !moder_b_mask) | moder_b_val,
        );
    }
}

/// Pulse output toggle function — stored as fn pointer to avoid storing raw addresses.
/// Monomorphized per pin type at `enable_pulse_output` call site.
type PulseToggleFn = fn(u32);

/// 3-phase driver, parameterized by 6 compile-time pin types.
///
/// AH/AL = Phase A high/low, BH/BL = Phase B, CH/CL = Phase C.
/// After monomorphization, all port/pin constants are inlined — zero overhead.
pub struct PhaseDriver<AH: GpioPin, AL: GpioPin, BH: GpioPin, BL: GpioPin, CH: GpioPin, CL: GpioPin>
{
    comp_pwm: bool,
    /// PWM/enable bridge mode: low-side pins are enable (output high/low)
    /// instead of complementary PWM (alternate mode).
    bridge_enable: bool,
    /// RPM pulse output toggle function + pin mask.
    pulse: Option<(PulseToggleFn, u32)>,
    _pins: PhantomData<(AH, AL, BH, BL, CH, CL)>,
}

impl<AH: GpioPin, AL: GpioPin, BH: GpioPin, BL: GpioPin, CH: GpioPin, CL: GpioPin>
    PhaseDriver<AH, AL, BH, BL, CH, CL>
{
    pub fn new(comp_pwm: bool) -> Self {
        Self {
            comp_pwm,
            bridge_enable: false,
            pulse: None,
            _pins: PhantomData,
        }
    }

    pub fn new_bridge(comp_pwm: bool) -> Self {
        Self {
            comp_pwm,
            bridge_enable: true,
            pulse: None,
            _pins: PhantomData,
        }
    }

    /// Wire the LOADED config's comp_pwm into the driver. The per-MCU
    /// init constructs the driver before the EEPROM config exists, with
    /// comp_pwm=false as the safe idle default — main MUST call this
    /// after config load or every commutation runs non-complementary
    /// (low-side pin left in GPIO-output during the driven phase; the
    /// freewheel goes through the body diode instead of the low FET).
    /// That silent mismatch was the 8%-wrong-sided-window disparity vs
    /// the clone: config said damped PWM, silicon ran undamped.
    pub fn set_comp_pwm(&mut self, v: bool) {
        self.comp_pwm = v;
    }

    /// Enable RPM pulse output on the given pin.
    /// Creates a monomorphized toggle function for the pin's port.
    pub fn enable_pulse_output<P: GpioPin>(&mut self) {
        P::set_mode(MODE_OUTPUT);
        P::set_low();
        // Capture the port's ODR toggle as a monomorphized fn pointer.
        fn toggle<Port: GpioPort>(mask: u32) {
            Port::write_odr(Port::read_odr() ^ mask);
        }
        self.pulse = Some((toggle::<P::Port>, P::BSRR_SET));
    }

    /// Phase PWM: high-side alternate (TIM1), low-side depends on mode.
    ///
    /// Normal: low-side alternate (comp_pwm) or output LOW.
    /// Bridge: enable pin output HIGH (comp_pwm) or no-op.
    #[inline]
    fn effective_comp_pwm(&self) -> bool {
        #[cfg(feature = "benchuart")]
        {
            match COMP_PWM_LIVE.load(core::sync::atomic::Ordering::Relaxed) {
                1 => return false,
                2 => return true,
                3 => return self.comp_pwm && !crate::isr::shared().old_routine(),
                _ => {}
            }
        }
        // AUTO drive mode — KEPT DIVERGENCE, promoted to ALL builds
        // (Tier B5). AM32 applies complementary drive unconditionally;
        // rm32 runs complementary only in interrupt mode and DIODE during
        // polling/recovery: comp drive's synchronous rectification BRAKES
        // the rotor the polling restart is accelerating, making the churn
        // attractor self-sustaining (measured twice: sustained-comp fell
        // into a permanent ~245 Hz churn on the bench campaign, and the
        // first BF DSHOT300 full ladder under always-comp churned the
        // ENTIRE envelope at <400 Hz e with 16k desyncs / 99.9%
        // excursions, where the AUTO benchuart build holds the parity
        // standard). The 'D' bench lever above can still force either.
        self.comp_pwm && !crate::isr::shared().old_routine()
    }

    #[inline]
    fn phase_pwm<H: GpioPin, L: GpioPin>(&self) {
        let comp = self.effective_comp_pwm();
        if self.bridge_enable {
            if comp {
                L::set_mode(MODE_OUTPUT);
                L::set_high(); // enable on
            }
        } else if !comp {
            L::set_mode(MODE_OUTPUT);
            L::set_low();
        } else {
            L::set_mode(MODE_ALTERNATE);
        }
        H::set_mode(MODE_ALTERNATE);
    }

    /// Phase LOW: low-side/enable on, high-side/PWM off.
    #[inline]
    fn phase_low<H: GpioPin, L: GpioPin>() {
        L::set_mode(MODE_OUTPUT);
        L::set_high();
        H::set_mode(MODE_OUTPUT);
        H::set_low();
    }

    /// Phase FLOAT: both FETs off / enable off.
    #[inline]
    fn phase_float<H: GpioPin, L: GpioPin>() {
        L::set_mode(MODE_OUTPUT);
        L::set_low();
        H::set_mode(MODE_OUTPUT);
        H::set_low();
    }
}

impl<AH: GpioPin, AL: GpioPin, BH: GpioPin, BL: GpioPin, CH: GpioPin, CL: GpioPin> PhaseOutput
    for PhaseDriver<AH, AL, BH, BL, CH, CL>
{
    fn com_step(&mut self, step: u8) {
        #[cfg(all(feature = "stm32l431", feature = "benchuart"))]
        if !self.bridge_enable && PHASE_ATOMIC_LIVE.load(core::sync::atomic::Ordering::Relaxed) != 0
        {
            l431_atomic_com_step(step, self.effective_comp_pwm());
            return;
        }
        match step {
            1 => {
                Self::phase_float::<CH, CL>();
                Self::phase_low::<BH, BL>();
                self.phase_pwm::<AH, AL>();
            }
            2 => {
                Self::phase_float::<AH, AL>();
                Self::phase_low::<BH, BL>();
                self.phase_pwm::<CH, CL>();
            }
            3 => {
                Self::phase_float::<BH, BL>();
                Self::phase_low::<AH, AL>();
                self.phase_pwm::<CH, CL>();
            }
            4 => {
                Self::phase_float::<CH, CL>();
                Self::phase_low::<AH, AL>();
                self.phase_pwm::<BH, BL>();
            }
            5 => {
                Self::phase_float::<AH, AL>();
                Self::phase_low::<CH, CL>();
                self.phase_pwm::<BH, BL>();
            }
            6 => {
                Self::phase_float::<BH, BL>();
                Self::phase_low::<CH, CL>();
                self.phase_pwm::<AH, AL>();
            }
            _ => {}
        }
    }

    fn all_off(&mut self) {
        Self::phase_float::<AH, AL>();
        Self::phase_float::<BH, BL>();
        Self::phase_float::<CH, CL>();
    }

    fn full_brake(&mut self) {
        Self::phase_low::<AH, AL>();
        Self::phase_low::<BH, BL>();
        Self::phase_low::<CH, CL>();
    }

    fn all_pwm(&mut self) {
        self.phase_pwm::<AH, AL>();
        self.phase_pwm::<BH, BL>();
        self.phase_pwm::<CH, CL>();
    }

    fn proportional_brake(&mut self) {
        if self.bridge_enable {
            return; // not supported on PWM/enable bridge boards
        }
        AH::set_mode(MODE_OUTPUT);
        AH::set_low();
        BH::set_mode(MODE_OUTPUT);
        BH::set_low();
        CH::set_mode(MODE_OUTPUT);
        CH::set_low();
        AL::set_mode(MODE_ALTERNATE);
        BL::set_mode(MODE_ALTERNATE);
        CL::set_mode(MODE_ALTERNATE);
    }

    fn pulse_toggle(&mut self, step: u8) {
        if let Some((toggle_fn, mask)) = self.pulse {
            if step == 1 || step == 4 {
                toggle_fn(mask);
            }
        }
    }
}

// --- Board-specific type aliases ---

use crate::gpio_pin::{PA7, PA8, PA9, PA10, PB0, PB1};

/// G0_A / F0_A / L4_N pin assignment (all three MCUs share the same pins).
pub type G0APhaseDriver = PhaseDriver<PA10, PB1, PA9, PB0, PA8, PA7>;
