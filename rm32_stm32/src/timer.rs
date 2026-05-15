//! Timer implementations.
//!
//! TIM2: Interval timer — free-running at 2MHz
//! TIM14/TIM16: Commutation timer — one-shot at 2MHz
//! PSC derived from MCU config to achieve 2MHz regardless of clock speed.
//!
//! Each MCU uses `define_raw_timer!` to generate a zero-sized struct implementing
//! `RawTimer`. `Tim2Interval` and `ComTimerImpl` are generic over `T: RawTimer`,
//! giving IDEs and linters full visibility into the timer API.

use crate::mcu::ChipConfig;
use rm32::hal::{ComTimer, IntervalTimer};

/// Low-level timer register access — implemented per-MCU via `define_raw_timer!`.
///
/// All methods are `&self` on a zero-sized type — the struct carries no data,
/// it just selects which PAC peripheral the methods operate on.
pub trait RawTimer {
    fn read_cr1(&self) -> u32;
    fn write_cr1(&self, v: u32);
    fn read_cnt(&self) -> u32;
    fn write_cnt(&self, v: u32);
    fn write_psc(&self, v: u32);
    fn write_arr(&self, v: u32);
    fn write_egr(&self, v: u32);
    fn write_sr(&self, v: u32);
    fn read_dier(&self) -> u32;
    fn write_dier(&self, v: u32);

    #[inline]
    fn modify_cr1(&self, f: impl FnOnce(u32) -> u32) {
        self.write_cr1(f(self.read_cr1()));
    }

    #[inline]
    fn modify_dier(&self, f: impl FnOnce(u32) -> u32) {
        self.write_dier(f(self.read_dier()));
    }
}

/// Generates a zero-sized struct implementing `RawTimer` for a PAC timer peripheral.
///
/// Two variants handle PAC accessor differences:
/// - `method`: G071/G431 PACs use `tim.cr1()`, `tim.sr()`, etc.
/// - `field`: F051/L431 PACs use `tim.cr1`, `tim.sr`, etc.
#[macro_export]
macro_rules! define_raw_timer {
    (method, $name:ident, $pac_periph:path) => {
        pub struct $name;

        impl $crate::timer::RawTimer for $name {
            // SAFETY: Each method accesses a single PAC peripheral via its singleton PTR.
            // Safe wrappers around unsafe PAC register access.
            #[inline]
            fn read_cr1(&self) -> u32 {
                unsafe { &*<$pac_periph>::PTR }.cr1().read().bits()
            }
            #[inline]
            fn write_cr1(&self, v: u32) {
                unsafe {
                    (&*<$pac_periph>::PTR).cr1().write(|w| w.bits(v));
                }
            }
            #[inline]
            fn read_cnt(&self) -> u32 {
                unsafe { &*<$pac_periph>::PTR }.cnt().read().bits()
            }
            #[inline]
            fn write_cnt(&self, v: u32) {
                unsafe {
                    (&*<$pac_periph>::PTR).cnt().write(|w| w.bits(v));
                }
            }
            #[inline]
            fn write_psc(&self, v: u32) {
                unsafe {
                    (&*<$pac_periph>::PTR).psc().write(|w| w.bits(v));
                }
            }
            #[inline]
            fn write_arr(&self, v: u32) {
                unsafe {
                    (&*<$pac_periph>::PTR).arr().write(|w| w.bits(v));
                }
            }
            #[inline]
            fn write_egr(&self, v: u32) {
                unsafe {
                    (&*<$pac_periph>::PTR).egr().write(|w| w.bits(v));
                }
            }
            #[inline]
            fn write_sr(&self, v: u32) {
                unsafe {
                    (&*<$pac_periph>::PTR).sr().write(|w| w.bits(v));
                }
            }
            #[inline]
            fn read_dier(&self) -> u32 {
                unsafe { &*<$pac_periph>::PTR }.dier().read().bits()
            }
            #[inline]
            fn write_dier(&self, v: u32) {
                unsafe {
                    (&*<$pac_periph>::PTR).dier().write(|w| w.bits(v));
                }
            }
        }
    };
    (field, $name:ident, $pac_periph:path) => {
        pub struct $name;

        impl $crate::timer::RawTimer for $name {
            #[inline]
            fn read_cr1(&self) -> u32 {
                unsafe { &*<$pac_periph>::PTR }.cr1.read().bits()
            }
            #[inline]
            fn write_cr1(&self, v: u32) {
                unsafe { (&*<$pac_periph>::PTR).cr1.write(|w| w.bits(v)) }
            }
            #[inline]
            fn read_cnt(&self) -> u32 {
                unsafe { &*<$pac_periph>::PTR }.cnt.read().bits()
            }
            #[inline]
            fn write_cnt(&self, v: u32) {
                unsafe { (&*<$pac_periph>::PTR).cnt.write(|w| w.bits(v)) }
            }
            #[inline]
            fn write_psc(&self, v: u32) {
                unsafe { (&*<$pac_periph>::PTR).psc.write(|w| w.bits(v)) }
            }
            #[inline]
            fn write_arr(&self, v: u32) {
                unsafe { (&*<$pac_periph>::PTR).arr.write(|w| w.bits(v)) }
            }
            #[inline]
            fn write_egr(&self, v: u32) {
                unsafe { (&*<$pac_periph>::PTR).egr.write(|w| w.bits(v)) }
            }
            #[inline]
            fn write_sr(&self, v: u32) {
                unsafe { (&*<$pac_periph>::PTR).sr.write(|w| w.bits(v)) }
            }
            #[inline]
            fn read_dier(&self) -> u32 {
                unsafe { &*<$pac_periph>::PTR }.dier.read().bits()
            }
            #[inline]
            fn write_dier(&self, v: u32) {
                unsafe { (&*<$pac_periph>::PTR).dier.write(|w| w.bits(v)) }
            }
        }
    };
}

