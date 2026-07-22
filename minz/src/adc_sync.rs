//! Hybrid ADC: injected group = PWM-synchronous mid-ON sampling for
//! control; regular group = free-running current oversample for the
//! spike microscope (GECKO / GECKO-scope).
//!
//! Two ADC1 conversion groups run at once:
//!
//! * **Injected group** — hardware-triggered by TIM1 TRGO2 (OC4REF
//!   falling edge at `CNT == CCR4`, i.e. `SAMPLE_TICKS` into each PWM
//!   cycle, deep in the high-side ON window). Four channels in one
//!   burst: ch9 (PA4, phase A), ch10 (PA5, phase B), ch8 (current),
//!   ch11 (PA6, vbat). `inj_read()` returns all four from JDR1-4.
//!   This REPRODUCES the old regular-group mid-ON `(A, B, current)`
//!   triplet bit-for-bit (same channels, same 47.5-cycle sampling on
//!   the razor-margin phase channels) and folds vbat in — so every
//!   control consumer (FALCON adc-confirm, vbus decay, overcurrent,
//!   the sag kill) sees an identical deterministic sample, plus vbat
//!   for free. The injected sequence PREEMPTS the regular free-run,
//!   so no regular conversion can delay it past the ON window.
//!
//! * **Regular group** — ch8 (current) only, CONTINUOUS free-run (no
//!   trigger), DMA1_CH1 circular into [`CUR_RING`]. At ~25 ADC cycles
//!   per conversion this yields ~150 current samples per 24 kHz PWM
//!   cycle: an intra-cycle current microscope. `CUR_RING` holds
//!   ~0.55 ms (2048 samples) — sized as a PRE-TRIGGER buffer: freeze
//!   on a >4 A spike and the ring holds the onset (first ZC miss
//!   through the 4 A crossing) at ~0.27 µs resolution, enough to tell
//!   a smooth winding-limited BEMF-aided ramp (4-5 A) from a
//!   sub-µs shoot-through transient (20-100 A) at a switching edge.
//!
//! The injected preemption punches a ~2.2 µs hole in the regular
//! stream once per cycle at mid-ON; harmless for the ms-scale current
//! shape, and marked by the per-cycle `CUR_RING`-head context the
//! firmware latches.
//!
//! After [`start`] runs, the HAL's `OneShot::read` (`SenseAdc`) must
//! NOT be called — it rewrites SQR/CFGR under the armed groups.
//! `SenseAdc` remains useful for power-up/calibration and `adc_to_mv`.

use core::sync::atomic::{AtomicU16, Ordering};

use crate::hal::stm32;
use crate::hal::stm32::{ADC1, TIM1};

/// Injected-group trigger point within the PWM cycle, TIM1 ticks
/// (80 MHz). 100 ticks = 1.25 µs (AM32's own CCR4): the 4-channel
/// injected sequence (A/B @47.5, current @12.5, vbat @47.5) must
/// finish INSIDE the high-side ON window, or the phase channels
/// sample low-side recirculation (~0 V). Whole burst ends ~3.5 µs;
/// ON = duty ≥ 333 ticks = 4.2 µs at amp 10, so it fits with margin
/// down to amp ≈ 8. Hard FLOOR: below ~1.0 µs the first channel
/// samples during dead-time + FET turn-on and reads 0. vbat is last
/// (phase-insensitive: a battery-divider node, valid in ON or OFF).
pub const SAMPLE_TICKS: u16 = 100;

/// Free-running current ring (GECKO-scope): ch8 samples filled
/// circularly by DMA1_CH1 at the ADC's continuous rate (~150 / PWM
/// cycle). 2048 samples ≈ 0.55 ms, 4 KiB. Atomics so DMA's hardware
/// writes and main-context dump reads never alias a `&mut`; reads are
/// only self-consistent while the capture is frozen, EXCEPT
/// [`last_raw`] which deliberately samples the live newest word.
pub const CUR_FRAMES: usize = 2048;
static CUR_RING: [AtomicU16; CUR_FRAMES] = [const { AtomicU16::new(0) }; CUR_FRAMES];

