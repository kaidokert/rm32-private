//! The ADC in hardware-triggered scan mode, drained by DMA1 channel 1 (step
//! 2d of the `hw/` move, E114). The DMA root hands each scan to
//! `shared::SHARED.scan`, which publishes it under its seqlock (E116).
//!
//! **Why the PAC and not the HAL** (moved from the binary; measured, not
//! theoretical): the HAL's `Adc::read` is a whole transaction per channel
//! (power up, reconfigure, convert, power down), which overran the run
//! guard's 200 µs tick at five calls per scan; its only `CHSELR` writer is that
//! single-channel read, so scan mode is unreachable; and `Adc::calibrate` spins
//! unbounded -- on this bench it wedged with `ADC_CR = 0x90000000` before the
//! watchdog was running. Its `DmaMode` trait lives on that same `Adc`
//! (`ref/stm32g0xx-hal/src/analog/adc.rs:414`), which is never constructed.

use core::cell::UnsafeCell;

use stm32g0xx_hal::rcc::{Enable, Rcc, Reset};
use stm32g0xx_hal::stm32;
use stm32g0xx_hal::stm32::Interrupt;

use super::nvic;
use crate::protection::RawScan;

/// One converted channel. The hardware walks the `CHSELR0` bitmask in
/// **ascending channel order**, so a result's position is where its channel
/// number sorts; positions are derived from this table, never written down.
#[derive(Copy, Clone)]
struct AdcChannel {
    ch: u8,
}

/// Every channel converted, ascending: the three shunt amplifiers (IN0, IN1,
/// IN4), the bus divider (IN6) and VREFINT (IN13). IN2/IN3 (the comparator's
/// own input pins) are deliberately not scanned: the sample-and-hold injected
/// charge onto them (notebook E041/E042).
const CHANNELS: [AdcChannel; 5] = [
    AdcChannel { ch: 0 },
    AdcChannel { ch: 1 },
    AdcChannel { ch: 4 },
    AdcChannel { ch: 6 },
    AdcChannel { ch: 13 },
];

/// Words per scan.
///
/// ENV-81 (campaign C) `dual-shunt`: the fully configurable sequencer converts
/// IA, IB, IC, VBUS, VREF, IA, IB, IC per trigger. The first five ranks are
/// today's ascending scan (same timing for bus and VREF); the shunts are
/// converted a second time ~14 us later and each shunt word is the mean of its
/// two samples, so the 12-bit scale and zero are unchanged.
#[cfg(not(feature = "dual-shunt"))]
pub const SCAN_LEN: usize = CHANNELS.len();
#[cfg(feature = "dual-shunt")]
pub const SCAN_LEN: usize = 8;

/// The `dual-shunt` sequence, IA IB IC VBUS VREF IA IB IC, packed four bits per rank
/// (SQ1 in bits 3:0), derived from the channel numbers so it cannot drift from them.
#[cfg(feature = "dual-shunt")]
const CHSELR1_DUAL: u32 = {
    let seq = [0u32, 1, 4, 6, 13, 0, 1, 4];
    let mut v = 0u32;
    let mut k = 0;
    while k < 8 {
        v |= seq[k] << (4 * k);
        k += 1;
    }
    v
};
#[cfg(feature = "dual-shunt")]
const _: () = assert!(CHSELR1_DUAL == 0x410D_6410);

const fn scan_index(ch: u8) -> usize {
    let mut i = 0;
    while i < CHANNELS.len() {
        if CHANNELS[i].ch == ch {
            return i;
        }
        i += 1;
    }
    panic!("channel not in CHANNELS");
}

const IX_ISENA: usize = scan_index(0);
const IX_ISENB: usize = scan_index(1);
const IX_ISENC: usize = scan_index(4);
const IX_VBUS: usize = scan_index(6);
const IX_VREF: usize = scan_index(13);

/// The channel mask, derived from the table.
#[cfg_attr(feature = "dual-shunt", allow(dead_code))]
const CH_MASK: u32 = {
    let mut m = 0u32;
    let mut i = 0;
    while i < CHANNELS.len() {
        m |= 1 << CHANNELS[i].ch;
        i += 1;
    }
    m
};

const _: () = {
    let mut i = 1;
    while i < CHANNELS.len() {
        assert!(CHANNELS[i - 1].ch < CHANNELS[i].ch, "CHANNELS must be ascending");
        i += 1;
    }
};

/// Spin budget for one register flag: generous against a ~5 µs conversion,
/// and bounded, which is the point.
const SPIN_LIMIT: u32 = 20_000;
/// DMA priority for the DMA root: a peer of COMP/COM; it copies five words.
const DMA_IRQ_PRIORITY: u8 = 0x40;

