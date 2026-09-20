//! Envelope ladder campaign: open-loop sine V/f through the first 30% of
//! the envelope (envelope := 100% duty at 14.3 Hz-e per duty-%), with the
//! full instrument spine (binz::harvest/stage/blackbox/telem).
//!
//! Rungs: 4..30% peak duty, step 2, f = duty * 14.3 Hz. Per rung: 1 s
//! V/f ramp -> 8 s dwell (10 kHz ISR guards + aggregation, 1 kHz binary
//! telemetry on VCOM @2 Mbaud) -> ~100 ms coast window (gates off, VPH1
//! reads raw BEMF: measured eHz + amplitude = the no-Hall ground truth)
//! -> resume, next rung. Soft envelope edge: sustained IS/VM/slip limits
//! end the ladder gracefully (no kill). Any guard kill safes the stage in
//! the ISR and the main loop dumps the blackbox over VCOM + RTT.
//!
//! Host commands (VCOM RX): 'k' = kill now, 'p' = provoke overcurrent
//! (drops the IS threshold to 1 mV; exercises detect->kill->dump).
//!
//! Run: `cargo run --release --example envelope-ladder`
//! Host: `python scripts/envelope_ladder.py`

#![no_std]
#![no_main]

use binz::{blackbox, harvest, stage, telem};
use core::fmt::Write;
use core::sync::atomic::Ordering::Relaxed;
use cortex_m_rt::entry;
use rtt_target::{rprintln, rtt_init_print};
use stm32g0xx_hal::prelude::*;
use stm32g0xx_hal::rcc::{Config, RccExt};
use stm32g0xx_hal::serial::FullConfig;
use stm32g0xx_hal::stm32;

const SINE_LUT: [u8; 256] = [
    128, 131, 134, 137, 140, 143, 146, 149, 152, 155, 158, 162, 165, 167, 170, 173, 176, 179, 182,
    185, 188, 190, 193, 196, 198, 201, 203, 206, 208, 211, 213, 215, 218, 220, 222, 224, 226, 228,
    230, 232, 234, 235, 237, 238, 240, 241, 243, 244, 245, 246, 248, 249, 250, 250, 251, 252, 253,
    253, 254, 254, 254, 255, 255, 255, 255, 255, 255, 255, 254, 254, 254, 253, 253, 252, 251, 250,
    250, 249, 248, 246, 245, 244, 243, 241, 240, 238, 237, 235, 234, 232, 230, 228, 226, 224, 222,
    220, 218, 215, 213, 211, 208, 206, 203, 201, 198, 196, 193, 190, 188, 185, 182, 179, 176, 173,
    170, 167, 165, 162, 158, 155, 152, 149, 146, 143, 140, 137, 134, 131, 128, 124, 121, 118, 115,
    112, 109, 106, 103, 100, 97, 93, 90, 88, 85, 82, 79, 76, 73, 70, 67, 65, 62, 59, 57, 54, 52,
    49, 47, 44, 42, 40, 37, 35, 33, 31, 29, 27, 25, 23, 21, 20, 18, 17, 15, 14, 12, 11, 10, 9, 7,
    6, 5, 5, 4, 3, 2, 2, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 2, 2, 3, 4, 5, 5, 6, 7, 9, 10, 11,
    12, 14, 15, 17, 18, 20, 21, 23, 25, 27, 29, 31, 33, 35, 37, 40, 42, 44, 47, 49, 52, 54, 57, 59,
    62, 65, 67, 70, 73, 76, 79, 82, 85, 88, 90, 93, 97, 100, 103, 106, 109, 112, 115, 118, 121,
    124,
];

