//! Bench shell (see shell.rs) + raw-TIM1 3-phase sine drive campaign.
//! TIM1 generates complementary PWM at 10 kHz with dead-time on the native
//! PA8/PA9/PA10 + PA7/PB0/PB1 pin set. Duty is runtime-selectable from
//! 0.1..10% (default target 6%), sine-modulated at 120 degree offsets. UART is drained
//! every loop pass so `off` stays responsive; a 5 s energized timeout is hard.
//! All wired ADC and comparator feedback is retained at 1 kHz for the final
//! 500 ms of an attempt. Then all gates and ENABLE are cleared before a 1 kHz,
//! 500 ms high-impedance BEMF coast capture and serial dump.
//!
//! Extra commands (on top of shell.rs):
//!   sine       toggle the sine drive (auto-sets en=1). Off = coast (gates low).
//!   sf <hz>    set electrical frequency (default 10 Hz)
//!   run<hz>    1% ALIGN -> 100Hz/7% CATCH -> RAMP -> target HOLD -> COAST
//! TIM1 channels map C/B/A to CH1/CH2/CH3 respectively, preserving the wire map.
//! Run: cargo run --release --example shell-sine

#![no_std]
#![no_main]

use core::fmt::Write;
use core::panic::PanicInfo;
use cortex_m::peripheral::syst::SystClkSource;
use cortex_m_rt::entry;
use rtt_target::{rprintln, rtt_init_print};
use stm32g0xx_hal::prelude::*;
use stm32g0xx_hal::rcc::{Config, RccExt};
use stm32g0xx_hal::serial::BasicConfig;
use stm32g0xx_hal::stm32;

const VREFINT_CAL_ADDR: u32 = 0x1FFF_75AA;
const COMP2_CSR: *mut u32 = 0x4001_0204 as *mut u32;
const CARRIER_HZ: u32 = 10_000;
const CONTROL_HZ: u32 = 1_000;
const PWM_ARR: u32 = 64_000_000 / CARRIER_HZ - 1;
const PWM_PERIOD_US: u32 = 100;
const MAX_ON_US_LIMIT: u32 = 100; // command units are 0.1%; hard ceiling = 10%
const DEFAULT_ON_US: u32 = 60; // 6.0%, operator-established catch region
const START_ON_US: u32 = 10; // 1.0%, proven electrically clean startup amplitude
const CATCH_DUTY_TENTHS: u32 = 70; // bench-proven start at 100 electrical Hz
const RUN_TICKS: u32 = 5_000;
const ALIGN_TICKS: u32 = 20;
const START_TICKS: u32 = 980;
const START_HZ: u32 = 100;
const RAMP_TICKS: u32 = 2_000;
const HOLD_TICKS: u32 = 2_000;
const CAPTURE_DIV: u32 = 1; // every 1 kHz sine/control update
const CAPTURE_HZ: u32 = CONTROL_HZ / CAPTURE_DIV;
const CAPTURE_N: usize = 500; // rolling final 500 ms
const COAST_HZ: u32 = 2_000;
const COAST_PERIOD_US: u16 = (1_000_000 / COAST_HZ) as u16;
const COAST_N: usize = 500; // first 250 ms after disable

// Raw capture record. Keeping ADC values unscaled matches minz's proven dump
// pattern: retain integer wire truth, carry VREFINT, convert on the host.
#[repr(C)]
#[derive(Clone, Copy)]
struct Capture {
    tick: u16,
    freq_chz: u16,
    ia: u16,
    ib: u16,
    ic: u16,
    vsenc: u16,
    neutral: u16,
    vbus: u16,
    vref: u16,
    theta: u8,
    flags: u8, // bit0 nFAULT high; bits1/2/3 comparator A/B/C high
    on_a: u8,
    on_b: u8,
    on_c: u8,
    stage: u8, // 0=fixed, 1=align, 2=start, 3=ramp, 4=hold
}

const EMPTY_CAPTURE: Capture = Capture {
    tick: 0,
    freq_chz: 0,
    ia: 0,
    ib: 0,
    ic: 0,
    vsenc: 0,
    neutral: 0,
    vbus: 0,
    vref: 0,
    theta: 0,
    flags: 0,
    on_a: 0,
    on_b: 0,
    on_c: 0,
    stage: 0,
};

static mut CAPTURE: [Capture; CAPTURE_N] = [EMPTY_CAPTURE; CAPTURE_N];

#[repr(C)]
#[derive(Clone, Copy)]
struct CoastCapture {
    tick: u16,
    vsenc: u16,
    neutral: u16,
    vbus: u16,
    vref: u16,
    flags: u8, // bit0 nFAULT high; bits1/2/3 comparator A/B/C high
    _pad: u8,
}

const EMPTY_COAST: CoastCapture = CoastCapture {
    tick: 0,
    vsenc: 0,
    neutral: 0,
    vbus: 0,
    vref: 0,
    flags: 0,
    _pad: 0,
};

static mut COAST_CAPTURE: [CoastCapture; COAST_N] = [EMPTY_COAST; COAST_N];

// sine LUT: 0..255, one electrical period (sine offset to 128 mid).
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

