//! Shared bench board init for `minz` examples.
//!
//! The chunk of boilerplate every example would otherwise repeat verbatim:
//! take the Cortex + PAC peripherals, constrain `FLASH/RCC/PWR`, configure
//! sysclk to [`crate::SYSCLK`] (80 MHz), call `panic::ensure_rtt()`. Pattern
//! cribbed from the winc-rs `feather/src/init.rs` `InitResult` shape — the
//! function takes the bits of `dp` it needs, returns what the caller still
//! wants (`cp`, configured `clocks`, the constrained `rcc`).
//!
//! Why not just take all of `dp`? Because each example uses a *different*
//! subset of `dp.GPIOx/TIMx/EXTI/SYSCFG/...`. Letting the caller keep `dp`
//! (partially-moved after handing `FLASH/RCC/PWR` to us) is more flexible
//! than enumerating every peripheral in the return struct.
//!
//! Usage:
//!
//! ```ignore
//! let cp = cortex_m::Peripherals::take().unwrap();
//! let dp = stm32::Peripherals::take().unwrap();
//! let BoardInit { cp, clocks, mut rcc } =
//!     minz::board_init::init(cp, dp.FLASH, dp.RCC, dp.PWR);
//!
//! // `dp` is partially moved but its other fields still work:
//! let mut gpioa = dp.GPIOA.split(&mut rcc.ahb2);
//! let timer    = Timer::tim2(dp.TIM2, freq, clocks, &mut rcc.apb1r1);
//! ```

use cortex_m::Peripherals as CortexPeripherals;

use crate::SYSCLK;
use crate::hal::prelude::*;
use crate::hal::rcc::{AHB1, AHB2, AHB3, APB1R1, APB1R2, APB2, Clocks, Rcc};
use crate::hal::stm32::{FLASH, GPIOA, GPIOB, PWR, RCC};
use crate::panic;

/// What `init()` hands back. We can't return the whole `Rcc` because
/// `cfgr.sysclk().freeze()` moves `rcc.cfgr` out, leaving `rcc` partially
/// moved. Instead we expose each bus marker individually — they're zero-sized
/// in the HAL, so this costs nothing at runtime and lets the caller use
/// whichever bus they need without owning the rest of `Rcc`.
pub struct BoardInit {
    pub cp: CortexPeripherals,
    pub clocks: Clocks,
    pub ahb1: AHB1,
    pub ahb2: AHB2,
    pub ahb3: AHB3,
    pub apb1r1: APB1R1,
    pub apb1r2: APB1R2,
    pub apb2: APB2,
}

/// Drive the six TIM1 motor-PWM pins (HIN1-3 / LIN1-3) to AF1.
///
/// Pin map (AM32-compatible):
///
/// | Net  | Pin  | TIM1 channel  |
/// |------|------|---------------|
/// | LIN1 | PB1  | TIM1_CH3N     |
/// | HIN1 | PA10 | TIM1_CH3      |
/// | LIN2 | PB0  | TIM1_CH2N     |
/// | HIN2 | PA9  | TIM1_CH2      |
/// | LIN3 | PA7  | TIM1_CH1N     |
/// | HIN3 | PA8  | TIM1_CH1      |
///
/// Takes raw PAC handles plus `&mut AHB2` (from `BoardInit`) and does the
/// split internally so the caller doesn't repeat 6 `into_alternate(...)`
/// lines verbatim. Consumes `GPIOA`/`GPIOB`; if a caller also wants other
/// pins from those ports, configure them before calling this.
pub fn configure_motor_pwm_pins(gpioa: GPIOA, gpiob: GPIOB, ahb2: &mut AHB2) {
    let mut gpioa = gpioa.split(ahb2);
    let mut gpiob = gpiob.split(ahb2);
    let _lin1 = gpiob
        .pb1
        .into_alternate::<1>(&mut gpiob.moder, &mut gpiob.otyper, &mut gpiob.afrl);
    let _hin1 =
        gpioa
            .pa10
            .into_alternate::<1>(&mut gpioa.moder, &mut gpioa.otyper, &mut gpioa.afrh);
    let _lin2 = gpiob
        .pb0
        .into_alternate::<1>(&mut gpiob.moder, &mut gpiob.otyper, &mut gpiob.afrl);
    let _hin2 = gpioa
        .pa9
        .into_alternate::<1>(&mut gpioa.moder, &mut gpioa.otyper, &mut gpioa.afrh);
    let _lin3 = gpioa
        .pa7
        .into_alternate::<1>(&mut gpioa.moder, &mut gpioa.otyper, &mut gpioa.afrl);
    let _hin3 = gpioa
        .pa8
        .into_alternate::<1>(&mut gpioa.moder, &mut gpioa.otyper, &mut gpioa.afrh);
}

/// Common bench setup: sysclk to [`SYSCLK`], RTT panic hook armed.
pub fn init(cp: CortexPeripherals, flash: FLASH, rcc: RCC, pwr: PWR) -> BoardInit {
    let mut flash = flash.constrain();
    let Rcc {
        ahb1,
        ahb2,
        ahb3,
        mut apb1r1,
        apb1r2,
        apb2,
        cfgr,
        ..
    } = rcc.constrain();
    let mut pwr = pwr.constrain(&mut apb1r1);
    let clocks = cfgr.sysclk(SYSCLK).freeze(&mut flash.acr, &mut pwr);
    panic::ensure_rtt();
    BoardInit {
        cp,
        clocks,
        ahb1,
        ahb2,
        ahb3,
        apb1r1,
        apb1r2,
        apb2,
    }
}