/// Read one current-ring word (frozen capture only, except via
/// [`last_raw`]).
#[inline]
pub fn cur_word(i: usize) -> u16 {
    CUR_RING[i % CUR_FRAMES].load(Ordering::Relaxed)
}

/// DMA write head = index of the NEXT slot the DMA will fill. The
/// newest completed sample is `(head - 1) % CUR_FRAMES`. Latched by
/// the firmware once per cycle (context marker) and at each
/// commutation (LPTIM2) so the host can map cycles/commutations onto
/// the oversampled stream.
#[inline]
pub fn cur_head() -> usize {
    let dma = unsafe { &*stm32::DMA1::ptr() };
    let remaining = dma.cndtr1.read().bits() as usize;
    (CUR_FRAMES - remaining) % CUR_FRAMES
}

/// Configure and start both ADC groups. Call after TIM1 init and
/// after the HAL `ADC::new` power-up/calibration (`SenseAdc::new`).
pub fn start(sample_ticks: u16) {
    let tim1 = unsafe { &*TIM1::ptr() };
    let adc = unsafe { &*ADC1::ptr() };

    // TIM1: sample point + OC4REF as TRGO2 (drives the INJECTED
    // trigger now). CCR4 is preloaded (OC4PE at init) — next update.
    tim1.ccr4.write(|w| w.ccr().bits(sample_ticks));
    // CR2.MMS2[23:20] = 0b0111 (OC4REF → TRGO2). No PAC accessor.
    tim1.cr2
        .modify(|r, w| unsafe { w.bits((r.bits() & !(0xF << 20)) | (0b0111 << 20)) });

    // Enable ADC idempotently (RM0394 16.4.9). All flag waits bounded
    // (crate::spin — unbounded hardware-flag spins are banned).
    if adc.cr.read().aden().bit_is_clear() {
        crate::spin::spin_until(100_000, || adc.cr.read().addis().bit_is_clear());
        adc.isr.write(|w| w.adrdy().set_bit());
        adc.cr.modify(|_, w| w.aden().set_bit());
        crate::spin::spin_until(100_000, || adc.isr.read().adrdy().bit_is_set());
    }
    // SQR/JSQR/SMPR/CFGR writes require no conversion in flight.
    if adc.cr.read().adstart().bit_is_set() {
        adc.cr.modify(|_, w| w.adstp().set_bit());
        crate::spin::spin_until(100_000, || adc.cr.read().adstart().bit_is_clear());
    }
    if adc.cr.read().jadstart().bit_is_set() {
        adc.cr.modify(|_, w| w.jadstp().set_bit());
        crate::spin::spin_until(100_000, || adc.cr.read().jadstart().bit_is_clear());
    }

    // Sample times: 47.5 cycles on the phase channels (ch9/ch10) —
    // REQUIRED with the 3.3 kΩ divider sources; shorter blinds the
    // sector-2 confirm (2×A ≈ vbus at the ZC, razor margin), proven
    // by A/B at both carriers. ch8 (current) 12.5 — INA180 output is
    // low-Z, and the short sample is what lets the injected burst fit
    // the ON window AND gives the free-run ring its ~150 samples/cycle
    // rate. ch11 (vbat) 47.5: in the injected burst (0.75 µs), no
    // longer the 8.2 µs software pump that used to collide mid-ON.
    adc.smpr1
        .modify(|_, w| unsafe { w.smp8().bits(0b010).smp9().bits(0b100) });
    adc.smpr2
        .modify(|_, w| unsafe { w.smp10().bits(0b100).smp11().bits(0b100) });

    // Regular group: ch8 only (L=0), CONTINUOUS free-run (CFGR CONT
    // below, no EXTEN) → DMA ring.
    adc.sqr1.write(|w| unsafe { w.l().bits(0).sq1().bits(8) });

    // Injected group: 4 conversions ch9(A), ch10(B), ch8(current),
    // ch11(vbat), hardware-triggered on TIM1_TRGO2 falling edge.
    // JSQR: JL[1:0]=3 | JEXTSEL[5:2]=0b1000 (TIM1_TRGO2, RM0394
    // injected trigger table) | JEXTEN[7:6]=0b10 (falling) |
    // JSQ1[12:8]=9 | JSQ2[18:14]=10 | JSQ3[24:20]=8 | JSQ4[30:26]=11.
    adc.jsqr.write(|w| unsafe {
        w.bits(3 | (0b1000 << 2) | (0b10 << 6) | (9 << 8) | (10 << 14) | (8 << 20) | (11 << 26))
    });

    // DMA1_CH1 ← ADC1 (CSELR C1S = 0000): circular, 16-bit both
    // sides, memory-increment, draining the ch8 regular stream into
    // the current ring. Only the regular group feeds DR/DMA — injected
    // results land in JDR1-4, untouched by the DMA.
    unsafe {
        (*stm32::RCC::ptr())
            .ahb1enr
            .modify(|_, w| w.dma1en().set_bit());
        let dma = &*stm32::DMA1::ptr();
        dma.cselr.modify(|r, w| w.bits(r.bits() & !0xF));
        dma.ccr1.write(|w| w.bits(0));
        dma.cpar1.write(|w| w.bits(adc.dr.as_ptr() as u32));
        dma.cmar1.write(|w| w.bits(CUR_RING.as_ptr() as u32));
        dma.cndtr1.write(|w| w.bits(CUR_FRAMES as u32));
        // MINC | CIRC | PSIZE=16 | MSIZE=16, then EN.
        dma.ccr1
            .write(|w| w.bits((1 << 5) | (1 << 7) | (0b01 << 8) | (0b01 << 10)));
        dma.ccr1.modify(|r, w| w.bits(r.bits() | 1));
    }

    // CFGR: DMAEN|DMACFG (bits 0,1 — circular DMA for the regular
    // group), OVRMOD=1 (bit 12 — overwrite DR so a slow reader never
    // stalls the free-run stream), CONT=1 (bit 13 — regular converts
    // back-to-back with no trigger), JQDIS=1 (bit 31 — plain JSQR
    // injected mode, no queue; hardware-triggered injected still
    // launches on JEXTEN). Regular EXTEN stays 0 (free-run).
    adc.cfgr
        .write(|w| unsafe { w.bits((1 << 0) | (1 << 1) | (1 << 12) | (1 << 13) | (1 << 31)) });

    // Arm the INJECTED group only. JADSTART arms it — a
    // hardware-triggered injected group (JEXTEN != 0) still needs
    // JADSTART set once, exactly as the regular group needs ADSTART;
    // it then stays armed and converts on each TRGO2 falling edge
    // until JADSTP. (Omitting it left JDR at 0 — vbat/A/B read 0.)
    //
    // The REGULAR free-run current oversample is deliberately left
    // STOPPED. During normal lock the ADC then does ONLY the injected
    // mid-ON burst — identical activity to the proven pre-hybrid build
    // — so the free-run's continuous conversions + once-per-cycle
    // injected preemption can't add jitter to the control path NOR
    // bias the injected ch8 read. (Measured: with the free-run ON, the
    // injected current sat +13 counts / +300 mA vs the pre-hybrid
    // control at idle; with it OFF the injected current matches the
    // control raw ~1. The free-run<->injected ch8 interaction was
    // real.) Enable the microscope on demand with [`oversample_start`]
    // (the `G` key / an armed >4 A trigger) and stop it after the dump.
    adc.cr.modify(|_, w| w.jadstart().set_bit());
}

