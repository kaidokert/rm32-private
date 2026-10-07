//! G071 ADC: CH4 current, CH6 voltage, CH12 temp. DMA1_CH2 circular + DMAMUX.
//!
//! `cfg(rm32_three_shunt)` (board YAML `current_shunt_channels: [0, 1, 4]`,
//! the binz DRV8304H bench): sequence IN0, IN1, IN4, IN6, IN12 and a
//! DC-link current estimate from the sum of the three low-side shunt
//! amplifiers — see `three_shunt`.

use crate::adc_hal::AdcPeripheral;
use crate::pac::{ADC, DMA1, GPIOA, RCC};
use crate::regs::{InitError, wait_for};

#[cfg(not(rm32_three_shunt))]
crate::define_adc_boilerplate!(
    ops: G071AdcOps,
    type_name: AdcReader,
    cal1: 0x1FFF_75A8, cal2: 0x1FFF_75CA,
    cal1_temp: 30, cal2_temp: 130,
    cal_vref_mv: 3000,
);

pub struct G071AdcOps;

/// Regular sequence, CHSELRMOD = 1 nibble format, 0xF terminated.
#[cfg(not(rm32_three_shunt))]
const SEQUENCE: u32 = 4 | (6 << 4) | (12 << 8) | (0xF << 12); // I, V, temp
/// 3-shunt: IN0, IN1, IN4 (shunts), IN6 (bus), IN12 (temp).
#[cfg(rm32_three_shunt)]
const SEQUENCE: u32 = (1 << 4) | (4 << 8) | (6 << 12) | (12 << 16) | (0xF << 20);

/// Program the regular sequence with the RM0444 handshake: set CHSELRMOD
/// (ADSTART = 0), wait for and clear CCRDY, write CHSELR, wait CCRDY again,
/// verify by readback. A CHSELR write issued before CCRDY is IGNORED — the
/// previous unconditioned write left CHSELR = 0, i.e. eight conversions of
/// IN0, so "current", "voltage" and "temperature" all read shunt A
/// (caught by the binz register diff vs firmware50, which hit the same
/// rule). Fails loudly instead of running on a wrong sequence.
fn program_sequence() -> Result<(), InitError> {
    let adc = unsafe { &*ADC::ptr() };
    const CCRDY: u32 = 1 << 13;
    adc.cfgr1()
        .modify(|r, w| unsafe { w.bits(r.bits() | (1 << 21)) }); // CHSELRMOD
    wait_for(
        || unsafe { (&*ADC::ptr()).isr().read().bits() & CCRDY != 0 },
        100_000,
        "ADC CCRDY (mode)",
    )?;
    adc.isr().write(|w| unsafe { w.bits(CCRDY) });
    adc.chselr1().write(|w| unsafe { w.bits(SEQUENCE) });
    wait_for(
        || unsafe { (&*ADC::ptr()).isr().read().bits() & CCRDY != 0 },
        100_000,
        "ADC CCRDY (sequence)",
    )?;
    adc.isr().write(|w| unsafe { w.bits(CCRDY) });
    if adc.chselr1().read().bits() != SEQUENCE {
        return Err(InitError::Timeout("ADC CHSELR readback"));
    }
    Ok(())
}

/// ADC_CR.ADVREGEN — kept set on every CR write.
const ADVREGEN: u32 = 1 << 28;

impl AdcPeripheral for G071AdcOps {
    fn enable_clocks(&self) {
        let rcc = unsafe { &*RCC::ptr() };
        rcc.apbenr2()
            .modify(|r, w| unsafe { w.bits(r.bits() | (1 << 20)) }); // ADCEN
        rcc.ahbenr()
            .modify(|r, w| unsafe { w.bits(r.bits() | (1 << 0)) }); // DMA1EN
    }

    fn configure_pins(&self) {
        let gpioa = unsafe { &*GPIOA::ptr() };
        gpioa.moder().modify(|r, w| unsafe {
            w.bits(r.bits() | (0b11 << 8) | (0b11 << 12)) // PA4, PA6 analog
        });
        #[cfg(rm32_three_shunt)]
        gpioa.moder().modify(|r, w| unsafe {
            w.bits(r.bits() | 0b11 | (0b11 << 2)) // PA0, PA1 analog
        });
    }