unsafe fn adc_read(ch: u8) -> u16 {
    let adc = &*stm32::ADC::ptr();
    adc.isr().write(|w| w.bits(1 << 13));
    adc.chselr0().write(|w| w.bits(1 << ch));
    while adc.isr().read().bits() & (1 << 13) == 0 {}
    adc.cr().modify(|r, w| w.bits(r.bits() | (1 << 2)));
    while adc.isr().read().bits() & (1 << 2) == 0 {}
    adc.dr().read().bits() as u16
}

fn set_pin(port: u8, bit: u32, on: bool) {
    let v = if on { 1 << bit } else { 1 << (bit + 16) };
    unsafe {
        match port {
            0 => (*stm32::GPIOA::ptr()).bsrr().write(|w| w.bits(v)),
            1 => (*stm32::GPIOB::ptr()).bsrr().write(|w| w.bits(v)),
            2 => (*stm32::GPIOC::ptr()).bsrr().write(|w| w.bits(v)),
            _ => (*stm32::GPIOD::ptr()).bsrr().write(|w| w.bits(v)),
        };
    }
}
fn bsrr_a(set: u32, clr: u32) {
    unsafe {
        (*stm32::GPIOA::ptr())
            .bsrr()
            .write(|w| w.bits(set | (clr << 16)));
    }
}
fn bsrr_b(set: u32, clr: u32) {
    unsafe {
        (*stm32::GPIOB::ptr())
            .bsrr()
            .write(|w| w.bits(set | (clr << 16)));
    }
}
fn get_odr(port: u8, bit: u32) -> bool {
    unsafe {
        let r = match port {
            0 => (*stm32::GPIOA::ptr()).odr().read().bits(),
            1 => (*stm32::GPIOB::ptr()).odr().read().bits(),
            2 => (*stm32::GPIOC::ptr()).odr().read().bits(),
            _ => (*stm32::GPIOD::ptr()).odr().read().bits(),
        };
        r & (1 << bit) != 0
    }
}
fn get_idr(port: u8, bit: u32) -> bool {
    unsafe {
        let r = match port {
            0 => (*stm32::GPIOA::ptr()).idr().read().bits(),
            1 => (*stm32::GPIOB::ptr()).idr().read().bits(),
            2 => (*stm32::GPIOC::ptr()).idr().read().bits(),
            _ => (*stm32::GPIOD::ptr()).idr().read().bits(),
        };
        r & (1 << bit) != 0
    }
}
fn pin_lookup(name: &[u8]) -> Option<(u8, u32)> {
    match name {
        b"ah" => Some((0, 10)),
        b"bh" => Some((0, 9)),
        b"ch" => Some((0, 8)),
        b"al" => Some((1, 1)),
        b"bl" => Some((1, 0)),
        b"cl" => Some((0, 7)),
        b"led" => Some((1, 5)),
        b"ld4" => Some((0, 5)),
        b"en" => Some((3, 1)),
        _ => None,
    }
}
const ALL_PINS: [(&str, u8, u32); 11] = [
    ("ah", 0, 10),
    ("bh", 0, 9),
    ("ch", 0, 8),
    ("al", 1, 1),
    ("bl", 1, 0),
    ("cl", 0, 7),
    ("led", 1, 5),
    ("ld4", 0, 5),
    ("en", 3, 1),
    ("nflt", 1, 14),
    ("btn", 2, 13),
];
fn comp_read(code: u32) -> bool {
    unsafe {
        let v = core::ptr::read_volatile(COMP2_CSR);
        core::ptr::write_volatile(
            COMP2_CSR,
            (v & !(0xF << 4 | 0x3 << 8)) | (code << 4) | (0b10 << 8),
        );
    }
    cortex_m::asm::delay(3000);
    unsafe { core::ptr::read_volatile(COMP2_CSR) & (1 << 30) != 0 }
}

fn feedback_flags() -> u8 {
    (get_idr(1, 14) as u8)
        | ((comp_read(6) as u8) << 1)
        | ((comp_read(7) as u8) << 2)
        | ((comp_read(8) as u8) << 3)
}

fn current_read_rotated(index: usize) -> [u16; 3] {
    let mut current = [0u16; 3];
    let channels = [0u8, 1u8, 4u8];
    let first = index % 3;
    for offset in 0..3 {
        let phase = (first + offset) % 3;
        current[phase] = unsafe { adc_read(channels[phase]) };
    }
    current
}

fn capture_write(
    index: usize,
    tick: u32,
    freq_chz: u32,
    theta: u32,
    on: [u16; 3],
    stage: u8,
) -> Capture {
    let current = current_read_rotated(index);
    let sample = Capture {
        tick: tick as u16,
        freq_chz: freq_chz as u16,
        ia: current[0],
        ib: current[1],
        ic: current[2],
        vsenc: unsafe { adc_read(2) },
        neutral: unsafe { adc_read(3) },
        vbus: unsafe { adc_read(6) },
        vref: unsafe { adc_read(13) },
        theta: (theta >> 24) as u8,
        flags: feedback_flags(),
        on_a: on[0] as u8,
        on_b: on[1] as u8,
        on_c: on[2] as u8,
        stage,
    };
    unsafe {
        core::ptr::addr_of_mut!(CAPTURE)
            .cast::<Capture>()
            .add(index)
            .write_volatile(sample);
    }
    sample
}

fn capture_read(index: usize) -> Capture {
    unsafe {
        core::ptr::addr_of!(CAPTURE)
            .cast::<Capture>()
            .add(index)
            .read_volatile()
    }
}