/// Enable the free-run current oversample: reset the ring and ADSTART
/// the (continuous) regular group. Injected control sampling is
/// unaffected. Call before a `G` dump (after a short fill delay) or
/// when arming the >4 A auto-trigger.
pub fn oversample_start() {
    let adc = unsafe { &*ADC1::ptr() };
    let dma = unsafe { &*stm32::DMA1::ptr() };
    if adc.cr.read().adstart().bit_is_set() {
        return;
    }
    unsafe {
        dma.ccr1.modify(|r, w| w.bits(r.bits() & !1));
        dma.cndtr1.write(|w| w.bits(CUR_FRAMES as u32));
        dma.ccr1.modify(|r, w| w.bits(r.bits() | 1));
    }
    adc.cr.modify(|_, w| w.adstart().set_bit());
}

/// Stop the free-run current oversample (ADSTP the regular group).
/// Injected control sampling continues. Returns the ADC to the
/// inject-only, pre-hybrid-equivalent state.
pub fn oversample_stop() {
    let adc = unsafe { &*ADC1::ptr() };
    if adc.cr.read().adstart().bit_is_set() {
        adc.cr.modify(|_, w| w.adstp().set_bit());
        // Bounded: ADSTP settles in µs; ISR-reachable via the WAX
        // trigger machinery — an unbounded wait here is a wedge.
        crate::spin::spin_until(100_000, || adc.cr.read().adstart().bit_is_clear());
    }
}

