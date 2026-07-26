//! L431 ADC: CH8 current, CH11 voltage, CH17 temp. DMA1_CH1 circular.

/// Bench live ADC pause ('A' command).
#[cfg(feature = "benchuart")]
pub static ADC_PAUSE: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

use crate::adc_hal::AdcPeripheral;
use crate::pac::{ADC_COMMON, ADC1, DMA1, GPIOA, RCC};
use crate::regs::{InitError, wait_for};

crate::define_adc_boilerplate!(
    ops: L431AdcOps,
    type_name: L431Adc,
    cal1: 0x1FFF_75A8, cal2: 0x1FFF_75CA,
    cal1_temp: 30, cal2_temp: 130,
);

pub struct L431AdcOps;

impl AdcPeripheral for L431AdcOps {
    fn enable_clocks(&self) {
        let rcc = unsafe { &*RCC::ptr() };
        rcc.ahb2enr
            .modify(|_, w| w.adcen().set_bit().gpioaen().set_bit());
        rcc.ahb1enr.modify(|_, w| w.dma1en().set_bit());
    }

    fn configure_pins(&self) {
        let gpioa = unsafe { &*GPIOA::ptr() };
        gpioa
            .moder
            .modify(|_, w| w.moder3().bits(0b11).moder6().bits(0b11));
    }

    fn configure_clock_source(&self) {
        let adc_common = unsafe { &*ADC_COMMON::ptr() };
        adc_common
            .ccr
            .modify(|_, w| unsafe { w.ckmode().bits(0b01) });
    }

    fn enable_temp_sensor(&self) {
        let adc_common = unsafe { &*ADC_COMMON::ptr() };
        adc_common.ccr.modify(|_, w| w.ch17sel().set_bit());
    }

    fn configure_dma(&self, buf_ptr: *const u16, buf_len: u16) {
        let adc = unsafe { &*ADC1::ptr() };
        let dma = unsafe { &*DMA1::ptr() };

        dma.cselr
            .modify(|r, w| unsafe { w.bits(r.bits() & !(0xF << 0)) });
        dma.ccr1.write(|w| unsafe { w.bits(0) });
        dma.cpar1
            .write(|w| unsafe { w.bits(adc.dr.as_ptr() as u32) });
        dma.cmar1.write(|w| unsafe { w.bits(buf_ptr as u32) });
        dma.cndtr1.write(|w| unsafe { w.bits(buf_len as u32) });
        dma.ccr1
            .write(|w| unsafe { w.bits((1 << 5) | (1 << 7) | (0b01 << 8) | (0b01 << 10)) });
        dma.ccr1.modify(|r, w| unsafe { w.bits(r.bits() | 1) });
    }

    fn configure_sampling(&self) {
        let adc = unsafe { &*ADC1::ptr() };
        adc.smpr1.modify(|_, w| unsafe { w.smp8().bits(0b100) });
        adc.smpr2.modify(|_, w| unsafe { w.smp11().bits(0b100) });
        adc.smpr2.modify(|_, w| unsafe { w.smp17().bits(0b100) });
    }

    fn configure_sequence(&self) {
        let adc = unsafe { &*ADC1::ptr() };
        adc.sqr1
            .write(|w| unsafe { w.l().bits(2).sq1().bits(8).sq2().bits(11).sq3().bits(17) });
    }

    fn enable_dma_mode(&self) {
        let adc = unsafe { &*ADC1::ptr() };
        adc.cfgr.write(|w| unsafe { w.bits((1 << 0) | (1 << 1)) });
    }

    fn power_up(&self) {
        let adc = unsafe { &*ADC1::ptr() };
        adc.cr.modify(|_, w| w.deeppwd().clear_bit());
        adc.cr.modify(|_, w| w.advregen().set_bit());
        cortex_m::asm::delay(80 * 20);
    }

    fn calibrate(&self) -> Result<(), InitError> {
        let adc = unsafe { &*ADC1::ptr() };
        adc.cr.modify(|_, w| w.adcaldif().clear_bit());
        adc.cr.modify(|_, w| w.adcal().set_bit());
        wait_for(|| !adc.cr.read().adcal().bit_is_set(), 100_000, "ADC cal")?;
        cortex_m::asm::delay(80 * 20);
        Ok(())
    }

    fn enable(&self) -> Result<(), InitError> {
        let adc = unsafe { &*ADC1::ptr() };
        adc.isr.write(|w| unsafe { w.bits(1 << 0) });
        adc.cr.modify(|_, w| w.aden().set_bit());
        wait_for(|| adc.isr.read().adrdy().bit_is_set(), 100_000, "ADC ready")?;
        Ok(())
    }

    fn start_conversion(&self) {
        // Bench 'A' diagnostic: paused ADC = no mux switching (clone's
        // parity-tag ADC was dormant); measurements freeze while paused.
        #[cfg(feature = "benchuart")]
        if ADC_PAUSE.load(core::sync::atomic::Ordering::Relaxed) {
            return;
        }
        let adc = unsafe { &*ADC1::ptr() };
        adc.cr.modify(|_, w| w.adstart().set_bit());
    }
}

/// Hardware-timed injected current sampling — the clone's FALCON
/// pattern (minz/src/adc_sync.rs), minimal port: ONE injected
/// conversion of ch8 (PA3, current shunt amp) triggered by TIM1_TRGO2
/// = OC4REF falling (CCR4=0x64 places the sample mid-PWM-ON). The
/// regular software-scanned group (vbat/temp @1 kHz) is untouched;
/// injected results land in JDR1 only. JADSTART must be set ONCE to
/// arm hardware-triggered injected conversions (the JADSTART scar).
/// Reader: `injected_current_raw` per commutation for the probe row.
#[cfg(feature = "benchuart")]
pub fn arm_injected_current() {
    use stm32l4xx_hal::pac::{ADC1, TIM1};
    unsafe {
        let adc = &*ADC1::ptr();
        let tim1 = &*TIM1::ptr();
        // TIM1 TRGO2 = OC4REF (MMS2 = 0b0111). CCR4 already 0x64.
        tim1.cr2
            .modify(|r, w| w.bits((r.bits() & !(0xF << 20)) | (0b0111 << 20)));
        // JQDIS: plain JSQR injected mode (no queue).
        adc.cfgr.modify(|r, w| w.bits(r.bits() | (1 << 31)));
        // JSQR: JL=0 (1 conv) | JEXTSEL=0b1000 (TIM1_TRGO2) |
        // JEXTEN=0b10 (falling) | JSQ1=8 (current).
        adc.jsqr
            .write(|w| w.bits((0b1000 << 2) | (0b10 << 6) | (8 << 8)));
        // Arm. Hardware triggers launch conversions from here on.
        adc.cr.modify(|r, w| w.bits(r.bits() | (1 << 3))); // JADSTART
    }
}

/// Latest injected current sample (raw ADC counts), any context.
#[cfg(feature = "benchuart")]
pub fn injected_current_raw() -> u16 {
    unsafe { ((*stm32l4xx_hal::pac::ADC1::ptr()).jdr1.read().bits() & 0xFFFF) as u16 }
}