/// The DMA engine's target: hardware memory, not program state. It is
/// written only by the DMA engine, during a conversion sequence that starts on
/// the next TIM6 trigger (~88 µs after the transfer-complete interrupt), and
/// read only by the DMA root, with volatile reads, right after that interrupt.
struct DmaCell(UnsafeCell<[u16; SCAN_LEN]>);
// SAFETY: see the discipline above; the CPU never writes it.
unsafe impl Sync for DmaCell {}

static DMA_BUF: DmaCell = DmaCell(UnsafeCell::new([0; SCAN_LEN]));

#[inline(always)]
fn adc() -> &'static stm32::adc::RegisterBlock {
    // SAFETY: ADC's block from the PAC's pointer constant; configured by the
    // foreground only while conversions are stopped.
    unsafe { &*stm32::ADC::ptr() }
}

#[inline(always)]
fn dma() -> &'static stm32::dma1::RegisterBlock {
    // SAFETY: DMA1's block from the PAC's pointer constant; channel 1 is this
    // module's alone.
    unsafe { &*stm32::DMA1::ptr() }
}

/// Crude bounded delay for the pre-timebase part of init.
#[inline(never)]
fn spin(n: u32) {
    let mut i = 0u32;
    while i < n {
        cortex_m::asm::nop();
        i += 1;
    }
}

/// Bring the ADC up in scan mode with bounded waits only (RM0444 order:
/// regulator on and settled, calibrate with ADEN = 0, configure, enable, then
/// latch the channel mask). `false` on any timeout: no feedback, no run.
pub fn init(rcc: &mut Rcc) -> bool {
    stm32::ADC::enable(rcc);
    stm32::ADC::reset(rcc);
    let a = adc();
    // PCLK/2 = 32 MHz, inside the 35 MHz maximum; CKMODE is write-protected
    // once ADEN is set, so it goes first.
    a.cfgr2().modify(|_, w| w.ckmode().pclk_div2());
    // Internal reference on: channel 13 reads noise without it.
    a.ccr().modify(|_, w| w.vrefen().set_bit());
    // Regulator up, then ~20 µs to settle (a counted spin: no timer yet).
    a.cr().modify(|_, w| w.advregen().set_bit());
    spin(4_000);
    if a.cr().read().aden().bit_is_set() {
        return false;
    }
    a.cr().modify(|_, w| w.adcal().set_bit());
    if !wait(|| a.cr().read().adcal().bit_is_clear()) {
        return false;
    }
    // 12-bit, right-aligned, single-shot, no DMA yet.
    a.cfgr1().modify(|_, w| {
        w.res()
            .bits12()
            .align()
            .clear_bit()
            .cont()
            .clear_bit()
            .dmaen()
            .clear_bit()
    });
    a.smpr().modify(|_, w| w.smp1().cycles79_5());
    // ENV-78 (campaign C) DIAGNOSTIC `adc-ovs4`: hardware 4x oversampling (shift 2,
    // so the 12-bit scale is unchanged), the three shunts at 160.5 cycles (SMP2):
    // 4 x (160.5 + 12.5) / 32 MHz = 21.6 us each, just over one 20.8 us PWM period,
    // so a shunt word is a PWM-period average instead of an instantaneous sample.
    // Bus and VREF stay at 79.5 cycles (4 x 2.875 = 11.5 us). Whole scan ~88 us
    // inside the 101 us trigger period. Must precede ADEN (CFGR2 is write-locked).
    #[cfg(feature = "adc-ovs4")]
    {
        a.cfgr2()
            .modify(|_, w| w.ovse().set_bit().ovsr().mul4().ovss().shift2().tovs().clear_bit());
        a.smpr().modify(|_, w| {
            w.smp2()
                .cycles160_5()
                .smpsel(0)
                .set_bit()
                .smpsel(1)
                .set_bit()
                .smpsel(4)
                .set_bit()
        });
    }
    a.cr().modify(|_, w| w.aden().set_bit());
    if !wait(|| a.isr().read().adrdy().bit_is_set()) {
        return false;
    }
    // The channel selection must be acknowledged before the first start. One
    // named `chsel(n)` bit per channel in the table.
    #[cfg(not(feature = "dual-shunt"))]
    {
        a.chselr0().write(|w| {
            let mut i = 0;
            while i < CHANNELS.len() {
                w.chsel(CHANNELS[i].ch).set_bit();
                i += 1;
            }
            w
        });
        debug_assert_eq!(a.chselr0().read().bits(), CH_MASK);
    }
    #[cfg(feature = "dual-shunt")]
    {
        // CHSELRMOD with ADSTART = 0, then wait for and clear CCRDY *before* writing the
        // sequence: RM0444 says a CHSELR write issued before CCRDY is ignored, and an ignored
        // write would leave the reset sequence (channel 0 in every rank) -- the sag guard
        // would then judge a shunt as the bus (C2 review #4).
        a.cfgr1().modify(|_, w| w.chselrmod().set_bit());
        if !wait(|| a.isr().read().ccrdy().bit_is_set()) {
            return false;
        }
        a.isr().write(|w| w.ccrdy().clear_bit_by_one());
        // SQ1..SQ8 = 0, 1, 4, 6, 13, 0, 1, 4 (4-bit fields, SQ1 in bits 3:0; RM0444 ADC_CHSELR
        // in CHSELRMOD = 1): IA, IB, IC, VBUS, VREF, then the three shunts again.
        // SAFETY: a documented register layout; every nibble is one of this board's channels.
        a.chselr1().write(|w| unsafe { w.bits(CHSELR1_DUAL) });
        if !wait(|| a.isr().read().ccrdy().bit_is_set()) {
            return false;
        }
        // Read back: a sequence that did not take means no feedback, so no run.
        a.chselr1().read().bits() == CHSELR1_DUAL
    }
    #[cfg(not(feature = "dual-shunt"))]
    {
        wait(|| a.isr().read().ccrdy().bit_is_set())
    }
}