fn coast_write(index: usize, tick: u32) {
    let sample = CoastCapture {
        tick: tick as u16,
        vsenc: unsafe { adc_read(2) },
        neutral: unsafe { adc_read(3) },
        vbus: unsafe { adc_read(6) },
        vref: unsafe { adc_read(13) },
        flags: feedback_flags(),
        _pad: 0,
    };
    unsafe {
        core::ptr::addr_of_mut!(COAST_CAPTURE)
            .cast::<CoastCapture>()
            .add(index)
            .write_volatile(sample);
    }
}

fn coast_read(index: usize) -> CoastCapture {
    unsafe {
        core::ptr::addr_of!(COAST_CAPTURE)
            .cast::<CoastCapture>()
            .add(index)
            .read_volatile()
    }
}

fn dump_capture<W: Write>(serial: &mut W, len: usize, head: usize, reason: u8, vcal: u32) {
    // Explicit header/footer and fixed-width raw hex follow minz's WAXWING
    // convention. Drive is already disabled before this function is called.
    let _ = writeln!(
        serial,
        "CAP n={} capacity={} head={} order=oldest_first sample_hz={} sample_point=control_tick_async_to_pwm adc_order=rotating_ABC_BCA_CAB vcal={} reason={} fields=tick,freq_chz,ia,ib,ic,vsenc,neutral,vbus,vref,theta,flags,on_a,on_b,on_c,stage",
        len, CAPTURE_N, head, CAPTURE_HZ, vcal, reason
    );
    let oldest = if len == CAPTURE_N { head } else { 0 };
    for logical in 0..len {
        let s = capture_read((oldest + logical) % CAPTURE_N);
        let _ = writeln!(
            serial,
            "{:04x} {:04x} {:04x} {:04x} {:04x} {:04x} {:04x} {:04x} {:04x} {:04x} {:04x} {:04x} {:04x} {:04x} {:04x}",
            s.tick,
            s.freq_chz,
            s.ia,
            s.ib,
            s.ic,
            s.vsenc,
            s.neutral,
            s.vbus,
            s.vref,
            s.theta as u16,
            s.flags as u16,
            s.on_a as u16,
            s.on_b as u16,
            s.on_c as u16,
            s.stage as u16,
        );
    }
    let _ = writeln!(serial, "CAP END");
}

fn dump_coast<W: Write>(serial: &mut W, len: usize, vcal: u32) {
    let _ = writeln!(
        serial,
        "COAST n={} sample_hz={} sample_point=bridge_disabled vcal={} fields=tick,vsenc,neutral,vbus,vref,flags",
        len, COAST_HZ, vcal,
    );
    for index in 0..len {
        let s = coast_read(index);
        let _ = writeln!(
            serial,
            "{:04x} {:04x} {:04x} {:04x} {:04x} {:04x}",
            s.tick, s.vsenc, s.neutral, s.vbus, s.vref, s.flags as u16,
        );
    }
    let _ = writeln!(serial, "COAST END");
}

// TIM17 1 MHz free-run timebase.
fn t17() -> u16 {
    unsafe { (*stm32::TIM17::ptr()).cnt().read().bits() as u16 }
}

// Gate bits: INH A=PA10 B=PA9 C=PA8 (all GPIOA); INL A=PB1 B=PB0 (GPIOB) C=PA7 (GPIOA).
const INH: [u32; 3] = [1 << 10, 1 << 9, 1 << 8];
fn gates_off() {
    unsafe {
        let tim = &*stm32::TIM1::ptr();
        tim.bdtr().modify(|r, w| w.bits(r.bits() & !(1 << 15))); // MOE=0
        tim.ccr1().write(|w| w.bits(0));
        tim.ccr2().write(|w| w.bits(0));
        tim.ccr3().write(|w| w.bits(0));
    }
    bsrr_a(0, (1 << 7) | (1 << 8) | (1 << 9) | (1 << 10));
    bsrr_b(0, (1 << 0) | (1 << 1));
}
fn pwm_moe(on: bool) {
    unsafe {
        let tim = &*stm32::TIM1::ptr();
        tim.bdtr().modify(|r, w| {
            let bits = if on {
                r.bits() | (1 << 15)
            } else {
                r.bits() & !(1 << 15)
            };
            w.bits(bits)
        });
    }
}
fn pwm_sine(duty_tenths: u32, theta: u32) -> [u16; 3] {
    let mut ccr = [0u32; 3];
    let mut on_us = [0u16; 3];
    for i in 0..3 {
        let ix = (((theta >> 24) + (i as u32) * 85) & 0xFF) as usize;
        ccr[i] = (PWM_ARR + 1) * duty_tenths * SINE_LUT[ix] as u32 / (1000 * 255);
        on_us[i] = ((ccr[i] * PWM_PERIOD_US + (PWM_ARR + 1) / 2) / (PWM_ARR + 1)) as u16;
    }
    unsafe {
        let tim = &*stm32::TIM1::ptr();
        // Native TIM1 pin pairing: CH3=A, CH2=B, CH1=C.
        tim.ccr3().write(|w| w.bits(ccr[0]));
        tim.ccr2().write(|w| w.bits(ccr[1]));
        tim.ccr1().write(|w| w.bits(ccr[2]));
    }
    on_us
}

