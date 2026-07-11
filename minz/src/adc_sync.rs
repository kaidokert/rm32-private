//! PWM-synchronous continuous current sampling (GECKO, current half).
//!
//! TIM1 already generates OC4REF (PWM mode 1, preloaded CCR4). This
//! module routes OC4REF to TRGO2 (`CR2.MMS2 = 0b0111`) and arms ADC1
//! to convert regular channel 8 (PA3 = INA180 current-sense output)
//! on every **falling** edge of OC4REF — i.e. at `CNT == CCR4` ticks
//! into each 24 kHz PWM cycle, inside the high-side ON window where
//! battery-shunt current equals motor current. No DMA and no new
//! IRQs: the `TIM1_UP_TIM16` ISR (already firing once per PWM cycle)
//! reads `DR` at the cycle wrap, ~35 µs after the conversion landed.
//!
//! Battery voltage (channel 11, PA6) moves to the **injected** group:
//! software-triggered on demand, hardware-inserted between regular
//! conversions, so it never fights the trigger-armed regular channel.
//!
//! After [`start`] runs, the HAL's `OneShot::read` (`SenseAdc::
//! isns_raw` / `vbat_raw`) must NOT be called — it rewrites SQR/CFGR
//! under the armed trigger. `SenseAdc` remains useful for its
//! power-up/calibration side effects and `adc_to_mv`.

use core::sync::atomic::{AtomicU16, Ordering};

use crate::hal::stm32;
use crate::hal::stm32::{ADC1, TIM1};

/// Default sample point within the PWM cycle, TIM1 ticks (80 MHz).
/// 100 ticks = 1.25 µs (AM32's own CCR4 value): the full 3-channel
/// sequence (ch9 @1.25-1.9 µs, ch10 @2.0-2.6 µs, ch8 @2.6-3.3 µs at
/// the 80 MHz ADC clock) must finish INSIDE the high-side ON window,
/// or the late channels sample low-side recirculation (~0 V on every
/// terminal). ON = duty ≥ 333 ticks = 4.2 µs at amp 10 — whole
/// sequence fits with margin down to amp ≈ 8. Also a hard FLOOR on
/// the trigger (48 kHz lesson): below ~1.0 µs the first channel
/// samples during dead-time + FET turn-on and reads 0. Current
/// deliberately converts LAST: at ~2.6 µs the INA180 (350 kHz) is
/// well settled.
pub const SAMPLE_TICKS: u16 = 100;

/// WAXWING waveform ring: frames of `[ch9(A), ch10(B), ch8(current)]`
/// u16 triplets, filled circularly by DMA1_CH1 at one frame per PWM
/// cycle. 2048 frames ≈ 85 ms of history, 12 KiB. Atomics so DMA's
/// hardware writes and main-context dump reads never alias a `&mut`
/// (same pattern as the example's `PWM_SAMPLE_BUF`); reads are only
/// meaningful while the capture is frozen.
pub const WAX_FRAMES: usize = 2048;
pub const WAX_CHANS: usize = 3;
pub const WAX_WORDS: usize = WAX_FRAMES * WAX_CHANS;
static WAX_RING: [AtomicU16; WAX_WORDS] = [const { AtomicU16::new(0) }; WAX_WORDS];

/// Read one ring word (frozen capture only — see [`freeze_waveform`]).
#[inline]
pub fn wax_word(i: usize) -> u16 {
    WAX_RING[i].load(Ordering::Relaxed)
}

