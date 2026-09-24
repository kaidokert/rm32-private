//! Every hardware register access firmware50 makes, behind safe functions.
//!
//! Target-only (`#[cfg(target_os = "none")]` in `lib.rs`): this is the
//! structural HAL glue the crate rules allow, and the only place a PAC
//! register block is dereferenced. Callers in the binary never touch a
//! register, never write `.bits()`, and never write `unsafe` for hardware.
//!
//! Conventions, so the goal's rule "named-field PAC modify/write" holds:
//!
//! * a field is written through its named writer: `.set(v)` where the PAC
//!   marks the field `Safe`, `.variant()`/named methods where it is
//!   enumerated, `.set_bit()`/`.clear_bit()` for single bits;
//! * a register returned to its reset value uses `.reset()`;
//! * a register block is obtained only through the PAC's own `ptr()` constant
//!   -- no address literal appears in this crate.
//!
//! Every accessor is `#[inline(always)]`: the four motor ISR roots call into
//! this module, and the fail-closed audit's cycle counts may not grow.
//!
//! Why the PAC and not the HAL, per peripheral, is stated where each block
//! is obtained (the reasons were established in the binary's history and are
//! kept verbatim there until each group moves here).

pub mod adc;
pub mod comp;
pub mod gpio;
pub mod nvic;
pub mod pwm;
pub mod system;
pub mod timers;

pub use timers::{clock, com_timer, pace};
