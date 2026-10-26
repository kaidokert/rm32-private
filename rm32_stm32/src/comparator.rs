//! Generic BEMF comparator — MCU details via CompOps + ExtiOps traits.

use crate::board_ext::BemfPins;
use crate::comp_hal::{CompOps, ExtiOps};
use rm32::hal::{BemfSensor, Phase};

/// Generic BEMF comparator. Zero cfg blocks — MCU differences in trait impls.
pub struct BemfComparator<C: CompOps, E: ExtiOps> {
    comp: C,
    exti: E,
    bemf_pins: BemfPins,
}

impl<C: CompOps, E: ExtiOps> BemfComparator<C, E> {
    pub fn new(comp: C, exti: E, bemf_pins: BemfPins) -> Self {
        Self {
            comp,
            exti,
            bemf_pins,
        }
    }
}

impl<C: CompOps, E: ExtiOps> BemfSensor for BemfComparator<C, E> {
    fn level(&self) -> bool {
        self.comp.output()
    }

    fn filter_read(&self) -> bool {
        // G071: one persistence-filter read costs what AM32's does, MEASURED
        // on the chip (mcu_g071::filter_cal, boot line `filter-read cyc`):
        // AM32's verbatim loop (`interruptRoutine` -> `getCompOutputLevel`
        // from AM32_DRV8304H_G071_2.20.elf: BL, flash-literal + pointer + CSR
        // loads, BX, `rising`/`filter_level` reloads) = 30.6 cycles per read,
        // with prefetch on or off; rm32's inlined read = 12.6. With half
        // AM32's time window per read, sub-1.5 us noise pulses on the floating
        // phase passed the filter: binz per-sector histograms showed the
        // sector-2 crossing (phase A falling) accepted >= 15 us early in
        // ~10-15 % of revolutions at 70-82.5 % duty. 14 NOPs (1 cycle each
        // with PRFTEN on, which init sets as AM32 does) = 30.6 cycles: per-read
        // parity. (An earlier 16-NOP pad was sized from a static estimate with
        // prefetch OFF, where a NOP costs 1.5 cycles: 44.6 cycles = 1.46x
        // AM32.) FILTER reads only: padding every read (gate/camping, pre-ZC
        // checks) made UART-coupled comparator storms starve the main loop
        // into an IWDG reset at 82.5 % (binz); those paths keep the fast read.
        #[cfg(feature = "stm32g071")]
        unsafe {
            core::arch::asm!(
                ".rept 14",
                "nop",
                ".endr",
                options(nomem, nostack, preserves_flags)
            );
        }
        self.comp.output()
    }

    #[inline(always)]
    fn select_phase(&mut self, phase: Phase, rising: bool) {
        let pin = match phase {
            Phase::A => self.bemf_pins.phase_a,
            Phase::B => self.bemf_pins.phase_b,
            Phase::C => self.bemf_pins.phase_c,
        };
        self.comp.set_inmsel(pin);

        if rising {
            self.exti.set_falling_edge();
        } else {
            self.exti.set_rising_edge();
        }
    }

    fn enable_irq(&mut self) {
        self.exti.enable_interrupt();
    }

    fn mask_irq(&mut self) {
        self.exti.mask_and_clear();
    }
}