/// Route OC4REF→TRGO2 and start hardware-triggered conversions of the
/// 3-channel sequence ch9 (PA4, phase A), ch10 (PA5, phase B), ch8
/// (current) — **current last** so `last_raw()` at the cycle wrap
/// still returns the current sample. DMA1_CH1 drains the sequence
/// into the waveform ring circularly. Call after TIM1 init and after
/// the HAL `ADC::new` power-up/calibration (`SenseAdc::new`).
pub fn start(sample_ticks: u16) {
    let tim1 = unsafe { &*TIM1::ptr() };
    let adc = unsafe { &*ADC1::ptr() };

    // TIM1: sample point + OC4REF as TRGO2. CCR4 is preloaded (OC4PE
    // set at init) — takes effect at the next update event.
    tim1.ccr4.write(|w| w.ccr().bits(sample_ticks));
    // CR2.MMS2[23:20] = 0b0111 (OC4REF → TRGO2). The stm32l4 PAC has
    // no `mms2` accessor (SVD hole) — raw bits, preserving the rest.
    tim1.cr2
        .modify(|r, w| unsafe { w.bits((r.bits() & !(0xF << 20)) | (0b0111 << 20)) });

    // The HAL powered and calibrated the ADC but its enable state
    // depends on call history — make it enabled, idempotently
    // (RM0394 16.4.9 sequence, same as the HAL's `enable`).
    if adc.cr.read().aden().bit_is_clear() {
        while adc.cr.read().addis().bit_is_set() {}
        adc.isr.write(|w| w.adrdy().set_bit());
        adc.cr.modify(|_, w| w.aden().set_bit());
        while adc.isr.read().adrdy().bit_is_clear() {}
    }
    // CFGR/SQR/SMPR writes below require no conversion in flight.
    if adc.cr.read().adstart().bit_is_set() {
        adc.cr.modify(|_, w| w.adstp().set_bit());
        while adc.cr.read().adstart().bit_is_set() {}
    }

    // Sample times: 47.5 cycles on the phase channels (ch9/ch10) —
    // REQUIRED with the 3.3 kΩ divider sources; shorter settings
    // blind sector 2, whose confirm rule has razor margins by
    // construction (2×A ≈ vbus at the ZC). Proven by A/B at both
    // carriers. ch8 (current) is the INA180's low-impedance op-amp
    // output — 12.5 cycles suffices, and the saved 437 ns is what
    // lets the whole sequence (ends ~3.06 µs) fit a 48 kHz ON window
    // from amp ≈ 15. ch11 keeps 640.5 for the on-demand vbat read.
    adc.smpr1
        .modify(|_, w| unsafe { w.smp8().bits(0b010).smp9().bits(0b100) });
    adc.smpr2
        .modify(|_, w| unsafe { w.smp10().bits(0b100).smp11().bits(0b111) });

    // Regular group: ch9, ch10, ch8 per trigger (L=2 → 3 conversions).
    adc.sqr1
        .write(|w| unsafe { w.l().bits(2).sq1().bits(9).sq2().bits(10).sq3().bits(8) });
    // Injected group: single ch11, software trigger (JQDIS below).
    adc.jsqr
        .write(|w| unsafe { w.jl().bits(0).jsq1().bits(11) });

    // DMA1_CH1 ← ADC1 (CSELR C1S = 0000): circular, 16-bit both
    // sides, memory-increment, draining the 3-word sequence into the
    // waveform ring. Same CCR recipe as rm32's L431 ADC DMA.
    unsafe {
        (*stm32::RCC::ptr())
            .ahb1enr
            .modify(|_, w| w.dma1en().set_bit());
        let dma = &*stm32::DMA1::ptr();
        dma.cselr.modify(|r, w| w.bits(r.bits() & !0xF));
        dma.ccr1.write(|w| w.bits(0));
        dma.cpar1.write(|w| w.bits(adc.dr.as_ptr() as u32));
        dma.cmar1.write(|w| w.bits(WAX_RING.as_ptr() as u32));
        dma.cndtr1.write(|w| w.bits(WAX_WORDS as u32));
        // MINC | CIRC | PSIZE=16 | MSIZE=16, then EN.
        dma.ccr1
            .write(|w| w.bits((1 << 5) | (1 << 7) | (0b01 << 8) | (0b01 << 10)));
        dma.ccr1.modify(|r, w| w.bits(r.bits() | 1));
    }

    // EXTSEL 0b1010 = EXT10 = TIM1_TRGO2 (RM0394 ADC1 trigger table),
    // EXTEN 0b10 = falling edge of OC4REF = CNT reaching CCR4.
    // OVRMOD=1: overwrite DR on overrun so a slow reader can never
    // stall the conversion stream. JQDIS=1 (bit 31, no PAC accessor):
    // plain JSQR-register injected mode, no queue semantics.
    // DMAEN|DMACFG (bits 0,1): circular DMA for the regular sequence.
    adc.cfgr.write(|w| unsafe {
        w.bits((1 << 0) | (1 << 1) | (0b1010 << 6) | (0b10 << 10) | (1 << 12) | (1 << 31))
    });

    // Arm: with EXTEN != 0 this waits for triggers rather than
    // converting immediately; stays armed until ADSTP.
    adc.cr.modify(|_, w| w.adstart().set_bit());
}

