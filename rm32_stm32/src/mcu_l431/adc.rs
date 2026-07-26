//! L431 ADC: CH8 current, CH11 voltage, CH17 temp. DMA1_CH1 circular.

/// Bench live ADC pause ('A' command).
#[cfg(feature = "benchuart")]
pub static ADC_PAUSE: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

// ---- WAXWING-lite (port of minz waxwing-lite-v1 = 25bd843) ----
// 4x1024 u16 rings written every 20 kHz tick while the injected burst
// is armed ('J'): A, B = raw mid-ON phase samples (JDR1/2), POS =
// TIM2.CNT (0.5 us), T1S = (step<<12)|(TIM1.CNT & 0x0FFF). Sole
// writer = the tick ISR; a dump after a fall is a ~52 ms post-mortem.
#[cfg(feature = "benchuart")]
pub const WAX_N: usize = 1024;
#[cfg(feature = "benchuart")]
static WAX_A: [core::sync::atomic::AtomicU16; 1024] =
    [const { core::sync::atomic::AtomicU16::new(0) }; 1024];
#[cfg(feature = "benchuart")]
static WAX_B: [core::sync::atomic::AtomicU16; 1024] =
    [const { core::sync::atomic::AtomicU16::new(0) }; 1024];
#[cfg(feature = "benchuart")]
static WAX_POS: [core::sync::atomic::AtomicU16; 1024] =
    [const { core::sync::atomic::AtomicU16::new(0) }; 1024];
#[cfg(feature = "benchuart")]
static WAX_T1S: [core::sync::atomic::AtomicU16; 1024] =
    [const { core::sync::atomic::AtomicU16::new(0) }; 1024];
#[cfg(feature = "benchuart")]
static WAX_HEAD: core::sync::atomic::AtomicU16 = core::sync::atomic::AtomicU16::new(0);
/// Freeze-on-fall (cousin's black-box correction): the ring is 52 ms
/// deep and always-writing — without a freeze at the fall classifier,
/// churn overwrites the deaf window long before any human-triggered
/// dump. Frozen -> tick skips writes; the 'x' dump reports and then
/// re-arms.
#[cfg(feature = "benchuart")]
static WAX_FROZEN: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

/// Freeze only when the rings are actually being written (injected
/// burst armed) — a freeze before 'J' preserves an empty ring.
#[cfg(feature = "benchuart")]
pub fn wax_freeze() -> bool {
    let adc = unsafe { &*stm32l4xx_hal::pac::ADC1::ptr() };
    if adc.jsqr.read().bits() == 0 {
        return false;
    }
    WAX_FROZEN.store(true, core::sync::atomic::Ordering::Relaxed);
    true
}

#[cfg(feature = "benchuart")]
pub fn wax_frozen() -> bool {
    WAX_FROZEN.load(core::sync::atomic::Ordering::Relaxed)
}

#[cfg(feature = "benchuart")]
pub fn wax_rearm() {
    WAX_FROZEN.store(false, core::sync::atomic::Ordering::Relaxed);
}

/// 20 kHz tick writer (ISR context, constant cost). No-op until the
/// injected burst has been armed ('J').
#[cfg(feature = "benchuart")]
pub fn wax_tick(step: u8) {
    use core::sync::atomic::Ordering;
    use stm32l4xx_hal::pac::{ADC1, TIM1, TIM2};
    let adc = unsafe { &*ADC1::ptr() };
    if adc.jsqr.read().bits() == 0 || WAX_FROZEN.load(Ordering::Relaxed) {
        return;
    }
    let h = WAX_HEAD.load(Ordering::Relaxed) as usize % WAX_N;
    let a = (adc.jdr1.read().bits() & 0xFFF) as u16;
    let b = (adc.jdr2.read().bits() & 0xFFF) as u16;
    let pos = unsafe { ((*TIM2::ptr()).cnt.read().bits() & 0xFFFF) as u16 };
    let t1 = unsafe { ((*TIM1::ptr()).cnt.read().bits() & 0x0FFF) as u16 };
    WAX_A[h].store(a, Ordering::Relaxed);
    WAX_B[h].store(b, Ordering::Relaxed);
    WAX_POS[h].store(pos, Ordering::Relaxed);
    WAX_T1S[h].store(((step as u16) << 12) | t1, Ordering::Relaxed);
    WAX_HEAD.store(((h + 1) % WAX_N) as u16, Ordering::Relaxed);
}

#[cfg(feature = "benchuart")]
pub fn wax_read(i: usize) -> (u16, u16, u16, u16) {
    use core::sync::atomic::Ordering;
    let i = i % WAX_N;
    (
        WAX_A[i].load(Ordering::Relaxed),
        WAX_B[i].load(Ordering::Relaxed),
        WAX_POS[i].load(Ordering::Relaxed),
        WAX_T1S[i].load(Ordering::Relaxed),
    )
}

#[cfg(feature = "benchuart")]
pub fn wax_head() -> usize {
    WAX_HEAD.load(core::sync::atomic::Ordering::Relaxed) as usize
}

// ---- GECKO on-demand current microscope (port of minz b720565) ----
// Free-run ch8 (current shunt) at max rate into a 2048-word DMA ring
// (~0.55 ms = ~9 windows at 58 µs), one-shot inside a 'g' capture.
// rm32 difference vs the clone: our regular group IS the 1 kHz scan,
// so the capture pauses the scan, borrows DMA CH1, and restores both.
// Guards are blind ~1 ms during the grab — acceptable one-shot.
#[cfg(feature = "benchuart")]
pub const GECKO_FRAMES: usize = 2048;
#[cfg(feature = "benchuart")]
static GECKO_RING: [core::sync::atomic::AtomicU16; 2048] =
    [const { core::sync::atomic::AtomicU16::new(0) }; 2048];
