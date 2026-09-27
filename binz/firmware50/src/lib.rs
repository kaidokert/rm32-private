//! firmware50 — standalone typed firmware for the NUCLEO-G071RB + BOOSTXL-DRV8304H
//! sensorless BLDC bench, reproducing the qualified behavior to ~10% throttle.
//!
//! Design rules (from FIRMWARE_CRATE_REBUILD_TASK.md):
//! - Policy/personality is expressed as **types**, composed once into a canonical
//!   `Production` controller. No Cargo feature matrix; `#[cfg]` stays structural
//!   (target HAL glue / absent peripherals / a separate diagnostic binary).
//! - Protections are non-negotiable and always present in the composition:
//!   signed-average current foldback, fast bus-sag three-scan hard stop, nFAULT,
//!   tracking, watchdog, all-off.
//! - Nothing reachable from the four motor ISR roots (COMP, COM, DMA, guard) may
//!   use soft division (`__aeabi_uidiv`/`uidivmod`) or an unbounded loop.
//!
//! Host-testable policy/protection state machines live here in `src/`; the target
//! entry points live in `bin/`.

#![no_std]
#![deny(unsafe_op_in_unsafe_fn)]
// Unsafe-related lints, enforced on every clippy run (goal item 6): the
// pedantic pointer/cast lints, and every `unsafe` block documented, one
// operation each.
#![warn(
    clippy::borrow_as_ptr,
    clippy::cast_ptr_alignment,
    clippy::missing_safety_doc,
    clippy::multiple_unsafe_ops_per_block,
    clippy::ptr_as_ptr,
    clippy::ptr_cast_constness,
    clippy::transmute_ptr_to_ptr,
    clippy::undocumented_unsafe_blocks,
    clippy::unnecessary_safety_comment,
    clippy::unnecessary_safety_doc
)]

#[cfg(test)]
extern crate std;

pub mod acquire;
pub mod bemf;
pub mod bridge;
pub mod capture;
pub mod chain;
pub mod command;
pub mod commutation;
pub mod driven;
pub mod duty;
pub mod fine;
pub mod fixed;
/// Every register access, behind safe functions (target-only HAL glue).
#[cfg(target_os = "none")]
pub mod hw;
pub mod oneshot;
pub mod ordertrace;
pub mod protection;
pub mod ramp;
pub mod rate;
pub mod report;
pub mod restart;
pub mod revisit;
/// The motor ISR roots' logic and wiring (target-only).
#[cfg(target_os = "none")]
pub mod roots;
pub mod run;
pub mod sagtrace;
pub mod seed;
pub mod shared;
pub mod sine;
pub mod sixstep;
pub mod startup;
pub mod tracking;
pub mod witness;