/// Freeze the waveform capture for a consistent dump: stop regular
/// conversions (injected vbat reads still work), disable the DMA
/// channel, and return the ring's **oldest-frame index** (= next
/// write slot). Restart with [`resume_waveform`].
///
/// While frozen, `last_raw()` goes stale (current stats and the
/// overcurrent trip see a constant) — keep dumps short.
pub fn freeze_waveform() -> usize {
    let adc = unsafe { &*ADC1::ptr() };
    let dma = unsafe { &*stm32::DMA1::ptr() };
    if adc.cr.read().adstart().bit_is_set() {
        adc.cr.modify(|_, w| w.adstp().set_bit());
        while adc.cr.read().adstart().bit_is_set() {}
    }
    let remaining = dma.cndtr1.read().bits() as usize;
    dma.ccr1.modify(|r, w| unsafe { w.bits(r.bits() & !1) });
    // +1: if ADSTP aborted mid-sequence the head frame is partially
    // overwritten — skip it so the dump starts on a whole frame.
    ((WAX_WORDS - remaining) / WAX_CHANS + 1) % WAX_FRAMES
}

/// Restart after [`freeze_waveform`]: reset the ring to slot 0 (the
/// ADC always restarts at SQ1, so frame/channel alignment is exact)
/// and re-arm the trigger.
pub fn resume_waveform() {
    let adc = unsafe { &*ADC1::ptr() };
    let dma = unsafe { &*stm32::DMA1::ptr() };
    unsafe {
        dma.cndtr1.write(|w| w.bits(WAX_WORDS as u32));
        dma.ccr1.modify(|r, w| w.bits(r.bits() | 1));
    }
    adc.cr.modify(|_, w| w.adstart().set_bit());
}

/// Latest completed current sample, read from the DMA ring — NOT
/// from `ADC.DR`. A CPU read of DR races the DMA request: when the
/// CPU wins, DMA misses that word and every subsequent ring word
/// shifts one channel (seen as A/B/current traces swapping mid-
/// capture). With DMA enabled, DR is hands-off; the ring's newest
/// complete frame carries the current in its third word.
///
/// Intended caller: the `TIM1_UP_TIM16` ISR at the cycle wrap — this
/// cycle's frame finished converting ~20 µs earlier. While the
/// capture is frozen for a dump it returns a recent stale value
/// (current stats and the overcurrent trip idle for ~110 ms).
#[inline]
pub fn last_raw() -> u16 {
    last_frame().2
}

/// Newest complete `(phase_a, phase_b, current)` frame from the DMA
/// ring — FALCON v3's confirmation source: the mid-ON phase samples
/// let TIM1_UP compute the floating phase's sign vs the driven-pair
/// neutral, which the discriminator probe showed is the only clean
/// post-ZC confirmation (0 % premature vs 25-73 % for the wrap-
/// sampled COMP bit).
#[inline]
pub fn last_frame() -> (u16, u16, u16) {
    let dma = unsafe { &*stm32::DMA1::ptr() };
    let remaining = dma.cndtr1.read().bits() as usize;
    let words = WAX_WORDS - remaining;
    let frame = (words / WAX_CHANS + WAX_FRAMES - 1) % WAX_FRAMES;
    let base = frame * WAX_CHANS;
    (wax_word(base), wax_word(base + 1), wax_word(base + 2))
}

/// On-demand vbat read via the injected group (~8.2 µs blocking at
/// the 640.5-cycle sample time). Safe while regular conversions are
/// trigger-armed — the hardware inserts the injected conversion after
/// any in-flight regular one.
pub fn read_vbat_injected() -> u16 {
    let adc = unsafe { &*ADC1::ptr() };
    adc.isr.write(|w| w.jeos().set_bit());
    adc.cr.modify(|_, w| w.jadstart().set_bit());
    while adc.isr.read().jeos().bit_is_clear() {}
    adc.isr.write(|w| w.jeos().set_bit());
    (adc.jdr1.read().bits() & 0x0FFF) as u16
}

/// Non-blocking injected-vbat pump for a periodic ISR: harvest the
/// previous conversion if complete (returns `Some(raw)`), then start
/// the next. Call every tick; the 8.2 µs conversion easily finishes
/// between 166 µs TIM7 ticks. Built for the firmware SAG KILL — the
/// 2026-07-10 burnt motor happened because supply-sag detection
/// lived only in a host script that looked once per ladder rung.
pub fn vbat_pump() -> Option<u16> {
    let adc = unsafe { &*ADC1::ptr() };
    let out = if adc.isr.read().jeos().bit_is_set() {
        adc.isr.write(|w| w.jeos().set_bit());
        Some((adc.jdr1.read().bits() & 0x0FFF) as u16)
    } else {
        None
    };
    adc.cr.modify(|_, w| w.jadstart().set_bit());
    out
}