fn wait(done: impl Fn() -> bool) -> bool {
    let mut n = 0u32;
    while !done() {
        n += 1;
        if n > SPIN_LIMIT {
            return false;
        }
    }
    true
}

/// Arm the hardware-triggered, DMA-drained scan (after `init`, with TIM6
/// running): DMAMUX request 5 -> DMA1 CH1, circular, 16-bit, high priority,
/// transfer-complete and error interrupts; ADC triggered by TIM6 TRGO.
pub fn dma_start(rcc: &mut Rcc) {
    stm32::DMA1::enable(rcc);
    // SAFETY: DMAMUX's block from the PAC's pointer constant; channel 0 only.
    let mux = unsafe { &*stm32::DMAMUX::ptr() };
    // SAFETY: DMAREQ_ID 5 is ADC (RM0444 DMAMUX request table); the PAC
    // leaves this field's writer unsafe because not every value is a request.
    mux.ccr(0).modify(|_, w| unsafe { w.dmareq_id().bits(5) });
    let a = adc();
    let ch = dma().ch1();
    ch.cr().modify(|_, w| w.en().clear_bit());
    // PAR/MAR take raw bus addresses (the PAC marks them unsafe).
    ch.par().write(|w| {
        // SAFETY: the peripheral end is ADC_DR, read-only data for the DMA.
        unsafe { w.pa().bits(a.dr().as_ptr() as u32) }
    });
    ch.mar().write(|w| {
        // SAFETY: `DMA_BUF` is a 'static buffer of exactly NDTR half-words
        // that only the DMA writes (see `DmaCell`).
        unsafe { w.ma().bits(DMA_BUF.0.get() as u32) }
    });
    ch.ndtr().write(|w| w.ndt().set(SCAN_LEN as u16));
    ch.cr().write(|w| {
        w.tcie().set_bit().teie().set_bit().circ().set_bit().minc().set_bit();
        w.psize().bits16().msize().bits16().pl().high()
    });
    ch.cr().modify(|_, w| w.en().set_bit());
    // Writable only with ADSTART = 0, which `init` leaves.
    // EXTSEL 0b101 is TIM6_TRGO (RM0444 ADC external triggers). This PAC's
    // EXTSEL enum is incomplete and labels value 5 Tim2Ch3, so the field is
    // written by value rather than through a mislabelled variant.
    a.cfgr1().modify(|_, w| {
        // SAFETY: a documented EXTSEL encoding (see above).
        unsafe { w.extsel().bits(0b101) };
        w.exten()
            .rising_edge()
            .dmacfg()
            .set_bit()
            .dmaen()
            .set_bit()
            .cont()
            .clear_bit()
    });
    super::pace::trgo_on_update();
    nvic::set_priority(Interrupt::DMA1_CHANNEL1, DMA_IRQ_PRIORITY);
    nvic::unmask(Interrupt::DMA1_CHANNEL1);
    // Conversions now start on each TIM6 trigger with no CPU involvement.
    a.cr().modify(|_, w| w.adstart().set_bit());
}