// This board cannot use binz's EVLDRIVE panic safing map. Keep a local panic
// handler that clears all six DRV8304 inputs and ENABLE before halting.
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    cortex_m::interrupt::disable();
    gates_off();
    set_pin(3, 1, false);
    loop {
        cortex_m::asm::nop();
    }
}
fn lows_on() {
    bsrr_a(1 << 7, 0); // PA7 (INL C)
    bsrr_b((1 << 0) | (1 << 1), 0); // PB0/PB1 (INL B/A)
}
// One 1 kHz carrier period: pulse each active phase's high side for on[i] us.
// Each phase's low side is restored immediately after its own high pulse plus
// dead-time. This is load-bearing: waiting for the longest pulse would leave
// shorter phases Hi-Z and remove the intended line-to-line drive interval.
fn sine_pulse(on: [u16; 3]) {
    // INL off for active phases so INH can raise (avoids 6x INH=INL=1 -> Hi-Z).
    let mut ac = 0u32;
    let mut bc = 0u32;
    if on[0] > 0 {
        bc |= 1 << 1;
    }
    if on[1] > 0 {
        bc |= 1 << 0;
    }
    if on[2] > 0 {
        ac |= 1 << 7;
    }
    if ac != 0 {
        bsrr_a(0, ac);
    }
    if bc != 0 {
        bsrr_b(0, bc);
    }
    let g = t17();
    while t17().wrapping_sub(g) < 1 {}
    // INH on for active phases together.
    let mut ai = 0u32;
    for i in 0..3 {
        if on[i] > 0 {
            ai |= INH[i];
        }
    }
    let ps = t17();
    bsrr_a(ai, 0);
    // Turn each INH off at its on-time (sorted ascending), then restore that
    // phase's low side after 1 us dead-time. Equal deadlines switch as a group.
    let mut ord = [0usize, 1, 2];
    if on[ord[0]] > on[ord[1]] {
        ord.swap(0, 1);
    }
    if on[ord[1]] > on[ord[2]] {
        ord.swap(1, 2);
    }
    if on[ord[0]] > on[ord[1]] {
        ord.swap(0, 1);
    }
    let mut k = 0usize;
    while k < 3 {
        let i = ord[k];
        if on[i] == 0 {
            k += 1;
            continue;
        }
        let deadline = on[i];
        while t17().wrapping_sub(ps) < deadline {}

        let mut hi_clr = 0u32;
        let mut low_a_set = 0u32;
        let mut low_b_set = 0u32;
        while k < 3 && on[ord[k]] == deadline {
            let phase = ord[k];
            hi_clr |= INH[phase];
            match phase {
                0 => low_b_set |= 1 << 1, // PB1 = INLA
                1 => low_b_set |= 1 << 0, // PB0 = INLB
                _ => low_a_set |= 1 << 7, // PA7 = INLC
            }
            k += 1;
        }
        bsrr_a(0, hi_clr);
        let dead = t17();
        while t17().wrapping_sub(dead) < 1 {}
        if low_a_set != 0 {
            bsrr_a(low_a_set, 0);
        }
        if low_b_set != 0 {
            bsrr_b(low_b_set, 0);
        }
    }
}

