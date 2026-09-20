//! BEMF-RING — the raw floating-phase ADC characterization instrument.
//!
//! A direct port of minz's WAXWING phase-voltage ring: a 20 kHz always-on
//! ring of the floating-phase ADC sample + 3-phase-mean neutral + position-
//! in-commutation-window + step + comparator level, dumped over VCOM as ASCII
//! hex for host reconstruction (scripts/bemf_ring.py). The host folds many
//! commutation windows onto one position axis (equivalent-time scatter) so you
//! SEE the true BEMF: a clean ramp crossing the neutral ~30 deg before
//! commutation, the demag spike right after commutation (sets BLANK), the
//! noise band at the crossing (sets HYST), all scaling with speed (the sweep
//! proves it's real omega-dependent BEMF, not a fixed artifact).
//!
//! Drive = SMOOTH open-loop forced six-step (the reference the operator
//! confirms sounds synced), swept across a few speeds. No closed loop, no
//! comparator knobs — this is the measurement that PERMITS tuning them.
//!
//! Run: `cargo run --release --example bemf-ring`

#![no_std]
#![no_main]

use binz::{mzhal, stage};
use core::fmt::Write;
use core::sync::atomic::{AtomicU16, AtomicU32, AtomicUsize, Ordering::Relaxed};
use cortex_m_rt::entry;
use rtt_target::{rprintln, rtt_init_print};
use stm32g0xx_hal::prelude::*;
use stm32g0xx_hal::rcc::{Config, RccExt};
use stm32g0xx_hal::serial::{FullConfig, Serial};
use stm32g0xx_hal::stm32;
use stm32g0xx_hal::stm32::{USART2, interrupt};

// ---- WAXWING-style ring (sole writer = TIM6 ISR) ----
const RING_N: usize = 1024;
static RG_VF: [AtomicU16; RING_N] = [const { AtomicU16::new(0) }; RING_N];
static RG_NEU: [AtomicU16; RING_N] = [const { AtomicU16::new(0) }; RING_N];
static RG_POS: [AtomicU16; RING_N] = [const { AtomicU16::new(0) }; RING_N];
/// (step<<12) | (TIM1.CNT & 0x0FFF) | (comp_level<<15)
static RG_T1S: [AtomicU16; RING_N] = [const { AtomicU16::new(0) }; RING_N];
static RG_HEAD: AtomicUsize = AtomicUsize::new(0);
/// Current forced step 1..6 (main writes, ISR reads for T1S).
static STEP: AtomicU16 = AtomicU16::new(1);
static RING_ON: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

#[interrupt]
fn TIM6_DAC_LPTIM1() {
    unsafe {
        (&*stm32::TIM6::ptr())
            .sr()
            .modify(|r, w| w.bits(r.bits() & !1));
    }
    mzhal::sample_bemf(); // refresh the DMA-scan caches + ZC_LEVEL
    if !RING_ON.load(Relaxed) {
        return;
    }
    let (vf, neu) = mzhal::float_neutral_raw();
    let (pos, t1) = unsafe {
        (
            (&*stm32::TIM2::ptr()).cnt().read().bits() as u16, // 0.5 us since commutation
            (&*stm32::TIM1::ptr()).cnt().read().bits() as u16 & 0x0FFF,
        )
    };
    let step = STEP.load(Relaxed);
    let comp = mzhal::zc_level() as u16;
    let h = RG_HEAD.load(Relaxed);
    RG_VF[h].store(vf, Relaxed);
    RG_NEU[h].store(neu, Relaxed);
    RG_POS[h].store(pos, Relaxed);
    RG_T1S[h].store((step << 12) | t1 | (comp << 15), Relaxed);
    RG_HEAD.store((h + 1) % RING_N, Relaxed);
}

fn dump_ring(serial: &mut Serial<USART2, FullConfig>, freq_us: u32) {
    let head = RG_HEAD.load(Relaxed);
    let _ = writeln!(
        serial,
        "RG n={} head={} period_us={}",
        RING_N, head, freq_us
    );
    for i in 0..RING_N {
        // vf neu pos t1s as 4 hex u16s per line (host reorders from head=).
        let _ = writeln!(
            serial,
            "{:04x} {:04x} {:04x} {:04x}",
            RG_VF[i].load(Relaxed),
            RG_NEU[i].load(Relaxed),
            RG_POS[i].load(Relaxed),
            RG_T1S[i].load(Relaxed),
        );
        while serial.flush().is_err() {}
    }
    let _ = writeln!(serial, "RG END");
    while serial.flush().is_err() {}
}