    fn configure_clock_source(&self) {
        let adc = unsafe { &*ADC::ptr() };
        adc.cfgr2().write(|w| unsafe { w.bits(0b10 << 30) }); // CKMODE
    }

    fn enable_temp_sensor(&self) {
        let adc = unsafe { &*ADC::ptr() };
        adc.ccr()
            .modify(|r, w| unsafe { w.bits(r.bits() | (1 << 23)) }); // TSEN
    }

    fn configure_dma(&self, buf_ptr: *const u16, buf_len: u16) {
        let adc = unsafe { &*ADC::ptr() };
        let dma = unsafe { &*DMA1::ptr() };
        let dmamux = unsafe { &*crate::pac::DMAMUX::ptr() };

        dmamux
            .ccr(1)
            .modify(|r, w| unsafe { w.bits((r.bits() & !0x3F) | 5) });
        let ch = dma.ch2();
        ch.cr().write(|w| w.en().clear_bit());
        ch.par()
            .write(|w| unsafe { w.bits(adc.dr().as_ptr() as u32) });
        ch.mar().write(|w| unsafe { w.bits(buf_ptr as u32) });
        ch.ndtr().write(|w| unsafe { w.bits(buf_len as u32) });
        ch.cr().write(|w| unsafe {
            w.bits((1 << 1) | (1 << 5) | (1 << 7) | (0b10 << 8) | (0b01 << 10) | (0b10 << 12))
        });
        ch.cr().modify(|r, w| unsafe { w.bits(r.bits() | 1) });
    }

    fn configure_sampling(&self) {
        let adc = unsafe { &*ADC::ptr() };
        #[cfg(not(rm32_three_shunt))]
        adc.smpr()
            .write(|w| unsafe { w.bits(0b011 | (0b111 << 4)) });
        // 3-shunt (binz): SMP1 = 79.5 cycles on every channel, firmware50's
        // setting for these CSA outputs and the 11.94x bus divider.
        #[cfg(rm32_three_shunt)]
        adc.smpr().write(|w| unsafe { w.bits(0b110) });
    }

    /// Sequence programming is deferred to `enable()` (after ADRDY), with
    /// the RM0444 CCRDY handshake — see `program_sequence`.
    fn configure_sequence(&self) {}

    fn enable_dma_mode(&self) {
        let adc = unsafe { &*ADC::ptr() };
        adc.cfgr1().modify(|r, w| unsafe {
            w.bits((r.bits() & !0b11) | (0b01 << 0)) // DMAEN
        });
        adc.cfgr1()
            .modify(|r, w| unsafe { w.bits(r.bits() & !(0b11 << 3)) });
    }

    /// ADC voltage regulator: must be on (and settled, tADCVREG_STUP
    /// <= 20 µs) before calibration (RM0444 15.3.2; AM32
    /// LL_ADC_EnableInternalRegulator). Previously never enabled, and the
    /// CR *writes* below cleared it — ADCAL then never completed, init
    /// returned Err (ignored), and every reading stayed 0.
    fn power_up(&self) {
        let adc = unsafe { &*ADC::ptr() };
        adc.cr().write(|w| unsafe { w.bits(ADVREGEN) });
        cortex_m::asm::delay(64 * 25);
    }

    fn calibrate(&self) -> Result<(), InitError> {
        let adc = unsafe { &*ADC::ptr() };
        adc.cr().write(|w| unsafe { w.bits(ADVREGEN | (1 << 31)) }); // ADCAL
        wait_for(
            || unsafe { (&*ADC::ptr()).cr().read().bits() & (1 << 31) == 0 },
            100_000,
            "ADC cal",
        )?;
        cortex_m::asm::delay(64 * 20);
        Ok(())
    }

    fn enable(&self) -> Result<(), InitError> {
        let adc = unsafe { &*ADC::ptr() };
        adc.isr().write(|w| unsafe { w.bits(1 << 0) }); // clear ADRDY
        adc.cr().write(|w| unsafe { w.bits(ADVREGEN | (1 << 0)) }); // ADEN
        wait_for(
            || unsafe { (&*ADC::ptr()).isr().read().bits() & (1 << 0) != 0 },
            100_000,
            "ADC ready",
        )?;
        program_sequence()
    }