#[entry]
fn main() -> ! {
    rtt_init_print!();
    let dp = stm32::Peripherals::take().unwrap();
    let cp = cortex_m::Peripherals::take().unwrap();
    let mut rcc = dp.RCC.freeze(Config::pll());

    let gpioa = dp.GPIOA.split(&mut rcc);
    let gpiob = dp.GPIOB.split(&mut rcc);
    let gpioc = dp.GPIOC.split(&mut rcc);
    let gpiod = dp.GPIOD.split(&mut rcc);

    let _ = (
        gpioa.pa0.into_analog(),
        gpioa.pa1.into_analog(),
        gpioa.pa2.into_analog(),
        gpioa.pa3.into_analog(),
        gpioa.pa4.into_analog(),
        gpioa.pa6.into_analog(),
        gpiob.pb3.into_analog(),
        gpiob.pb7.into_analog(),
    );
    let _ = (
        gpiob.pb14.into_floating_input(),
        gpioc.pc13.into_floating_input(),
    );
    let _ = (
        gpioa.pa5.into_push_pull_output(),
        gpioa.pa7.into_push_pull_output(),
        gpioa.pa8.into_push_pull_output(),
        gpioa.pa9.into_push_pull_output(),
        gpioa.pa10.into_push_pull_output(),
        gpiob.pb0.into_push_pull_output(),
        gpiob.pb1.into_push_pull_output(),
        gpiob.pb5.into_push_pull_output(),
        gpiod.pd1.into_push_pull_output(),
    );
    for (p, b) in [
        (0, 5),
        (0, 7),
        (0, 8),
        (0, 9),
        (0, 10),
        (1, 0),
        (1, 1),
        (1, 5),
        (3, 1),
    ] {
        set_pin(p, b, false);
    }

    // Put the six native TIM1 pins into AF2 only after their GPIO latches are
    // low, then configure the proven raw-register complementary PWM pattern.
    unsafe {
        let rcc_raw = &*stm32::RCC::ptr();
        rcc_raw
            .apbenr2()
            .modify(|r, w| w.bits(r.bits() | (1 << 11))); // TIM1EN
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
        let pb = &*stm32::GPIOB::ptr();
        pb.moder()
            .modify(|_, w| w.moder0().alternate().moder1().alternate());
        pb.afrl().modify(|_, w| w.afr(0).af2().afr(1).af2());

        let tim = &*stm32::TIM1::ptr();
        tim.cr1().write(|w| w.bits(0));
        tim.cr2().write(|w| w.bits(0));
        tim.psc().write(|w| w.bits(0));
        tim.arr().write(|w| w.bits(PWM_ARR));
        tim.rcr().write(|w| w.bits(0));
        tim.ccmr1_output().write(|w| w.bits(0x6868)); // CH1/2 PWM1 + preload
        tim.ccmr2_output().write(|w| w.bits(0x0068)); // CH3 PWM1 + preload
        tim.ccer().write(|w| w.bits(0x0555)); // main + complementary, active high
        tim.ccr1().write(|w| w.bits(0));
        tim.ccr2().write(|w| w.bits(0));
        tim.ccr3().write(|w| w.bits(0));
        tim.bdtr().write(|w| w.bits((1 << 11) | (1 << 10) | 26)); // OSSR|OSSI|~0.4 us DT, MOE=0
        tim.egr().write(|w| w.bits(1));
        tim.sr().write(|w| w.bits(0));
        tim.cr1().write(|w| w.bits(0x81)); // ARPE|CEN
    }

    let mut serial = dp
        .USART3
        .usart(
            (gpioc.pc10, gpioc.pc11),
            BasicConfig::default().baudrate(115_200.bps()),
            &mut rcc,
        )
        .unwrap();

    unsafe {
        let rcc_raw = &*stm32::RCC::ptr();
        rcc_raw.apbenr2().modify(|r, w| w.bits(r.bits() | (1 << 0))); // SYSCFGEN
        rcc_raw
            .apbenr2()
            .modify(|r, w| w.bits(r.bits() | (1 << 20))); // ADCEN
        rcc_raw
            .apbenr2()
            .modify(|r, w| w.bits(r.bits() | (1 << 18))); // TIM17EN
        // TIM17 = 1 MHz free-run.
        let t = &*stm32::TIM17::ptr();
        t.psc().write(|w| w.bits(63)); // 64 MHz / 64 = 1 MHz
        t.arr().write(|w| w.bits(0xFFFF));
        t.egr().write(|w| w.bits(1)); // UG latch PSC
        t.cr1().write(|w| w.bits(1)); // CEN
        let adc = &*stm32::ADC::ptr();
        adc.cfgr2().write(|w| w.bits(0b10 << 30));
        adc.cr().write(|w| w.bits(1 << 28));
        cortex_m::asm::delay(64 * 30);
        adc.cr().modify(|r, w| w.bits(r.bits() | (1 << 31)));
        while adc.cr().read().bits() & (1 << 31) != 0 {}
        cortex_m::asm::delay(64 * 5);
        adc.smpr().write(|w| w.bits(0b111));
        adc.ccr().modify(|r, w| w.bits(r.bits() | (1 << 22)));
        adc.isr().write(|w| w.bits(1));
        adc.cr().modify(|r, w| w.bits(r.bits() | 1));
        while adc.isr().read().bits() & 1 == 0 {}
        core::ptr::write_volatile(COMP2_CSR, (0b1000 << 4) | (0b10 << 8) | 1);
        cortex_m::asm::delay(320);
    }
    let vcal = unsafe { core::ptr::read_volatile(VREFINT_CAL_ADDR as *const u16) } as u32;

    let mut syst = cp.SYST;
    syst.set_clock_source(SystClkSource::Core);
    syst.set_reload(64_000 - 1);
    syst.clear_current();
    syst.enable_counter();

    let _ = writeln!(
        serial,
        "\r\n=== DRV8304 shell + raw TIM1 10kHz sine ===  ? for help"
    );
    while serial.flush().is_err() {}
    let _ = write!(serial, "> ");
    while serial.flush().is_err() {}
    rprintln!("shell-sine ready");

    let mut buf = [0u8; 32];
    let mut idx = 0usize;
    let mut millis: u32 = 0;
    let mut last_blink: u32 = 0;
    let mut blink = true;
    let mut bphase = false;

    // Drive mode: 0=idle, 1=fixed-frequency diagnostic, 2=startup campaign.
    let mut drive_mode: u8 = 0;
    let mut sfreq: u32 = 10; // electrical Hz
    let mut target_hz: u32 = 10;
    let mut max_on_us: u32 = DEFAULT_ON_US;
    let mut theta: u32 = 0;
    let mut last_edge: u16 = 0;
    let mut sine_ticks: u32 = 0;
    let mut capture_head: usize = 0;
    let mut capture_len: usize = 0;
    let mut coast = false;
    let mut coast_edge: u16 = 0;
    let mut coast_len: usize = 0;
    let mut dump_pending = false;
    let mut dump_reason: u8 = 0; // 1=timeout, 2=nFAULT, 3=host, 4=ADC rail, 5=VBUS UV

    loop {
        if syst.has_wrapped() {
            millis = millis.wrapping_add(1);
        }
        if blink && millis.wrapping_sub(last_blink) >= 500 {
            last_blink = millis;
            bphase = !bphase;
            set_pin(0, 5, bphase);
            set_pin(1, 5, !bphase);
        }

        // Update the sine envelope at 1 kHz. TIM1 independently emits exact
        // complementary 10 kHz pulses between these control updates.
        if drive_mode != 0 {
            let now = t17();
            if now.wrapping_sub(last_edge) >= 1000 {
                last_edge = now;
                let (freq_chz, effective_on_us, stage) = if drive_mode == 2 {
                    if sine_ticks < ALIGN_TICKS {
                        // A short, proven-safe 1% static vector establishes the
                        // initial electrical reference without stall-current dwell.
                        (0, START_ON_US.min(max_on_us), 1)
                    } else if sine_ticks < ALIGN_TICKS + START_TICKS {
                        (START_HZ * 100, CATCH_DUTY_TENTHS, 2)
                    } else if sine_ticks < ALIGN_TICKS + START_TICKS + RAMP_TICKS {
                        let elapsed = sine_ticks - ALIGN_TICKS - START_TICKS;
                        let target_chz = target_hz * 100;
                        let start_chz = START_HZ * 100;
                        let freq_chz = start_chz - (start_chz - target_chz) * elapsed / RAMP_TICKS;
                        let duty = if max_on_us <= CATCH_DUTY_TENTHS {
                            CATCH_DUTY_TENTHS
                                - (CATCH_DUTY_TENTHS - max_on_us) * elapsed / RAMP_TICKS
                        } else {
                            CATCH_DUTY_TENTHS
                                + (max_on_us - CATCH_DUTY_TENTHS) * elapsed / RAMP_TICKS
                        };
                        (freq_chz, duty, 3)
                    } else {
                        (target_hz * 100, max_on_us, 4)
                    }
                } else {
                    (sfreq * 100, max_on_us, 0)
                };
                let on = pwm_sine(effective_on_us, theta);
                if freq_chz != 0 {
                    let phase_inc = ((freq_chz as u64) * 4_294_967_296u64 / 100_000u64) as u32;
                    theta = theta.wrapping_add(phase_inc);
                }
                sine_ticks += 1;
                if !get_idr(1, 14) {
                    drive_mode = 0;
                    gates_off();
                    set_pin(3, 1, false);
                    dump_reason = 2;
                    coast = true;
                    coast_edge = t17();
                    coast_len = 0;
                    let _ = writeln!(serial, "\r\nnFAULT -> gates + en OFF; coast capture");
                    let _ = write!(serial, "> ");
                } else if sine_ticks % CAPTURE_DIV == 0 {
                    let sample =
                        capture_write(capture_head, sine_ticks, freq_chz, theta, on, stage);
                    capture_head = (capture_head + 1) % CAPTURE_N;
                    capture_len = (capture_len + 1).min(CAPTURE_N);

                    let adc_railed = sample.ia == 0
                        || sample.ib == 0
                        || sample.ic == 0
                        || sample.ia >= 4095
                        || sample.ib >= 4095
                        || sample.ic >= 4095;
                    let vdda_mv = if sample.vref > 0 {
                        3000 * vcal / sample.vref as u32
                    } else {
                        0
                    };
                    let vbus_mv = sample.vbus as u32 * vdda_mv / 4096 * 1194 / 100;
                    if sample.flags & 1 == 0 {
                        drive_mode = 0;
                        gates_off();
                        set_pin(3, 1, false);
                        dump_reason = 2;
                        coast = true;
                        coast_edge = t17();
                        coast_len = 0;
                        let _ = writeln!(
                            serial,
                            "\r\nnFAULT during feedback scan -> gates + en OFF; coast capture"
                        );
                        let _ = write!(serial, "> ");
                    } else if adc_railed {
                        drive_mode = 0;
                        gates_off();
                        set_pin(3, 1, false);
                        dump_reason = 4;
                        coast = true;
                        coast_edge = t17();
                        coast_len = 0;
                        let _ = writeln!(
                            serial,
                            "\r\ncurrent ADC rail -> gates + en OFF; coast capture"
                        );
                        let _ = write!(serial, "> ");
                    } else if vbus_mv < 6000 {
                        drive_mode = 0;
                        gates_off();
                        set_pin(3, 1, false);
                        dump_reason = 5;
                        coast = true;
                        coast_edge = t17();
                        coast_len = 0;
                        let _ =
                            writeln!(serial, "\r\nVBUS below 6V -> gates + en OFF; coast capture");
                        let _ = write!(serial, "> ");
                    }
                }
                if drive_mode != 0 && sine_ticks >= RUN_TICKS {
                    drive_mode = 0;
                    gates_off();
                    set_pin(3, 1, false);
                    dump_reason = 1;
                    coast = true;
                    coast_edge = t17();
                    coast_len = 0;
                    let _ = writeln!(
                        serial,
                        "\r\n5s energized limit -> gates + en OFF; coast capture"
                    );
                    let _ = write!(serial, "> ");
                }
            }
        }

        if coast {
            let now = t17();
            if now.wrapping_sub(coast_edge) >= COAST_PERIOD_US {
                coast_edge = now;
                coast_write(coast_len, coast_len as u32);
                coast_len += 1;
                if coast_len >= COAST_N {
                    coast = false;
                    dump_pending = true;
                }
            }
        }

        if dump_pending && drive_mode == 0 && !coast {
            dump_pending = false;
            dump_capture(&mut serial, capture_len, capture_head, dump_reason, vcal);
            dump_coast(&mut serial, coast_len, vcal);
            capture_len = 0;
            capture_head = 0;
            coast_len = 0;
            let _ = write!(serial, "> ");
            while serial.flush().is_err() {}
        }

        // Drain ALL available UART bytes each pass (no overrun).
        while let Ok(b) = serial.read() {
            if b == b'\r' || b == b'\n' {
                let line = &buf[..idx];
                let _ = writeln!(serial, "");
                if idx == 0 {
                } else if line == b"?" || line == b"help" {
                    let _ = writeln!(
                        serial,
                        "a r<n> c i p blink off | sine sf<hz> du<0.1pct> run<hz> | pin=0/1: ah bh ch al bl cl led ld4 en"
                    );
                } else if line == b"a" {
                    let vref = unsafe { adc_read(13) } as u32;
                    let vdda = if vref > 0 { 3000 * vcal / vref } else { 0 };
                    let mv = |raw: u16| raw as u32 * vdda / 4096;
                    let ia = mv(unsafe { adc_read(0) });
                    let ib = mv(unsafe { adc_read(1) });
                    let ic = mv(unsafe { adc_read(4) });
                    let vbus = mv(unsafe { adc_read(6) }) * 1194 / 100;
                    let vsc = mv(unsafe { adc_read(2) });
                    let neu = mv(unsafe { adc_read(3) });
                    let off = (vdda / 2) as i32;
                    let ma = |m: u32| (m as i32 - off) * 1000 / 70;
                    let _ = writeln!(serial, "VDDA={}mV VBUS={}mV", vdda, vbus);
                    let _ = writeln!(
                        serial,
                        "IA={}mV({}mA) IB={}mV({}mA) IC={}mV({}mA)",
                        ia,
                        ma(ia),
                        ib,
                        ma(ib),
                        ic,
                        ma(ic)
                    );
                    let _ = writeln!(serial, "VSENC={}mV NEU={}mV", vsc, neu);
                } else if line.starts_with(b"run") {
                    let mut n = 0u32;
                    let mut any = false;
                    for &c in &line[3..] {
                        if c.is_ascii_digit() {
                            n = n * 10 + (c - b'0') as u32;
                            any = true;
                        }
                    }
                    if any && n >= 1 && n <= 50 && drive_mode == 0 && !coast {
                        target_hz = n;
                        gates_off();
                        set_pin(3, 1, true);
                        cortex_m::asm::delay(64_000); // DRV wake before applying lows
                        if get_idr(1, 14) {
                            theta = 0;
                            pwm_sine(START_ON_US.min(max_on_us), theta);
                            pwm_moe(true);
                            last_edge = t17();
                            sine_ticks = 0;
                            capture_head = 0;
                            capture_len = 0;
                            coast_len = 0;
                            dump_pending = false;
                            dump_reason = 0;
                            drive_mode = 2;
                            let _ = writeln!(
                                serial,
                                "RUN: align={}ms@{}.{:01}% catch={}Hz/{}ms@{}.{:01}% ramp={}ms target={}Hz/{}.{:01}% hold={}ms",
                                ALIGN_TICKS,
                                START_ON_US.min(max_on_us) / 10,
                                START_ON_US.min(max_on_us) % 10,
                                START_HZ,
                                START_TICKS,
                                CATCH_DUTY_TENTHS / 10,
                                CATCH_DUTY_TENTHS % 10,
                                RAMP_TICKS,
                                target_hz,
                                max_on_us / 10,
                                max_on_us % 10,
                                HOLD_TICKS,
                            );
                        } else {
                            gates_off();
                            set_pin(3, 1, false);
                            let _ = writeln!(serial, "RUN refused: nFAULT low; gates + en OFF");
                        }
                    } else if drive_mode != 0 || coast {
                        let _ = writeln!(serial, "?busy; use off first");
                    } else {
                        let _ = writeln!(serial, "?run <1..50 Hz>");
                    }
                } else if line[0] == b'r' && !line.starts_with(b"rf") {
                    let mut n = 0u32;
                    let mut any = false;
                    for &c in &line[1..] {
                        if c.is_ascii_digit() {
                            n = n * 10 + (c - b'0') as u32;
                            any = true;
                        }
                    }
                    if any && n <= 18 {
                        let vref = unsafe { adc_read(13) } as u32;
                        let vdda = if vref > 0 { 3000 * vcal / vref } else { 0 };
                        let raw = unsafe { adc_read(n as u8) };
                        let _ =
                            writeln!(serial, "ch{} raw={} {}mV", n, raw, raw as u32 * vdda / 4096);
                    } else {
                        let _ = writeln!(serial, "?r <0..18>");
                    }
                } else if line == b"c" {
                    let s = |x: bool| if x { "hi" } else { "lo" };
                    let _ = writeln!(
                        serial,
                        "BEMF: A={} B={} C={}",
                        s(comp_read(6)),
                        s(comp_read(7)),
                        s(comp_read(8))
                    );
                } else if line == b"i" {
                    let _ = write!(serial, "IN:");
                    for (nm, p, b) in ALL_PINS {
                        let _ = write!(serial, " {}={}", nm, if get_idr(p, b) { 1 } else { 0 });
                    }
                    let _ = writeln!(serial, "");
                } else if line == b"p" {
                    let g = |p, b| if get_odr(p, b) { '1' } else { '0' };
                    let tim = unsafe { &*stm32::TIM1::ptr() };
                    let moe = (tim.bdtr().read().bits() >> 15) & 1;
                    let _ = writeln!(
                        serial,
                        "OUT: ah={} bh={} ch={} al={} bl={} cl={} led={} ld4={} en={} | mode={} coast={} sf={}Hz target={}Hz duty={}.{:01}% TIM1:moe={} ccrA={} ccrB={} ccrC={} arr={}",
                        g(0, 10),
                        g(0, 9),
                        g(0, 8),
                        g(1, 1),
                        g(1, 0),
                        g(0, 7),
                        g(1, 5),
                        g(0, 5),
                        g(3, 1),
                        drive_mode,
                        if coast { 1 } else { 0 },
                        sfreq,
                        target_hz,
                        max_on_us / 10,
                        max_on_us % 10,
                        moe,
                        tim.ccr3().read().bits(),
                        tim.ccr2().read().bits(),
                        tim.ccr1().read().bits(),
                        tim.arr().read().bits(),
                    );
                } else if line == b"blink" {
                    blink = !blink;
                    let _ = writeln!(serial, "blink {}", if blink { "on" } else { "off" });
                } else if line == b"off" {
                    let was_driving = drive_mode != 0;
                    drive_mode = 0;
                    gates_off();
                    set_pin(3, 1, false);
                    if was_driving && capture_len > 0 {
                        dump_reason = 3;
                        dump_pending = false;
                        coast = true;
                        coast_edge = t17();
                        coast_len = 0;
                        let _ = writeln!(serial, "gates + en OFF; host-stop coast capture");
                    } else if coast {
                        coast = false;
                        dump_pending = capture_len > 0;
                        let _ = writeln!(serial, "gates + en OFF; coast stopped");
                    } else {
                        let _ = writeln!(serial, "gates + en OFF, idle");
                    }
                } else if line == b"sine" {
                    if drive_mode == 0 && !coast {
                        gates_off();
                        set_pin(3, 1, true);
                        cortex_m::asm::delay(64_000);
                        if !get_idr(1, 14) {
                            gates_off();
                            set_pin(3, 1, false);
                            let _ = writeln!(serial, "SINE refused: nFAULT low; gates + en OFF");
                        } else {
                            drive_mode = 1;
                            theta = 0;
                            pwm_sine(max_on_us, theta);
                            pwm_moe(true);
                            last_edge = t17();
                            sine_ticks = 0;
                            capture_head = 0;
                            capture_len = 0;
                            coast_len = 0;
                            dump_pending = false;
                            dump_reason = 0;
                            let _ = writeln!(
                                serial,
                                "SINE on: TIM1 10kHz carrier, {}.{:01}% max, {}Hz, en=1, 5s + coast capture",
                                max_on_us / 10,
                                max_on_us % 10,
                                sfreq
                            );
                        }
                    } else {
                        if capture_len > 0 {
                            dump_reason = 3;
                            dump_pending = true;
                        }
                        drive_mode = 0;
                        coast = false;
                        gates_off();
                        set_pin(3, 1, false);
                        let _ = writeln!(serial, "SINE off: gates + en OFF");
                    }
                } else if line.starts_with(b"du") {
                    let mut n = 0u32;
                    let mut any = false;
                    for &c in &line[2..] {
                        if c.is_ascii_digit() {
                            n = n * 10 + (c - b'0') as u32;
                            any = true;
                        }
                    }
                    if any && n >= 1 && n <= MAX_ON_US_LIMIT {
                        max_on_us = n;
                        let _ = writeln!(
                            serial,
                            "du={} tenths-percent ({}.{:01}%)",
                            max_on_us,
                            max_on_us / 10,
                            max_on_us % 10
                        );
                    } else {
                        let _ = writeln!(serial, "?du <1..100 tenths-percent>");
                    }
                } else if line.starts_with(b"sf") {
                    let mut n = 0u32;
                    let mut any = false;
                    for &c in &line[2..] {
                        if c.is_ascii_digit() {
                            n = n * 10 + (c - b'0') as u32;
                            any = true;
                        }
                    }
                    if any && n >= 1 && n <= 500 {
                        sfreq = n;
                        let _ = writeln!(serial, "sf={}Hz", sfreq);
                    } else {
                        let _ = writeln!(serial, "?sf <1..500>");
                    }
                } else if let Some(eq) = line.iter().position(|&c| c == b'=') {
                    let name = &line[..eq];
                    let val = line.get(eq + 1).copied().unwrap_or(b'0');
                    let on = val == b'1' || val == b'h';
                    match pin_lookup(name) {
                        Some((p, b)) => {
                            if name == b"led" || name == b"ld4" {
                                blink = false;
                            }
                            set_pin(p, b, on);
                            let _ = writeln!(
                                serial,
                                "{}={}",
                                core::str::from_utf8(name).unwrap_or("?"),
                                if on { 1 } else { 0 }
                            );
                        }
                        None => {
                            let _ = writeln!(serial, "?pin");
                        }
                    }
                } else {
                    let _ = writeln!(serial, "?cmd");
                }
                idx = 0;
                let _ = write!(serial, "> ");
            } else if idx < buf.len() - 1 && b >= 0x20 {
                buf[idx] = b;
                idx += 1;
                let _ = serial.write(b);
            }
        }
    }
}
