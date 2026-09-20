#![no_std]

// The minz-core control brain (BEMF ZC, commutation, desync policy,
// filter/blank/advance, state machine) is available on this M0+ target via
// portable-atomic. Re-exported for the closed-loop examples to consume.
pub use minz_core;

// NUCLEO-G071RB bringup crate. Examples link this lib (`use binz as _;`) to
// pick up the panic handler; call `rtt_init_print!()` first thing in main so
// panics are visible.
//
// Instrument spine (see CLAUDE.md):
//   stage    - the one shared stage-safe primitive (all kills route here)
//   harvest  - 10 kHz TIM6->ADC->DMA scan, ISR-class guards, TIM17 timebase
//   blackbox - 64-event flight recorder, dumped on any kill
//   telem    - 22-byte binary VCOM frame + non-blocking TX ring

pub mod blackbox;
pub mod harvest;
pub mod mzhal;
pub mod stage;
pub mod telem;

use core::panic::PanicInfo;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    stage::force_safe(); // a panic is an exit path: gates off first
    rtt_target::rprintln!("PANIC: {}", info);
    loop {
        cortex_m::asm::nop();
    }
}