    fn start_conversion(&self) {
        let adc = unsafe { &*ADC::ptr() };
        adc.cr()
            .modify(|r, w| unsafe { w.bits(r.bits() | (1 << 2)) }); // ADSTART
    }
}

#[cfg(rm32_three_shunt)]
pub use three_shunt::{AdcReader, arm_hw_trigger, last_ms_sample, new_adc, post_init, tick_scan};

/// DC-link current from three low-side shunt amplifiers (DRV8304H: 7 mOhm,
/// gain 10, bidirectional around VREF/2).
///
/// Per conversion the three shunt codes are summed. Motoring current pulls
/// the amplifier outputs DOWN, so current is `zero - sum`, where `zero` is
/// the bridge-off sum captured at boot (`capture_zero`, ENABLE low). Over
/// a PWM period the sink shunt carries the phase current only while the
/// source high side conducts (the complementary off-interval cancels), so
/// the time-mean of the sum is the DC-link current.
///
/// Scale and offset are firmware50's metered calibration verbatim
/// (binz/firmware50/src/protection.rs, ENV-92: `RAW_LIMIT` = 4 A over 100
/// scans at an assumed VDDA of 3600 mV; meter = (fw + 116 mA) / 1.131),
/// so the two firmwares report the same mA for the same physical current.
#[cfg(rm32_three_shunt)]
mod three_shunt {
    use super::G071AdcOps;
    use crate::adc_hal::TempCalibration;
    use crate::dma_buf::DmaBuf;
    use core::sync::atomic::{AtomicI32, AtomicU32, Ordering::Relaxed};
    use rm32::hal::Adc;
    use rm32::units;

