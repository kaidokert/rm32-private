//! Bench-only tooling for the rm32 firmware (not part of the motor-control
//! core): the bench UART command parser, the time-series recorder and the
//! event blackbox. Firmware links it only from its bench builds.

#![no_std]

pub mod bench_input;
pub mod bench_rec;
pub mod blackbox;
