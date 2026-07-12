//! minz-core — the extracted, host-testable brain of the minz bench
//! firmware (`minz/examples/motor_tester2.rs`).
//!
//! Everything here is pure logic: state structs plus data-in /
//! data-out methods. Timestamps, ADC samples, and comparator levels
//! arrive as *arguments*; nothing reads hardware. That seam is what
//! lets the exact code that runs in the ISRs be unit-tested on the
//! host — the design goal being to convict logic bugs in
//! milliseconds on a PC instead of on a bench at the price of burnt
//! motors (see the 2026-07-10 post-mortem).
//!
//! Every module carries regression tests for the specific bugs this
//! project hit on hardware; the test names cite the incidents.

#![cfg_attr(not(test), no_std)]

pub mod a85;
pub mod blackbox;
pub mod drive;
pub mod dump;
pub mod estimator;
pub mod guards;
pub mod rates;
pub mod sense;
pub mod throttle;
pub mod ticks;
pub mod timing;
pub mod ui;
pub mod window;
pub mod wire;
pub mod zc;
