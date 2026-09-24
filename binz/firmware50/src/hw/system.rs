//! The independent watchdog's timeout and the reset-cause flags.

use stm32g0xx_hal::stm32;

/// Why this boot happened: RCC_CSR's reset flags.
#[derive(Clone, Copy, Debug, Default)]
pub struct ResetCause {
    pub iwdg: bool,
    pub wwdg: bool,
    pub lpwr: bool,
    pub sft: bool,
    pub pwr: bool,
    pub pin: bool,
    pub obl: bool,
}

/// Read the reset flags, then clear them (RMVF) so the next boot reports
/// afresh.
///
/// **Why PAC and not the HAL:** the vendored HAL exposes no reset-cause API
/// (no `iwdgrstf` anywhere under `ref/stm32g0xx-hal/src`).
pub fn take_reset_cause() -> ResetCause {
    // SAFETY: RCC's block from the PAC's pointer constant; CSR's reset flags
    // and RMVF are touched nowhere else.
    let rcc = unsafe { &*stm32::RCC::ptr() };
    let r = rcc.csr().read();
    let cause = ResetCause {
        iwdg: r.iwdgrstf().bit_is_set(),
        wwdg: r.wwdgrstf().bit_is_set(),
        lpwr: r.lpwrrstf().bit_is_set(),
        sft: r.sftrstf().bit_is_set(),
        pwr: r.pwrrstf().bit_is_set(),
        pin: r.pinrstf().bit_is_set(),
        obl: r.oblrstf().bit_is_set(),
    };
    rcc.csr().modify(|_, w| w.rmvf().set_bit());
    cause
}

/// Turn on the flash prefetch buffer (E135) and report the readback. Call
/// after the HAL's clock setup, which programs the wait states.
///
/// **Why PAC and not the HAL:** the vendored HAL touches FLASH_ACR only for
/// LATENCY, inside `Rcc::freeze` (`ref/stm32g0xx-hal/src/rcc/mod.rs:154-155`),
/// and has no prefetch API (no `prften` under `ref/stm32g0xx-hal/src`). A
/// named-field **modify**, never a write: a wholesale ACR store once cleared
/// DBG_SWEN and disabled SWD (`bin/shell-pwm.rs`).
pub fn flash_prefetch_enable() -> bool {
    // SAFETY: FLASH's block from the PAC's pointer constant; after the HAL's
    // clock setup nothing else writes ACR.
    let flash = unsafe { &*stm32::FLASH::ptr() };
    flash.acr().modify(|_, w| w.prften().set_bit());
    flash_prefetch_on()
}

/// Whether the flash prefetch buffer is on (FLASH_ACR.PRFTEN), for the banner.
pub fn flash_prefetch_on() -> bool {
    // SAFETY: FLASH's block from the PAC's pointer constant; a read.
    let flash = unsafe { &*stm32::FLASH::ptr() };
    flash.acr().read().prften().bit_is_set()
}

/// Program the IWDG timeout: PR = 0 (LSI/4 = 8 kHz) and the given reload.
/// Call after the HAL's `IndependedWatchdog::start`, which keeps ownership
/// and its `feed()`.
///
/// **A HAL defect found by provoking it (E080).** `IndependedWatchdog::start`
/// computes its reload at 16384 Hz --
/// `crate::time::cycles(period, 16_384.Hz())`
/// (`ref/stm32g0xx-hal/src/watchdog.rs:16`) -- but at PR = 0 this IWDG counts
/// LSI/4 = 8 kHz, so its "50 ms" is ~102 ms. This is the same register
/// sequence the HAL uses, with the reload computed for the clock actually
/// counting. Returns false if the prescaler/reload update never completed.
pub fn iwdg_set_timeout(reload: u16) -> bool {
    // SAFETY: IWDG's block from the PAC's pointer constant; the HAL's
    // watchdog object only ever writes KR (feed), which this sequence ends
    // with anyway.
    let iwdg = unsafe { &*stm32::IWDG::ptr() };
    iwdg.kr().write(|w| w.key().start());
    iwdg.kr().write(|w| w.key().unlock());
    iwdg.pr().write(|w| w.pr().divide_by4());
    iwdg.rlr().write(|w| w.rl().set(reload));
    let busy = || {
        let s = iwdg.sr().read();
        s.pvu().bit_is_set() || s.rvu().bit_is_set() || s.wvu().bit_is_set()
    };
    let mut n = 0u32;
    while busy() && n < 1_000_000 {
        n += 1;
    }
    iwdg.kr().write(|w| w.key().feed());
    !busy()
}
