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
use crate::hal::gpio::gpioa::{PA7, PA8, PA9, PA10};
use crate::hal::gpio::gpiob::{PB0, PB1};
use crate::hal::gpio::{Afr, H8, L8, MODER, OTYPER};
use crate::hal::prelude::*;
use crate::hal::rcc::{AHB1, AHB2, AHB3, APB1R1, APB1R2, APB2, CCIPR, Clocks, Rcc};
use crate::hal::stm32::{FLASH, PWR, RCC};
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
    pub ccipr: CCIPR,
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
/// Takes the already-split pins plus the four GPIOA register handles
/// and three GPIOB register handles. Generic over each pin's current
/// mode so callers can pass pins straight out of `split()` (default
/// `Analog`) or pins that have already been touched. The configured
/// pins are dropped — TIM1 drives the AF, so the typestate guard isn't
/// needed at the call site.
///
/// Split-friendly variant: the caller does its own
/// `GPIOA.split(...)` / `GPIOB.split(...)` first so other pins on
/// those ports (UART TX/RX, soft-UART RX, etc.) remain available.
#[allow(clippy::too_many_arguments)]
pub fn configure_motor_pwm_pins<M0, M1, M2, M3, M4, M5>(
    pa7: PA7<M0>,
    pa8: PA8<M1>,
    pa9: PA9<M2>,
    pa10: PA10<M3>,
    pb0: PB0<M4>,
    pb1: PB1<M5>,
    a_moder: &mut MODER<'A'>,
    a_otyper: &mut OTYPER<'A'>,
    a_afrl: &mut Afr<L8, 'A'>,
    a_afrh: &mut Afr<H8, 'A'>,
    b_moder: &mut MODER<'B'>,
    b_otyper: &mut OTYPER<'B'>,
    b_afrl: &mut Afr<L8, 'B'>,
) {
    let _lin1 = pb1.into_alternate::<1>(b_moder, b_otyper, b_afrl);
    let _hin1 = pa10.into_alternate::<1>(a_moder, a_otyper, a_afrh);
    let _lin2 = pb0.into_alternate::<1>(b_moder, b_otyper, b_afrl);
    let _hin2 = pa9.into_alternate::<1>(a_moder, a_otyper, a_afrh);
    let _lin3 = pa7.into_alternate::<1>(a_moder, a_otyper, a_afrl);
    let _hin3 = pa8.into_alternate::<1>(a_moder, a_otyper, a_afrh);
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
        ccipr,
        ..
    } = rcc.constrain();
    let mut pwr = pwr.constrain(&mut apb1r1);
    let clocks = cfgr.sysclk(SYSCLK).freeze(&mut flash.acr, &mut pwr);
    // PRFTEN: flash prefetch ON. The HAL leaves it off (and rm32
    // keeps it off for AM32 register parity — an rm32 constraint,
    // not a minz one). With PRFTEN off + 4 wait states, hot-ISR
    // fetch timing depends on code ALIGNMENT — layout-only changes
    // (even init-code moves) measurably shifted the marginal
    // engage regime (2026-07-12 piecewise-rewire evidence: an
    // init-only E5 step dropped engage@15 from 8/8 to 3-4/8).
    unsafe {
        (*crate::hal::stm32::FLASH::ptr())
            .acr
            .modify(|_, w| w.prften().set_bit());
    }
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
        ccipr,
    }
}
