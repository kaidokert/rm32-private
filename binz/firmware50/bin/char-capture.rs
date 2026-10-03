//! Measurement image for the battery characterization study (2026-10-01).
//!
//! Production's controller, protections and roots, with the three
//! `firmware50::charz` instruments installed through the two recorder seams
//! production leaves empty: `CharRec` as the `SagLog` (duty schedule +
//! time-series recorder) and `CharChain` as both roots' `ChainLog` (per-sector
//! interval histograms, service lateness, tail rings). Every protection is the
//! production policy, unchanged.
//!
//! Protocol (one wire format for every run):
//!
//! * `{period settle_ms post_ms duty,slew,hold duty,slew,hold ...}` installs a
//!   profile and echoes `PROFILE ...`. Slew is tenths per second, `0` a step,
//!   `65535` the production staircase.
//! * `P` runs it: one production rung whose target is the schedule's highest
//!   duty, with the schedule replacing the ramp after the loop closes; the run
//!   ends at the schedule's end (reason 2) or at any protection stop. The normal
//!   report follows, then the `CHAR*` dump (`charz::emit`), after `safe_off`.
//! * Every other key is the production shell.

#![no_std]
#![no_main]
#![deny(unsafe_op_in_unsafe_fn)]

mod board;

use cortex_m_rt::entry;
use firmware50::bridge::safe_off;
use firmware50::capture::NoLog;
use firmware50::charz::{self, CharChain, CharRec, MAX_WP, Profile, Waypoint};
use firmware50::report::Sink;
use firmware50::roots::{self, Drv8304};
use firmware50::run::policy;
use firmware50::run::{Controller, Hal};
use stm32g0xx_hal::stm32;
use stm32g0xx_hal::stm32::interrupt;

#[interrupt]
fn TIM16() {
    // SAFETY: the PAC's vector `TIM16 = 21` ("21 - TIM16 global interrupt"), at `COM_IRQ_PRIORITY`.
    unsafe { roots::com_root::<CharChain>() }
}

#[interrupt]
fn ADC_COMP() {
    // SAFETY: the PAC's vector `ADC_COMP = 12` ("12 - ADC and COMP interrupts"), at `COMP_IRQ_PRIORITY`.
    unsafe { roots::comp_root::<NoLog, CharChain>() }
}

type CharProduction = Controller<
    policy::Wiring,
    policy::BemfPolicy,
    policy::AdvancePolicy,
    policy::CurrentProtection,
    policy::BusSagProtection,
    policy::Restart,
    policy::Telemetry,
    CharRec,
>;

/// Parse `period settle post d,s,h ...` (the text between the braces).
fn parse(text: &[u8]) -> Option<Profile> {
    let mut nums = [0u32; 3 + 3 * MAX_WP];
    let mut n = 0;
    let mut cur: Option<u32> = None;
    for &c in text {
        if c.is_ascii_digit() {
            cur = Some(cur.unwrap_or(0).checked_mul(10)?.checked_add(u32::from(c - b'0'))?);
        } else if c == b' ' || c == b',' {
            if let Some(v) = cur.take() {
                if n >= nums.len() {
                    return None;
                }
                nums[n] = v;
                n += 1;
            }
        } else {
            return None;
        }
    }
    if let Some(v) = cur.take() {
        if n >= nums.len() {
            return None;
        }
        nums[n] = v;
        n += 1;
    }
    if n < 6 || (n - 3) % 3 != 0 {
        return None;
    }
    let mut p = Profile::EMPTY;
    p.period_scans = u16::try_from(nums[0]).ok()?.max(1);
    p.settle_ms = nums[1];
    p.post_ms = nums[2];
    p.n = (n - 3) / 3;
    for i in 0..p.n {
        let duty = u16::try_from(nums[3 + 3 * i]).ok()?;
        if duty > policy::SIXSTEP_DUTY_CAP || duty < 30 {
            return None;
        }
        p.wp[i] = Waypoint {
            duty,
            slew_tps: u16::try_from(nums[4 + 3 * i]).ok()?,
            hold_ms: nums[5 + 3 * i],
        };
    }
    Some(p)
}

#[entry]
fn main() -> ! {
    let Some((mut board, adc_ok)) = stm32::Peripherals::take().and_then(board::init) else {
        loop {
            cortex_m::asm::nop();
        }
    };
    firmware50::hw::fine::init();
    safe_off(&mut Drv8304);
    board::banner(&mut board, adc_ok);
    board.say("CHARCAPTURE measurement image: {profile} then P\r\n");
    board.tx_flush();
    let mut p = CharProduction::new();
    let mut profile: Option<Profile> = None;
    let mut buf = [0u8; 160];
    loop {
        board.now();
        board.drain();
        let Some(b) = board.rx() else { continue };
        if b == b'{' {
            let mut len = 0;
            let ok = loop {
                board.now();
                board.drain();
                let Some(c) = board.rx() else { continue };
                if c == b'}' {
                    break true;
                }
                if len == buf.len() {
                    break false;
                }
                buf[len] = c;
                len += 1;
            };
            profile = if ok { parse(&buf[..len]) } else { None };
            match &profile {
                Some(pr) => {
                    board.say("PROFILE ");
                    board.kv("n", pr.n as u32);
                    board.kv("period_scans", u32::from(pr.period_scans));
                    board.kv("settle_ms", pr.settle_ms);
                    board.kv("post_ms", pr.post_ms);
                    for w in &pr.wp[..pr.n] {
                        board.say("wp=");
                        board.say_u32(u32::from(w.duty));
                        board.say(",");
                        board.say_u32(u32::from(w.slew_tps));
                        board.say(",");
                        board.say_u32(w.hold_ms);
                        board.say(" ");
                    }
                    board.say("\r\n");
                }
                None => board.say("PROFILE REFUSED\r\n"),
            }
            board.tx_flush();
            continue;
        }
        if b == b'P' {
            let Some(pr) = profile else {
                board.say("PROFILE NONE\r\n");
                board.tx_flush();
                continue;
            };
            charz::arm(&pr);
            let t0 = board.now();
            let _ = p.rung(&mut board, pr.max_duty(), None, 0, policy::BEMF_TOTAL_MS, None);
            charz::emit(&mut Echo(&mut board), t0);
            continue;
        }
        p.command(&mut board, b);
    }
}

struct Echo<'a>(&'a mut board::Board);

impl Sink for Echo<'_> {
    fn put(&mut self, b: u8) {
        self.0.put(b);
    }
    fn flush(&mut self) {
        self.0.tx_flush();
    }
}