/// Re-align the scan and its DMA channel from a known state (E080): stop
/// conversions, reset the channel's count and flags, restart both.
pub fn resync() {
    let a = adc();
    if a.cr().read().adstart().bit_is_set() {
        a.cr().modify(|_, w| w.adstp().set_bit());
        let mut n = 0u32;
        while a.cr().read().adstp().bit_is_set() && n < 100_000 {
            n += 1;
        }
    }
    let d = dma();
    let ch = d.ch1();
    ch.cr().modify(|_, w| w.en().clear_bit());
    d.ifcr().write(|w| w.cgif1().set_bit());
    ch.ndtr().write(|w| w.ndt().set(SCAN_LEN as u16));
    ch.cr().modify(|_, w| w.en().set_bit());
    // Every ADC status flag is write-1-to-clear.
    a.isr().write(|w| {
        w.adrdy()
            .clear_bit_by_one()
            .eosmp()
            .clear_bit_by_one()
            .eoc()
            .clear_bit_by_one();
        w.eos()
            .clear_bit_by_one()
            .ovr()
            .clear_bit_by_one()
            .awd1()
            .clear_bit_by_one();
        w.awd2()
            .clear_bit_by_one()
            .awd3()
            .clear_bit_by_one()
            .eocal()
            .clear_bit_by_one();
        w.ccrdy().clear_bit_by_one()
    });
    a.cr().modify(|_, w| w.adstart().set_bit());
}

/// The DMA root's work: acknowledge every channel-1 flag (a handler that
/// clears only TCIF re-fires forever on a transfer error, `rm32/CLAUDE.md`),
/// then read the scan that just landed. Straight-line.
/// Did the ADC drop a conversion since the last call, clearing the flag?
///
/// **Why this is worth an ISR read** (Q60-1): DMA1 CH1 is circular with
/// `NDTR = SCAN_LEN`, so one dropped conversion rotates the buffer
/// *permanently* against [`scan_index`], and every later scan reads the wrong
/// channel in each slot -- a phase code lands in the `vref` slot, which passes
/// the only validation production performs. `resync_adc` runs only at arm, so
/// nothing recovers it mid-run, and nothing counted it. Straight-line: one
/// read, one test, one write-1-to-clear.
#[inline(always)]
#[must_use]
pub fn adc_overran() -> bool {
    let a = adc();
    if a.isr().read().ovr().bit_is_set() {
        a.isr().write(|w| w.ovr().clear_bit_by_one());
        true
    } else {
        false
    }
}

#[inline(always)]
#[must_use]
pub fn dma_isr() -> RawScan {
    dma().ifcr().write(|w| {
        w.cgif1()
            .set_bit()
            .ctcif1()
            .set_bit()
            .chtif1()
            .set_bit()
            .cteif1()
            .set_bit()
    });
    #[cfg(not(feature = "dual-shunt"))]
    let (phase_a, phase_b, phase_c) = (dma_word(IX_ISENA), dma_word(IX_ISENB), dma_word(IX_ISENC));
    #[cfg(feature = "dual-shunt")]
    let (phase_a, phase_b, phase_c) = (
        ((u32::from(dma_word(IX_ISENA)) + u32::from(dma_word(5)) + 1) >> 1) as u16,
        ((u32::from(dma_word(IX_ISENB)) + u32::from(dma_word(6)) + 1) >> 1) as u16,
        ((u32::from(dma_word(IX_ISENC)) + u32::from(dma_word(7)) + 1) >> 1) as u16,
    );
    RawScan {
        phase_a,
        phase_b,
        phase_c,
        bus: dma_word(IX_VBUS),
        vref: dma_word(IX_VREF),
    }
}

/// One word of the scan that just landed.
#[inline(always)]
fn dma_word(ix: usize) -> u16 {
    let p = DMA_BUF.0.get().cast::<u16>().wrapping_add(ix);
    // SAFETY: see `DmaCell`: `ix` is a compile-time position inside the
    // `SCAN_LEN`-word buffer, which the DMA does not rewrite until the next
    // TIM6 trigger, ~88 µs after the transfer-complete interrupt.
    unsafe { core::ptr::read_volatile(p) }
}

/// Gate-4 feedback-age stimulus: stop the DMA channel (the producer stalls;
/// `resync` restores it).
pub fn stall_dma() {
    dma().ch1().cr().modify(|_, w| w.en().clear_bit());
}

/// Factory VREFINT calibration. A calibration word in system memory, not a
/// peripheral register, so it has no PAC representation; the vendored HAL
/// reads the same address the same way and does not export it
/// (`ref/stm32g0xx-hal/src/analog/adc.rs:256-258`: "DS12766 3.13.2 ...
/// `ptr::read_volatile(0x1FFF_75AA as *const u16)`").
fn vrefint_cal() -> u32 {
    // SAFETY: a read-only factory word at the documented address.
    unsafe { core::ptr::read_volatile(0x1FFF_75AA as *const u16) as u32 }
}

/// VDDA in mV from a raw VREFINT code: `3000 * VREFINT_CAL / code`.
#[must_use]
pub fn vdda_mv(vref_raw: u16) -> u32 {
    if vref_raw == 0 {
        return 0;
    }
    3_000u32 * vrefint_cal() / vref_raw as u32
}