#[cfg(feature = "benchuart")]
static SCAN_BUF_PTR: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
#[cfg(feature = "benchuart")]
static SCAN_BUF_LEN: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);

#[cfg(feature = "benchuart")]
pub fn gecko_word(i: usize) -> u16 {
    GECKO_RING[i % GECKO_FRAMES].load(core::sync::atomic::Ordering::Relaxed)
}

/// One-shot GECKO capture. Main context only. Returns the ring head
/// (oldest-sample index). Scan is paused, CH1 borrowed, both restored.
#[cfg(feature = "benchuart")]
pub fn gecko_capture() -> usize {
    use core::sync::atomic::Ordering;
    use stm32l4xx_hal::pac::{ADC1, DMA1};
    let adc = unsafe { &*ADC1::ptr() };
    let dma = unsafe { &*DMA1::ptr() };
    ADC_PAUSE.store(true, Ordering::Relaxed);
    // Quiesce any in-flight scan sequence (bounded).
    if adc.cr.read().adstart().bit_is_set() {
        adc.cr.modify(|_, w| w.adstp().set_bit());
        for _ in 0..100_000u32 {
            if adc.cr.read().adstart().bit_is_clear() {
                break;
            }
        }
    }
    let saved_sqr1 = adc.sqr1.read().bits();
    unsafe {
        // ch8 only, continuous, overwrite on overrun; keep DMAEN|DMACFG.
        adc.sqr1.write(|w| w.bits(8 << 6));
        adc.cfgr
            .modify(|r, w| w.bits(r.bits() | (1 << 13) | (1 << 12)));
        dma.ccr1.modify(|r, w| w.bits(r.bits() & !1));
        dma.cmar1.write(|w| w.bits(GECKO_RING.as_ptr() as u32));
        dma.cndtr1.write(|w| w.bits(GECKO_FRAMES as u32));
        dma.ccr1.modify(|r, w| w.bits(r.bits() | 1));
    }
    adc.cr.modify(|_, w| w.adstart().set_bit());
    // Ring wraps once: 2048 samples at ~2.6 Msps-ish -> spin ~1 ms.
    cortex_m::asm::delay(80_000 * 2);
    adc.cr.modify(|_, w| w.adstp().set_bit());
    for _ in 0..100_000u32 {
        if adc.cr.read().adstart().bit_is_clear() {
            break;
        }
    }
    let head = (GECKO_FRAMES - dma.cndtr1.read().bits() as usize) % GECKO_FRAMES;
    unsafe {
        // Restore the 1 kHz scan: CH1 back to the 3-word buffer,
        // continuous+ovrmod off, sequence back.
        dma.ccr1.modify(|r, w| w.bits(r.bits() & !1));
        dma.cmar1
            .write(|w| w.bits(SCAN_BUF_PTR.load(Ordering::Relaxed)));
        dma.cndtr1
            .write(|w| w.bits(SCAN_BUF_LEN.load(Ordering::Relaxed)));
        dma.ccr1.modify(|r, w| w.bits(r.bits() | 1));
        adc.cfgr
            .modify(|r, w| w.bits(r.bits() & !((1 << 13) | (1 << 12))));
        adc.sqr1.write(|w| w.bits(saved_sqr1));
    }
    ADC_PAUSE.store(false, Ordering::Relaxed);
    head
}

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
        // Stashed so the GECKO capture can restore the scan's DMA target
        // after borrowing CH1 (see gecko_capture).
        #[cfg(feature = "benchuart")]
        {
            use core::sync::atomic::Ordering;
            SCAN_BUF_PTR.store(buf_ptr as u32, Ordering::Relaxed);
            SCAN_BUF_LEN.store(buf_len as u32, Ordering::Relaxed);
        }
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
        // WAXWING burst (clone waxwing-lite-v1 = 25bd843): JL=3 (4 conv),
        // JEXTSEL=0b1000 (TIM1_TRGO2), JEXTEN=0b10 (falling),
        // JSQ1=ch9 (PA4 phase A), JSQ2=ch10 (PA5 phase B),
        // JSQ3=ch8 (current), JSQ4=ch11 (vbat).
        adc.jsqr.write(|w| {
            w.bits(3 | (0b1000 << 2) | (0b10 << 6) | (9 << 8) | (10 << 14) | (8 << 20) | (11 << 26))
        });
        // Phase channels at 47.5 cycles (smp9 = SMPR1[29:27],
        // smp10 = SMPR2[2:0]).
        adc.smpr1.modify(|r, w| w.bits(r.bits() | (0b100 << 27)));
        adc.smpr2.modify(|r, w| w.bits(r.bits() | 0b100));
        // Arm. Hardware triggers launch conversions from here on.
        adc.cr.modify(|r, w| w.bits(r.bits() | (1 << 3))); // JADSTART
    }
}

/// Latest injected current sample (raw ADC counts), any context.
#[cfg(feature = "benchuart")]
pub fn injected_current_raw() -> u16 {
    // WAXWING burst order: current is JDR3 (JDR1/2 = phase A/B).
    unsafe { ((*stm32l4xx_hal::pac::ADC1::ptr()).jdr3.read().bits() & 0xFFF) as u16 }
}