// Raw timer types defined in mcu_xxx/chip.rs, re-exported via mcu::*.
use crate::mcu::{ComTimerRaw, Tim2Raw};

/// TIM2 as free-running interval timer (2MHz, 0.5us/tick).
pub struct Tim2Interval {
    raw: Tim2Raw,
}

impl Default for Tim2Interval {
    fn default() -> Self {
        Self::new()
    }
}

impl Tim2Interval {
    pub fn new() -> Self {
        crate::mcu::enable_tim2_clock();

        let raw = Tim2Raw;
        raw.modify_cr1(|v| v & !(1 << 0)); // CEN=0
        raw.write_psc(crate::mcu::Chip::TIMER_PSC as u32);
        // ARR = 0xFFFF (16-bit wrap, AM32-matched). TIM2 is 32-bit on L4/G4,
        // but AM32's HAL_TIM_Base_Init writes only the low 16 bits, leaving
        // the upper half at 0. Commutation interval measurement uses u16
        // anyway. Matching this avoids any cross-wrap arithmetic surprises.
        raw.write_arr(0xFFFF);
        raw.write_egr(1); // UG
        raw.write_cnt(0);
        raw.modify_cr1(|v| v | (1 << 0)); // CEN=1
        Self { raw }
    }
}

impl IntervalTimer for Tim2Interval {
    fn count(&self) -> u32 {
        self.raw.read_cnt()
    }

    fn set_count(&mut self, val: u32) {
        self.raw.write_cnt(val);
    }
}

/// Free-running commutation timer with ARPE (AM32 pattern).
///
/// TIM14 on G071/F051, TIM16 on L431/G431. Timer runs continuously from
/// boot with CR1 = ARPE | CEN. set_and_enable() writes the new ARR (which
/// goes to the preload register because ARPE=1) and force-loads it via UG,
/// resetting CNT to 0. The timer never stops, so commutation timing has no
/// disable→enable jitter gap.
///
/// Previous one-shot pattern (disable CEN, set ARR, re-enable CEN per
/// commutation) introduced enough timing jitter to make motor control
/// visibly choppy. Captured AM32 register dumps during motor spin show
/// TIM16.CR1=0x81 (ARPE+CEN) which is what this matches.
pub struct Tim14Com {
    raw: ComTimerRaw,
}

impl Default for Tim14Com {
    fn default() -> Self {
        Self::new()
    }
}

impl Tim14Com {
    pub fn new() -> Self {
        crate::mcu::enable_com_timer_clock();

        let raw = ComTimerRaw;
        raw.write_psc(crate::mcu::Chip::TIMER_PSC as u32);
        raw.write_arr(0xFFFF);
        raw.write_egr(1); // UG — latch PSC + ARR into active registers
        // CR1 = ARPE | CEN. Bit 7 = ARPE (auto-reload preload), bit 0 = CEN.
        raw.modify_cr1(|v| v | (1 << 7) | (1 << 0));
        Self { raw }
    }
}

impl ComTimer for Tim14Com {
    fn set_and_enable(&mut self, timeout: u16) {
        // Timer is running (CR1=ARPE+CEN). Write new ARR (goes to preload),
        // issue UG to force-load it into active and reset CNT. UG also sets
        // UIF — clear SR before re-arming UIE so the ISR fires on the next
        // real update event, not on the UG-induced flag.
        self.raw.write_arr(timeout as u32);
        self.raw.write_egr(1); // UG
        self.raw.write_sr(0);
        self.raw.modify_dier(|v| v | 1); // UIE
    }

    fn disable_interrupt(&mut self) {
        self.raw.modify_dier(|v| v & !1);
    }

    fn enable_interrupt(&mut self) {
        self.raw.modify_dier(|v| v | 1);
    }
}