const CARRIER_HZ: u32 = 10_000;
const ARR: u32 = 64_000_000 / CARRIER_HZ - 1; // 6399
const DTG: u8 = 26;
// Finer steps in the region this motor actually sustains open-loop
// (sync edge ~6-8%); a few coarse high rungs remain in case a healthier
// supply/motor reaches them. Ladder soft-stops at the current edge.
const RUNGS: [u32; 12] = [3, 4, 5, 6, 7, 8, 9, 10, 12, 15, 20, 30];
const CHZ_PER_PCT: u32 = 1430; // 14.3 Hz-e per duty-% (V/f, 7% <-> 100 Hz)
const DWELL_TICKS: u32 = 4 * CARRIER_HZ; // shortened for heat margin at high (desynced) rungs
const RAMP_TICKS: u32 = 5 * CARRIER_HZ / 2; // 2.5 s between rungs: the
// low-inertia rotor needs time to accelerate to the next V/f speed or it
// slips and draws stall current (measured: 1 s jump killed at 10%).
const PARK_TICKS: u32 = CARRIER_HZ / 2;
const START_RAMP_TICKS: u32 = 3 * CARRIER_HZ; // match spin-pwm's proven 3 s
// Soft envelope-edge limits (graceful stop, not kills):
// Soft envelope edge (graceful stop at rung end, BEFORE any hard kill):
// PSU limits ~800 mA, so stop at ~750 mA average or a 600 mV bus sag.
const SOFT_IS_MEAN_DELTA_MV: u16 = 45; // ~750 mA avg (60 mV/A)
const SOFT_VM_SAG_MV: u16 = 600;
const SOFT_SLIP_PCT: u32 = 25;
const CATCH_PCT: u32 = 7; // proven catch point: 7% / 100 Hz (spin-pwm)
const ALIGN_PCT: u32 = 3; // park/low-speed boost amplitude
// Graceful soft-abort on sustained STALL current (EWMA pin-mV over
// baseline) -- raised so the ladder climbs to the TRUE edge instead of
// aborting on synced-drive current. ~5 A of sustained lock current.
const SOFT_ABORT_IS_MV: u16 = 300;
// Bus-sag GRACEFUL edge (dwell-checked): fires at ~10.4 V (590 pin-mV),
// well ABOVE the hard VM-floor kill (now 75% of boot ~504 pin-mV / 8.9 V),
// so the ladder always auto-stops CLEAN (zero guard kills) as the 0.8 A
// PSU current-limits. Boot bus ~11.8 V = 672 pin-mV.
const BUS_SAG_ABORT_PIN_MV: u16 = 590;

fn duty_chz(pct: u32) -> u32 {
    pct * CHZ_PER_PCT
}

/// Stream the raw coast buffer as `CD,idx,pin_mv` text lines for offline
/// waveform review. Blocking (only runs during a coast, motor drifting).
fn dump_coast<W: Write>(w: &mut W) {
    let buf = harvest::coast_samples();
    for (i, &v) in buf.iter().enumerate() {
        let _ = writeln!(w, "CD,{},{}", i, v);
    }
}