    static BUF: DmaBuf<u16, 5> = DmaBuf::new();
    /// Bridge-off shunt sum; 0 = not captured (current reads 0).
    static ZERO: AtomicU32 = AtomicU32::new(0);
    /// The zero in 1/16 counts (1 count = 11 mA metered); 0 = none.
    static ZERO_X16: AtomicU32 = AtomicU32::new(0);
    /// Track the zero (slow EWMA) while the bridge is armed and idle: the
    /// CSAs settle for several seconds after ENABLE (binz: idle drifted
    /// 137 -> 125 mA over 4-8 s after a 1.5 s zero). Main sets it.
    static TRACK: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);
    /// Bench aggregate: sum / count of per-scan metered mA since the last
    /// `take_mean_ma` (main-loop only; plain load/store, single writer).
    static ACC_MA: AtomicI32 = AtomicI32::new(0);
    static ACC_N: AtomicU32 = AtomicU32::new(0);
    /// Shunt-sum accumulator of the 20 kHz scans (TIM6 writes, the 1 kHz
    /// dispatch drains under a critical section).
    static SCAN_SUM: AtomicU32 = AtomicU32::new(0);
    static SCAN_N: AtomicU32 = AtomicU32::new(0);

    /// 20 kHz control tick: fold the finished scan into the accumulator.
    /// Conversions are hardware-triggered by TIM6 TRGO (`arm_hw_trigger`).
    ///
    /// One scan per 1 kHz dispatch aliased against the 24 kHz carrier:
    /// 64000 cycles mod ARR+1 (2666) = 16, so the sample instant crept
    /// through the PWM period over ~167 ms and sat in the high-side
    /// on-window for ~17 ms at a time, reading full phase current (~2 A
    /// at 10 % duty) for longer than the guard's 20 ms debounce (binz:
    /// OC kills at a steady 504 eHz with a 200 mA mean). At one scan per
    /// tick the instant steps 3200 mod 2666 = 534 cycles (0.2 period),
    /// so each 1 kHz mean spans four carrier periods evenly.
    ///
    /// Software starts from the tick ISR were biased: COMP/TIM14 preempt
    /// or delay TIM6, so scan instants clustered around commutations, and
    /// the hold reading moved 180 -> 263 mA between ISR-end and ISR-entry
    /// starts at the same speed. firmware50's TIM6-TRGO trigger samples at
    /// the timer update itself, independent of ISR activity.
    ///
    /// A fixed trigger rate still sampled a LATTICE of carrier phases:
    /// 3200 mod 1333 = 534 = 0.40 period, i.e. 5 phases drifting over
    /// ~83 ms. At 37.5 % duty each 1 ms mean held 1 or 2 of the 5 points in
    /// the high-side window (0.2 / 0.4 x phase current), a +/-50 % swing
    /// lasting tens of ms: three false OC kills at a steady, desync-free
    /// 37.5 % rung (binz climb). Now TIM15 triggers each scan with a period
    /// re-randomised every tick (LFSR, 2400..4447 cycles), so scan phases are
    /// uncorrelated with the carrier at any ARR (variable PWM included).
    #[inline]
    pub fn tick_scan() {
        let adc = unsafe { &*crate::pac::ADC::ptr() };
        if adc.isr().read().bits() & (1 << 3) != 0 {
            adc.isr().write(|w| unsafe { w.bits(1 << 3) }); // EOS: count once
            let s = shunt_sum(BUF.read());
            SCAN_SUM.store(SCAN_SUM.load(Relaxed).wrapping_add(s), Relaxed);
            SCAN_N.store(SCAN_N.load(Relaxed).wrapping_add(1), Relaxed);
        }
        // 16-bit Galois LFSR (taps 0xB400), single writer (this tick).
        let mut l = LFSR.load(Relaxed);
        l = (l >> 1) ^ (0u32.wrapping_sub(l & 1) & 0xB400);
        LFSR.store(l, Relaxed);
        let tim15 = unsafe { &*crate::pac::TIM15::ptr() };
        tim15.arr().write(|w| unsafe { w.bits(2400 + (l & 0x7FF)) }); // preloaded
    }

    static LFSR: AtomicU32 = AtomicU32::new(0xACE1);
    /// Latest per-ms metered mA (1 kHz dispatch) and its sequence number,
    /// for the bench split current guard.
    static LAST_MA: AtomicI32 = AtomicI32::new(0);
    static LAST_SEQ: AtomicU32 = AtomicU32::new(0);

    /// (sequence, mA) of the latest per-ms current sample.
    pub fn last_ms_sample() -> (u32, i32) {
        (LAST_SEQ.load(Relaxed), LAST_MA.load(Relaxed))
    }

    /// Hardware-trigger the scan from TIM15 TRGO (EXTSEL 0b100, RM0444;
    /// firmware50's method used TIM6 = 0b101), rising edge, ADC-side circular
    /// DMA (DMACFG) so every trigger's sequence lands in BUF, CONT off.
    /// TIM15's period is randomised per tick by `tick_scan`.
    /// Call after TIM6 is configured; CFGR1 is writable with ADSTART = 0.
    pub fn arm_hw_trigger() {
        let adc = unsafe { &*crate::pac::ADC::ptr() };
        let tim15 = unsafe { &*crate::pac::TIM15::ptr() };
        let rcc = unsafe { &*crate::pac::RCC::ptr() };
        rcc.apbenr2().modify(|_, w| w.tim15en().set_bit());
        // Stop any software-started sequence (ADSTP) and re-align the
        // circular DMA to slot 0 — a stray earlier conversion left the
        // index offset and BUF[3] read a shunt as the bus (19.7 V).
        if adc.cr().read().bits() & (1 << 2) != 0 {
            adc.cr()
                .modify(|r, w| unsafe { w.bits(r.bits() | (1 << 4)) });
            let _ = crate::regs::wait_for(
                || adc.cr().read().bits() & (1 << 2) == 0,
                100_000,
                "ADC stop",
            );
        }
        let ch = unsafe { &*crate::pac::DMA1::ptr() }.ch2();
        ch.cr().modify(|r, w| unsafe { w.bits(r.bits() & !1) });
        ch.ndtr().write(|w| unsafe { w.bits(5) });
        let _ = adc.dr().read().bits(); // drop a stale EOC
        ch.cr().modify(|r, w| unsafe { w.bits(r.bits() | 1) });
        adc.cfgr1().modify(|r, w| unsafe {
            let v = r.bits() & !((0b111 << 6) | (0b11 << 10) | (1 << 13));
            w.bits(v | (0b100 << 6) | (0b01 << 10) | (1 << 1) | 1)
        });
        // TIM15: 64 MHz, ARR preloaded (ARPE), TRGO on update (MMS 010).
        tim15.psc().write(|w| unsafe { w.bits(0) });
        tim15.arr().write(|w| unsafe { w.bits(3200) });
        tim15
            .cr2()
            .modify(|r, w| unsafe { w.bits((r.bits() & !(0b111 << 4)) | (0b010 << 4)) });
        tim15.egr().write(|w| w.ug().set_bit());
        tim15.cr1().write(|w| unsafe { w.bits((1 << 7) | 1) }); // ARPE | CEN
        adc.isr().write(|w| unsafe { w.bits(1 << 3) }); // clear EOS
        adc.cr()
            .modify(|r, w| unsafe { w.bits(r.bits() | (1 << 2)) }); // ADSTART: arm
    }

    /// firmware50 `RAW_LIMIT`: residual counts for 4 A over 100 scans.
    const RAW_LIMIT: i32 = 31_857;
    const METER_GAIN_X1000: i32 = 1_131;
    const METER_OFFSET_MA: i32 = 116;
    /// Encoding bias so `AdcCount::to_milliamps(1600, 80)` (board YAML
    /// current_offset 1600, millivolt_per_amp 80) returns metered mA,
    /// negative (regenerating) currents included.
    pub const RAW_BIAS_MA: i32 = 2_000;

    /// Residual in 1/16 counts -> metered mA (firmware50 ENV-92 fit:
    /// fw_mA = residual * 4000 * 100 / RAW_LIMIT, meter = (fw + 116) / 1.131;
    /// 12556 / 16 rounds to 785).
    /// |residual_x16| <= 16 * 3 * 4095, * 785 < 2^31.
    #[inline]
    pub fn metered_ma_x16(residual_x16: i32) -> i32 {
        (residual_x16 * ((400_000_000 / RAW_LIMIT + 8) / 16) + METER_OFFSET_MA * 1000)
            / METER_GAIN_X1000
    }

    pub struct AdcReader {
        temp_cal: TempCalibration,
    }

    fn temp_cal() -> TempCalibration {
        // SAFETY: G071 TS_CAL1/TS_CAL2 ROM addresses (RM0444).
        unsafe { TempCalibration::from_rom(0x1FFF_75A8, 0x1FFF_75CA, 30, 130, 3000) }
    }

    pub fn new_adc() -> AdcReader {
        AdcReader {
            temp_cal: temp_cal(),
        }
    }

    pub fn post_init() -> AdcReader {
        new_adc()
    }

    #[inline]
    fn shunt_sum(b: &[u16; 5]) -> u32 {
        b[0] as u32 + b[1] as u32 + b[2] as u32
    }

    impl AdcReader {
        pub fn init(&self) -> Result<(), crate::regs::InitError> {
            crate::adc_generic::GenericAdc::new(G071AdcOps, &BUF, temp_cal()).init()
        }

        /// Average `n` bridge-idle sequences into the zero reference. Call
        /// with the gate driver ENABLED (awake: the DRV8304 CSAs are off in
        /// sleep) and no drive applied. Returns 0 (and stores no zero) if
        /// the readings are not plausible amplifier idle levels.
        pub fn capture_zero(&mut self, n: u32) -> u32 {
            // The 20 kHz tick owns the conversions (`tick_scan`); average its
            // scans. Polling ADSTART here starved forever against the tick's
            // restarts and the IWDG reset the board (binz, arm -> reboot).
            let drain = || {
                cortex_m::interrupt::free(|_| {
                    let r = (SCAN_SUM.load(Relaxed), SCAN_N.load(Relaxed));
                    SCAN_SUM.store(0, Relaxed);
                    SCAN_N.store(0, Relaxed);
                    r
                })
            };
            let _ = drain();
            // n scans at 20 kHz; bounded at 4x that (~n * 200 us).
            for _ in 0..n * 4 {
                if SCAN_N.load(Relaxed) >= n {
                    break;
                }
                cortex_m::asm::delay(64 * 50);
            }
            let (acc, got) = drain();
            let zero = (acc + got / 2).checked_div(got).unwrap_or(0);
            let b = BUF.read();
            // Each amplifier idles at VREF/2 (~2048). Anything else means
            // the driver is asleep (CSAs unpowered) or drive is present —
            // refuse it rather than calibrate current against garbage.
            let sane = got >= n && b[..3].iter().all(|&v| (1600..=2500).contains(&v));
            ZERO.store(if sane { zero } else { 0 }, Relaxed);
            let z16 = (acc * 16 + got / 2).checked_div(got).unwrap_or(0);
            ZERO_X16.store(if sane { z16 } else { 0 }, Relaxed);
            crate::dprintln!(
                "[rm32] 3-shunt zero {}: sum={} {}/{} sequences, last buf {} {} {} {} {}",
                if sane { "OK" } else { "REJECTED" },
                zero,
                got,
                n,
                b[0],
                b[1],
                b[2],
                b[3],
                b[4]
            );
            if sane { zero } else { 0 }
        }

        /// Mean metered mA over the scans since the previous call, and the
        /// scan count. Bench report only.
        pub fn take_mean_ma(&self) -> (i32, u32) {
            let n = ACC_N.load(Relaxed);
            let s = ACC_MA.load(Relaxed);
            ACC_N.store(0, Relaxed);
            ACC_MA.store(0, Relaxed);
            if n == 0 { (0, 0) } else { (s / n as i32, n) }
        }

        pub fn zero(&self) -> u32 {
            ZERO.load(Relaxed)
        }

        /// Forget the zero (driver disabled: the next enable recalibrates).
        pub fn clear_zero(&self) {
            ZERO.store(0, Relaxed);
            ZERO_X16.store(0, Relaxed);
            TRACK.store(false, Relaxed);
        }

        /// Follow the zero while the bridge is armed and idle (no drive);
        /// frozen as soon as `on` drops (drive starts).
        pub fn set_zero_tracking(&self, on: bool) {
            TRACK.store(on, Relaxed);
        }

        /// Raw shunt codes of the last scan (IN0, IN1, IN4).
        pub fn shunts(&self) -> [u16; 3] {
            let b = BUF.read();
            [b[0], b[1], b[2]]
        }
    }

    impl Adc for AdcReader {
        fn start_conversion(&mut self) {
            use crate::adc_hal::AdcPeripheral;
            G071AdcOps.start_conversion();
        }

        /// Encoded metered mA (+ `RAW_BIAS_MA`); see module docs. Called
        /// once per 1 kHz ADC dispatch by the core. Uses the mean of the
        /// 20 kHz scans since the previous call (`tick_scan`).
        fn raw_current(&self) -> u16 {
            let (sum, n) = cortex_m::interrupt::free(|_| {
                let r = (SCAN_SUM.load(Relaxed), SCAN_N.load(Relaxed));
                SCAN_SUM.store(0, Relaxed);
                SCAN_N.store(0, Relaxed);
                r
            });
            let mut z16 = ZERO_X16.load(Relaxed) as i32;
            if z16 == 0 {
                return RAW_BIAS_MA as u16;
            }
            let scan16 = match (sum * 16 + n / 2).checked_div(n) {
                Some(v) => v as i32,
                None => (shunt_sum(BUF.read()) * 16) as i32,
            };
            if TRACK.load(Relaxed) {
                z16 += (scan16 - z16) / 64; // tau = 64 ms at 1 kHz
                ZERO_X16.store(z16 as u32, Relaxed);
                ZERO.store(((z16 + 8) >> 4) as u32, Relaxed);
            }
            let ma = metered_ma_x16(z16 - scan16);
            ACC_MA.store(ACC_MA.load(Relaxed).wrapping_add(ma), Relaxed);
            LAST_MA.store(ma, Relaxed);
            LAST_SEQ.store(LAST_SEQ.load(Relaxed).wrapping_add(1), Relaxed);
            ACC_N.store(ACC_N.load(Relaxed).wrapping_add(1), Relaxed);
            (ma + RAW_BIAS_MA).clamp(0, u16::MAX as i32) as u16
        }

        fn raw_voltage(&self) -> u16 {
            BUF.read()[3]
        }

        fn raw_temperature(&self) -> u16 {
            BUF.read()[4]
        }

        fn calc_temperature(&self, raw: u16) -> units::DegreesCelsius {
            units::calc_temperature_pure(
                raw,
                self.temp_cal.cal1_val,
                self.temp_cal.cal2_val,
                self.temp_cal.cal1_temp,
                self.temp_cal.cal2_temp,
                3300,
                self.temp_cal.cal_vref_mv,
            )
        }
    }
}