#[entry]
fn main() -> ! {
    rtt_init_print!();
    let dp = stm32::Peripherals::take().unwrap();
    let cp = cortex_m::Peripherals::take().unwrap();
    let mut rcc = dp.RCC.freeze(Config::pll());
    let mut delay = cp.SYST.delay(&mut rcc);

    let gpioa = dp.GPIOA.split(&mut rcc);
    let gpiob = dp.GPIOB.split(&mut rcc);
    let gpioc = dp.GPIOC.split(&mut rcc);
    let gpiod = dp.GPIOD.split(&mut rcc);

    let _ = (
        gpioa.pa1.into_analog(),
        gpiob.pb0.into_analog(),
        gpiob.pb1.into_analog(),
        gpiob.pb2.into_analog(),
        gpiob.pb11.into_analog(),
    );
    let _nflt = gpioa.pa6.into_floating_input();
    let mut en = gpioc.pc9.into_open_drain_output();
    let mut stby = gpioc.pc8.into_push_pull_output();
    en.set_low().ok();
    stby.set_high().ok();

    let mut serial = dp
        .USART2
        .usart(
            (gpioa.pa2, gpioa.pa3),
            FullConfig::default()
                .baudrate(2_000_000.bps())
                .fifo_enable(),
            &mut rcc,
        )
        .unwrap();

    let _ = (
        gpioa.pa7, gpioa.pa8, gpioa.pa9, gpioa.pa10, gpiod.pd3, gpiod.pd4,
    );
    unsafe {
        let pa = &*stm32::GPIOA::ptr();
        pa.moder().modify(|_, w| {
            w.moder7()
                .alternate()
                .moder8()
                .alternate()
                .moder9()
                .alternate()
                .moder10()
                .alternate()
        });
        pa.afrl().modify(|_, w| w.afr(7).af2());
        pa.afrh()
            .modify(|_, w| w.afr(0).af2().afr(1).af2().afr(2).af2());
        let pd = &*stm32::GPIOD::ptr();
        pd.moder()
            .modify(|_, w| w.moder3().alternate().moder4().alternate());
        pd.afrl().modify(|_, w| w.afr(3).af2().afr(4).af2());
    }

    mzhal::tim1_init();
    mzhal::adc_init();
    mzhal::timers_init(); // TIM2 (interval) + TIM16
    delay.delay(5.millis());

    let vm0 = mzhal::vm_mv();
    let bus_floor = vm0 * 3 / 5;
    rprintln!("bemf-ring: VM {} mV, floor {}", vm0, bus_floor);

    // TIM6 @ 20 kHz.
    unsafe {
        let rcc_raw = &*stm32::RCC::ptr();
        rcc_raw.apbenr1().modify(|r, w| w.bits(r.bits() | (1 << 4)));
        let t6 = &*stm32::TIM6::ptr();
        t6.psc().write(|w| w.bits(63));
        t6.arr().write(|w| w.bits(49));
        t6.dier().write(|w| w.bits(1));
        t6.egr().write(|w| w.bits(1));
        t6.cr1().write(|w| w.bits(1));
    }

    en.set_high().ok();
    delay.delay(2.millis());
    mzhal::moe(true);
    delay.delay(20.millis());

    unsafe {
        cortex_m::peripheral::NVIC::unmask(stm32::Interrupt::TIM6_DAC_LPTIM1);
    }

    // Align.
    let align_ccr: u16 = 160;
    mzhal::drive_step(1, align_ccr);
    STEP.store(1, Relaxed);
    delay.delay(300.millis());

    // Accelerating ramp into a smooth spin, then capture at a few hold speeds.
    let ccr: u16 = 640; // 20% (the closed-loop's operating duty)
    let mut step: u8 = 1;
    let mut period = 16_000u32;
    // ramp down to 5000 us/step
    while period > 5_000 {
        step = (step % 6) + 1;
        mzhal::drive_step(step, ccr);
        STEP.store(step as u16, Relaxed);
        unsafe {
            (&*stm32::TIM2::ptr()).cnt().write(|w| w.bits(0)); // POS resets per commutation
        }
        let bus = mzhal::vm_mv();
        if bus < bus_floor {
            stage::force_safe();
            rprintln!("bemf-ring: sag {} abort", bus);
            loop {
                cortex_m::asm::nop();
            }
        }
        delay.delay(period.micros());
        period -= 60;
    }

    // Hold a FIXED smooth speed and SWEEP the ADC trigger point across the PWM
    // period (ON window is CNT<640 at 20% duty; OFF/freewheel is CNT>640). The
    // per-trigger VF swing reveals WHERE the BEMF is actually visible — the
    // mid-ON point (80) showed almost none, so hunt the OFF window.
    let hold_us = 4000u32;
    // commutate one forced step (+ POS reset + sag guard). Ring captures only
    // while this is being called (RING_ON), so the rotor is actually SPINNING.
    let mut commutate = |step: &mut u8| -> bool {
        *step = (*step % 6) + 1;
        mzhal::drive_step(*step, ccr);
        STEP.store(*step as u16, Relaxed);
        unsafe {
            (&*stm32::TIM2::ptr()).cnt().write(|w| w.bits(0));
        }
        mzhal::vm_mv() >= bus_floor
    };
    for &trig in &[80u16, 300, 640, 900, 1300, 1800, 2400] {
        mzhal::set_adc_trigger(trig);
        // spin up ~250 ms (ring off)
        RING_ON.store(false, Relaxed);
        for _ in 0..60 {
            for _ in 0..6 {
                if !commutate(&mut step) {
                    stage::force_safe();
                    rprintln!("bemf-ring: sag abort");
                    loop {
                        cortex_m::asm::nop();
                    }
                }
                delay.delay(hold_us.micros());
            }
        }
        // CAPTURE while commutating: ~90 ms (>RING_N/20kHz) so the ring fills
        // with genuinely spinning samples across all 6 steps.
        RING_ON.store(true, Relaxed);
        for _ in 0..24 {
            for _ in 0..6 {
                if !commutate(&mut step) {
                    stage::force_safe();
                    loop {
                        cortex_m::asm::nop();
                    }
                }
                delay.delay(hold_us.micros());
            }
        }
        RING_ON.store(false, Relaxed);
        rprintln!("bemf-ring: dumping trigger={} (spinning capture)", trig);
        dump_ring(&mut serial, trig as u32);
    }

    stage::force_safe();
    let _ = writeln!(serial, "RG ALLDONE");
    rprintln!("bemf-ring: done, stage safed");
    loop {
        cortex_m::asm::nop();
    }
}