/// Newest completed injected burst: `(phase_a, phase_b, current,
/// vbat)` from JDR1-4, all mid-ON at `SAMPLE_TICKS`. Intended caller:
/// `TIM1_UP_TIM16` at the cycle wrap — this cycle's injected burst
/// (triggered ~1.25 µs in) finished tens of µs earlier. Reproduces
/// the old `last_frame()` `(A, B, current)` deterministically and
/// adds vbat.
#[inline]
pub fn inj_read() -> (u16, u16, u16, u16) {
    let adc = unsafe { &*ADC1::ptr() };
    (
        (adc.jdr1.read().bits() & 0x0FFF) as u16,
        (adc.jdr2.read().bits() & 0x0FFF) as u16,
        (adc.jdr3.read().bits() & 0x0FFF) as u16,
        (adc.jdr4.read().bits() & 0x0FFF) as u16,
    )
}

/// The `minz_core::am32_hal::InjAdc` register impl over the ADC1
/// injected group — zero-sized, static dispatch; delegates to
/// [`inj_read`], unchanged.
pub struct InjAdc1;

impl minz_core::am32_hal::InjAdc for InjAdc1 {
    #[inline(always)]
    fn inj_read(&self) -> (u16, u16, u16, u16) {
        inj_read()
    }
}

/// Latest current sample from the free-run ring's newest word — the
/// diagnostic mirror of the injected current. Cheap live read; used
/// as a coarse per-cycle spike detector for the analog black-box
/// trigger. (Control still uses the deterministic injected current
/// from [`inj_read`].)
#[inline]
pub fn last_raw() -> u16 {
    let head = cur_head();
    cur_word((head + CUR_FRAMES - 1) % CUR_FRAMES)
}

/// Freeze the free-run current capture for a consistent dump: stop
/// regular conversions (injected control sampling continues), disable
/// the DMA channel, return the ring's oldest-sample index (= next
/// write slot). Restart with [`resume_current`]. While frozen,
/// [`last_raw`] goes stale — keep dumps short.
pub fn freeze_current() -> usize {
    let adc = unsafe { &*ADC1::ptr() };
    let dma = unsafe { &*stm32::DMA1::ptr() };
    if adc.cr.read().adstart().bit_is_set() {
        adc.cr.modify(|_, w| w.adstp().set_bit());
        // Bounded: called from ISR context at the WAX trigger fire.
        crate::spin::spin_until(100_000, || adc.cr.read().adstart().bit_is_clear());
    }
    let remaining = dma.cndtr1.read().bits() as usize;
    dma.ccr1.modify(|r, w| unsafe { w.bits(r.bits() & !1) });
    // +1: ADSTP may abort mid-word — skip the partial slot.
    ((CUR_FRAMES - remaining) + 1) % CUR_FRAMES
}

/// Restart after [`freeze_current`]: reset the ring to slot 0 and
/// re-arm the regular free-run.
pub fn resume_current() {
    let adc = unsafe { &*ADC1::ptr() };
    let dma = unsafe { &*stm32::DMA1::ptr() };
    unsafe {
        dma.cndtr1.write(|w| w.bits(CUR_FRAMES as u32));
        dma.ccr1.modify(|r, w| w.bits(r.bits() | 1));
    }
    adc.cr.modify(|_, w| w.adstart().set_bit());
}