/// Analyze a finished coast capture: (measured eHz*10, BEMF peak-to-min
/// in terminal mV). Skips the first ~2 ms of settle.
fn analyze_coast() -> (u32, u32) {
    let buf = harvest::coast_samples();
    let s = &buf[20..]; // ~2 ms settle
    let (mut mx, mut mn) = (0u16, u16::MAX);
    for &v in s {
        if v > mx {
            mx = v;
        }
        if v < mn {
            mn = v;
        }
    }
    let hi = mx / 2 + mx / 8;
    let lo = mx / 2 - mx / 8;
    let mut above = s[0] > hi;
    let mut humps = 0u32;
    for &v in s {
        if !above && v > hi {
            above = true;
            humps += 1;
        } else if above && v < lo {
            above = false;
        }
    }
    let win_us = (harvest::COAST_LEN as u32 - 20) * 101;
    let ehz10 = humps * 10_000_000 / win_us;
    let mv = (mx.saturating_sub(mn)) as u32 * 1567 / 100;
    (ehz10, mv)
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

    // Analog inputs for the harvest scan.
    let _bus = gpioa.pa1.into_analog();
    let _is = gpiob.pb11.into_analog();
    let _t = gpioc.pc4.into_analog();
    let _v1 = gpiob.pb1.into_analog();
    let _v2 = gpiob.pb0.into_analog();
    let _v3 = gpiob.pb2.into_analog();
    let _nflt = gpioa.pa6.into_floating_input();

    let mut en = gpioc.pc9.into_open_drain_output();
    let mut stby = gpioc.pc8.into_push_pull_output();
    en.set_low().ok();
    stby.set_high().ok();

    // VCOM: USART2 PA2/PA3 @ 2 Mbaud, FIFOs on.
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

    // Six drive pins to TIM1 AF2 (proven config).
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

    // TIM1 raw setup (spin-pwm recipe).
    unsafe {
        let rcc_raw = &*stm32::RCC::ptr();
        rcc_raw
            .apbenr2()
            .modify(|r, w| w.bits(r.bits() | (1 << 11)));
        let tim = &*stm32::TIM1::ptr();
        tim.cr1().write(|w| w.bits(0));
        tim.psc().write(|w| w.bits(0));
        tim.arr().write(|w| w.bits(ARR));
        tim.rcr().write(|w| w.bits(0));
        tim.ccmr1_output().write(|w| w.bits(0x6868));
        tim.ccmr2_output().write(|w| w.bits(0x0068));
        tim.ccer().write(|w| w.bits(0x0555));
        tim.ccr1().write(|w| w.bits(0));
        tim.ccr2().write(|w| w.bits(0));
        tim.ccr3().write(|w| w.bits(0));
        tim.bdtr()
            .write(|w| w.bits((1 << 11) | (1 << 10) | DTG as u32));
        tim.egr().write(|w| w.bits(1));
        tim.cr1().write(|w| w.bits(0x81)); // ARPE|CEN
    }

    // Instrument spine up.
    harvest::init();
    delay.delay(50.millis()); // let the scan populate
    let vm0 = harvest::VM_MV.load(Relaxed) as u32 * 1759 / 100;
    rprintln!(
        "envelope-ladder: VM~{}.{:02} V, rungs {:?}%, carrier {} Hz",
        vm0 / 1000,
        (vm0 % 1000) / 10,
        RUNGS,
        CARRIER_HZ
    );
    if vm0 < 8000 {
        stage::force_safe();
        rprintln!("!! VM too low, aborting before arm");
        loop {
            cortex_m::asm::nop();
        }
    }
    // Driver on + precharge, THEN arm (nFLT guard reads the EN node).
    en.set_high().ok();
    delay.delay(2.millis());
    unsafe {
        (&*stm32::TIM1::ptr())
            .bdtr()
            .modify(|r, w| w.bits(r.bits() | (1 << 15)));
    }
    delay.delay(30.millis());
    harvest::arm();
    // Emit the true zero-current baseline + VM divider so the host study
    // computes absolute current correctly (min(is_min) is NOT the baseline
    // -- OFF-vector samples dip below idle).
    let _ = writeln!(
        serial,
        "BASE,{},{}",
        harvest::IS_BASE_MV.load(Relaxed),
        harvest::VM_MV.load(Relaxed)
    );
    rprintln!(
        "armed: IS base {} mV, VM floor {} mV(pin)",
        harvest::IS_BASE_MV.load(Relaxed),
        harvest::VM_FLOOR_MV.load(Relaxed)
    );

    // Ladder state.
    let tim = unsafe { &*stm32::TIM1::ptr() };
    let mut txring = telem::TxRing::new();
    let mut seq: u8 = 0;
    let mut phase: u32 = 0;
    let mut amp: u32 = 0;
    let mut freq_chz: u32 = 500; // 5 Hz
    let mut tick: u32 = 0;
    let mut rung: usize = 0;
    // stage codes: 0 park, 1 start-ramp, 2 dwell, 3 inter-rung ramp, 4 coast
    let mut st: u8 = 0;
    let mut st_tick: u32 = 0;
    let mut ramp_from: (u32, u32) = (0, 500); // (amp, chz)
    let mut end_reason: &str = "";
    let mut measured_ehz10: u32 = 0;
    let mut coast_settle: u32 = 0;
    let mut bus_sag_logged: bool = false;
    let mut coast_phase0: u16 = 0;

    let amp_of = |pct: u32| (ARR + 1) * pct / 100;

    loop {
        // Pace on TIM1 update.
        while tim.sr().read().bits() & 1 == 0 {}
        tim.sr().modify(|r, w| unsafe { w.bits(r.bits() & !1) });
        tick += 1;
        st_tick += 1;

        // Host commands.
        if let Ok(b) = serial.read() {
            match b {
                b'k' => {
                    stage::force_safe();
                    harvest::KILL.store(blackbox::KIND_KILL_HOST, Relaxed);
                }
                b'p' => {
                    blackbox::push(
                        harvest::TICKS_100US.load(Relaxed),
                        blackbox::KIND_PROVOKE,
                        0,
                    );
                    harvest::IS_KILL_DELTA_MV.store(1, Relaxed);
                }
                _ => {}
            }
        }

        // ISR kill? Stage is already safe; report and stop.
        let kill = harvest::KILL.load(Relaxed);
        if kill != 0 {
            end_reason = "KILL";
            blackbox::dump(&mut serial);
            let _ = writeln!(serial, "END,kill,{}", kill);
            rprintln!("ladder ended: ISR kill kind {}", kill);
            break;
        }

        // GRACEFUL auto-stop at the PSU-limited edge -- fires ABOVE the hard
        // VM-floor kill so the ladder ends cleanly (zero guard kills) when
        // the 0.8 A supply can no longer hold the bus. This IS the envelope
        // edge on a current-limited supply; 30% is unreachable here (the bus
        // would collapse), and closed-loop is the path past it. Dwell-only
        // (st==2, 1 s settle) so ramp transients don't abort.
        if st == 2 && st_tick > CARRIER_HZ {
            let vm_pin = harvest::VM_MV.load(Relaxed);
            let iavg = harvest::IS_AVG_MV.load(Relaxed);
            if vm_pin < BUS_SAG_ABORT_PIN_MV {
                end_reason = "envelope edge: PSU current limit (graceful, pre-kill)";
                rprintln!(
                    "graceful stop at rung {} ({}%): bus sag {} pin-mV",
                    rung,
                    RUNGS[rung],
                    vm_pin
                );
                let _ = bus_sag_logged;
                break;
            }
            if iavg.abs_diff(harvest::IS_BASE_MV.load(Relaxed)) > SOFT_ABORT_IS_MV {
                end_reason = "thermal courtesy: sustained stall current";
                rprintln!(
                    "graceful stop at rung {} ({}%): stall current",
                    rung,
                    RUNGS[rung]
                );
                break;
            }
        }

        // State machine.
        match st {
            0 => {
                // Park/align at a LOW amplitude (ALIGN_PCT). Holding the
                // full 7% at 5 Hz over-fluxes and pulls multi-amp current
                // (the V/f trap); a gentle align current is enough.
                amp = amp_of(ALIGN_PCT) * st_tick / PARK_TICKS;
                if st_tick >= PARK_TICKS {
                    st = 1;
                    st_tick = 0;
                }
            }
            1 => {
                // Catch ramp with proper V/f: frequency 5->100 Hz AND
                // amplitude scaled to frequency (constant flux), with an
                // ALIGN_PCT low-speed boost so it has starting torque.
                let target = duty_chz(CATCH_PCT);
                freq_chz = 500 + (target - 500) * st_tick / START_RAMP_TICKS;
                let vf = amp_of(CATCH_PCT) as u64 * freq_chz as u64 / target as u64;
                amp = core::cmp::max(amp_of(ALIGN_PCT), vf as u32);
                if st_tick >= START_RAMP_TICKS {
                    // Verify the catch with a coast before laddering.
                    harvest::COAST_IDX.store(0, Relaxed);
                    harvest::COAST_MODE.store(1, Relaxed);
                    unsafe {
                        tim.bdtr().modify(|r, w| w.bits(r.bits() & !(1 << 15)));
                    }
                    st = 6;
                    st_tick = 0;
                    coast_settle = 0;
                }
            }
            6 => {
                // Catch-verification coast.
                if harvest::COAST_MODE.load(Relaxed) == 2 {
                    let (ehz10, mv) = analyze_coast();
                    harvest::COAST_MODE.store(0, Relaxed);
                    // One-shot raw coast dump for offline waveform review.
                    dump_coast(&mut serial);
                    let _ = writeln!(serial, "CATCH,{},{},{}", freq_chz, ehz10, mv);
                    rprintln!(
                        "catch: cmd {}.{:02} Hz, coast eHz*10={} BEMF {} mV",
                        freq_chz / 100,
                        freq_chz % 100,
                        ehz10,
                        mv
                    );
                    // Coast BEMF is a low-inertia-limited proxy, not a hard
                    // gate -- proceed and let the current metrics + operator
                    // eyes be the sync truth.
                    unsafe {
                        tim.bdtr().modify(|r, w| w.bits(r.bits() | (1 << 15)));
                    }
                    ramp_from = (amp, freq_chz);
                    st = 3;
                    st_tick = 0;
                } else {
                    coast_settle += 1;
                    if coast_settle > CARRIER_HZ {
                        end_reason = "coast timeout";
                        break;
                    }
                }
            }
            2 => {
                // Dwell.
                if st_tick >= DWELL_TICKS {
                    // Rung summary from ISR aggregates.
                    let n = harvest::AGG_N.load(Relaxed).max(1);
                    let is_mean = harvest::AGG_IS_SUM.load(Relaxed) / n;
                    let vm_mean = harvest::AGG_VM_SUM.load(Relaxed) / n;
                    let is_min = harvest::AGG_IS_MIN.load(Relaxed);
                    let is_max = harvest::AGG_IS_MAX.load(Relaxed);
                    let vm_min = harvest::AGG_VM_MIN.load(Relaxed);
                    let vm_max = harvest::AGG_VM_MAX.load(Relaxed);
                    // Coast: gates off, capture BEMF. Record the commanded
                    // field phase at gate-off (LUT index 0..255 = 0..360 deg
                    // electrical) -- the coast BEMF's phase at t=0 vs this is
                    // the LOAD ANGLE (rotor lag behind the commanded field).
                    coast_phase0 = (phase >> 24) as u16;
                    blackbox::push(
                        harvest::TICKS_100US.load(Relaxed),
                        blackbox::KIND_COAST_START,
                        coast_phase0,
                    );
                    harvest::COAST_IDX.store(0, Relaxed);
                    harvest::COAST_MODE.store(1, Relaxed);
                    unsafe {
                        tim.bdtr().modify(|r, w| w.bits(r.bits() & !(1 << 15)));
                    }
                    st = 4;
                    st_tick = 0;
                    coast_settle = 0;
                    // Emit rung summary line (measured eHz appended after coast).
                    let _ = writeln!(
                        serial,
                        "RUNG,{},{},{},{},{},{},{},{},{}",
                        rung,
                        RUNGS[rung],
                        freq_chz,
                        is_min,
                        is_mean,
                        is_max,
                        vm_min,
                        vm_mean,
                        vm_max
                    );
                    rprintln!(
                        "rung {} ({}%): IS {}/{}/{} mV VM {}/{}/{} mV",
                        rung,
                        RUNGS[rung],
                        is_min,
                        is_mean,
                        is_max,
                        vm_min,
                        vm_mean,
                        vm_max
                    );
                    // Soft envelope edge checks (evaluated at rung end).
                    let base = harvest::IS_BASE_MV.load(Relaxed);
                    if (is_mean as u16).abs_diff(base) > SOFT_IS_MEAN_DELTA_MV {
                        end_reason = "envelope: IS mean";
                    }
                    // All pin-mV. Boot pin-mV = floor / 0.85; SAG threshold
                    // is bus-mV / 17.59 (the recurring units scar).
                    let boot_vm = harvest::VM_FLOOR_MV.load(Relaxed) as u32 * 20 / 17;
                    let sag_pin = (SOFT_VM_SAG_MV as u32) * 100 / 1759;
                    if (vm_mean as u32) + sag_pin < boot_vm {
                        end_reason = "envelope: VM sag";
                    }
                }
            }
            3 => {
                // Inter-rung V/f ramp -- SIGNED: the first transition
                // (catch 7% -> rung 4%) ramps DOWN; u32 math here wrapped
                // and blew CCR past ARR (real bench kill, 2026-09-05).
                let ta = amp_of(RUNGS[rung]) as i64;
                let tf = duty_chz(RUNGS[rung]) as i64;
                let fa = ramp_from.0 as i64;
                let ff = ramp_from.1 as i64;
                amp = (fa + (ta - fa) * st_tick as i64 / RAMP_TICKS as i64).max(0) as u32;
                freq_chz = (ff + (tf - ff) * st_tick as i64 / RAMP_TICKS as i64).max(500) as u32;
                if st_tick >= RAMP_TICKS {
                    st = 2;
                    st_tick = 0;
                    harvest::reset_aggregates();
                    blackbox::push(
                        harvest::TICKS_100US.load(Relaxed),
                        blackbox::KIND_RUNG_START,
                        RUNGS[rung] as u16,
                    );
                }
            }
            4 => {
                // Coasting; phase keeps advancing for smooth re-catch.
                if harvest::COAST_MODE.load(Relaxed) == 2 {
                    let (e10, bmv) = analyze_coast();
                    measured_ehz10 = e10;
                    let bemf_term_mv = bmv;
                    let t = harvest::TICKS_100US.load(Relaxed);
                    blackbox::push(t, blackbox::KIND_COAST_RESULT_HZ10, measured_ehz10 as u16);
                    blackbox::push(t, blackbox::KIND_COAST_RESULT_MV, bemf_term_mv as u16);
                    let _ = writeln!(
                        serial,
                        "COAST,{},{},{},{},{}",
                        rung, freq_chz, measured_ehz10, bemf_term_mv, coast_phase0
                    );
                    // Per-rung coast waveform (CDR,rung,idx,mv) so the host
                    // can extract the rotor BEMF phase and compute LOAD ANGLE
                    // (rotor phase at gate-off vs the commanded field phase
                    // coast_phase0). Motor is drifting -- blocking write ok.
                    {
                        let buf = harvest::coast_samples();
                        let mut i = 0;
                        while i < harvest::COAST_LEN {
                            let _ = writeln!(serial, "CDR,{},{},{}", rung, i, buf[i]);
                            while serial.flush().is_err() {}
                            i += 1;
                        }
                    }
                    rprintln!(
                        "coast: cmd {}.{:02} Hz, measured {}.{} Hz-e, BEMF ~{} mV pk",
                        freq_chz / 100,
                        freq_chz % 100,
                        measured_ehz10 / 10,
                        measured_ehz10 % 10,
                        bemf_term_mv
                    );
                    harvest::COAST_MODE.store(0, Relaxed);
                    let _ = end_reason;
                    let _ = measured_ehz10;
                    // Resume drive.
                    unsafe {
                        tim.bdtr().modify(|r, w| w.bits(r.bits() | (1 << 15)));
                    }
                    blackbox::push(t, blackbox::KIND_RESUME, RUNGS[rung] as u16);
                    // Next rung or done. (Envelope edge = current/VM soft
                    // limits evaluated at rung end, below; coast eHz is a
                    // reported proxy, not a stop trigger on this prop.)
                    if !end_reason.is_empty() {
                        break;
                    }
                    if rung + 1 < RUNGS.len() {
                        rung += 1;
                        ramp_from = (amp, freq_chz);
                        st = 3;
                        st_tick = 0;
                    } else {
                        end_reason = "complete";
                        break;
                    }
                } else {
                    coast_settle += 1;
                    if coast_settle > CARRIER_HZ {
                        // capture never completed (shouldn't happen)
                        end_reason = "coast timeout";
                        break;
                    }
                }
            }
            _ => {}
        }

        // Sine outputs (harmless during coast: MOE off).
        if st != 0 {
            phase = phase.wrapping_add(
                ((freq_chz as u64) * (1u64 << 32) / (CARRIER_HZ as u64 * 100)) as u32,
            );
        }
        let i0 = (phase >> 24) as usize;
        let i1 = (phase.wrapping_add(0x5555_5555) >> 24) as usize;
        let i2 = (phase.wrapping_add(0xAAAA_AAAA) >> 24) as usize;
        unsafe {
            tim.ccr1().write(|w| w.bits(amp * SINE_LUT[i0] as u32 >> 8));
            tim.ccr2().write(|w| w.bits(amp * SINE_LUT[i1] as u32 >> 8));
            tim.ccr3().write(|w| w.bits(amp * SINE_LUT[i2] as u32 >> 8));
        }

        // 1 kHz binary telemetry.
        if tick % 10 == 0 {
            seq = seq.wrapping_add(1);
            let state_byte = (rung as u8 & 0x0F) | if st == 4 { 0x40 } else { 0 };
            let f = telem::build_frame(
                seq,
                state_byte,
                (phase >> 16) as u16,
                harvest::VPH1_MV.load(Relaxed),
                harvest::VPH2_MV.load(Relaxed),
                harvest::VPH3_MV.load(Relaxed),
                harvest::IS_MV.load(Relaxed),
                harvest::VM_MV.load(Relaxed),
                freq_chz as u16,
                amp as u16,
            );
            txring.push(&f);
        }
        // Drain a few bytes per tick, never blocking.
        txring.drain(4, |b| serial.write(b).is_ok());
    }

    // Graceful end (kill path already dumped above).
    stage::force_safe();
    harvest::disarm();
    if end_reason != "KILL" {
        blackbox::push(
            harvest::TICKS_100US.load(Relaxed),
            blackbox::KIND_KILL_DONE,
            rung as u16,
        );
        blackbox::dump(&mut serial);
        let _ = writeln!(serial, "END,{},{}", end_reason, RUNGS[rung]);
        rprintln!(
            "ladder ended: {} at rung {} ({}%)",
            end_reason,
            rung,
            RUNGS[rung]
        );
    }
    loop {
        // Keep draining so the tail of the stream reaches the host.
        txring.drain(8, |b| serial.write(b).is_ok());
        cortex_m::asm::nop();
    }
}
