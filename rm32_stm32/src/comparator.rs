//! Generic BEMF comparator — MCU details via CompOps + ExtiOps traits.

use crate::comp_hal::{CompOps, ExtiOps};
use rm32::board::BemfPins;
use rm32::hal::Comparator as CompTrait;

/// Generic BEMF comparator. Zero cfg blocks — MCU differences in trait impls.
pub struct BemfComparator<C: CompOps, E: ExtiOps> {
    step: u8,
    rising: bool,
    comp: C,
    exti: E,
    bemf_pins: BemfPins,
}

impl<C: CompOps, E: ExtiOps> BemfComparator<C, E> {
    pub fn new(comp: C, exti: E, bemf_pins: BemfPins) -> Self {
        Self {
            step: 1,
            rising: true,
            comp,
            exti,
            bemf_pins,
        }
    }
}

impl<C: CompOps, E: ExtiOps> CompTrait for BemfComparator<C, E> {
    fn set_step(&mut self, step: u8, rising: bool) {
        self.step = step;
        self.rising = rising;
    }

    fn output_level(&self) -> bool {
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

    fn change_input(&mut self) {
        let phase = match self.step {
            1 | 4 => self.bemf_pins.phase_c,
            2 | 5 => self.bemf_pins.phase_a,
            3 | 6 => self.bemf_pins.phase_b,
            _ => self.bemf_pins.phase_c,
        };
        self.comp.set_inmsel(phase);

        if self.rising {
            self.exti.set_falling_edge();
        } else {
            self.exti.set_rising_edge();
        }
    }

    fn enable_interrupts(&mut self) {
        self.exti.enable_interrupt();
    }

    fn mask_interrupts(&mut self) {
        self.exti.mask_and_clear();
    }
}
