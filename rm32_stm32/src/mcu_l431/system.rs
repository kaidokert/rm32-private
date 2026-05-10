//! System control (IRQ, watchdog, reset) for STM32L431.

pub struct System {
    _private: (),
}

impl System {
    pub fn new() -> Self {
        Self { _private: () }
    }
}

impl rm32::hal::System for System {
    fn reset(&mut self) -> ! {
        cortex_m::peripheral::SCB::sys_reset()
    }
    fn enable_irq(&mut self) {
        unsafe { cortex_m::interrupt::enable() };
    }
    fn disable_irq(&mut self) {
        cortex_m::interrupt::disable();
    }
    fn start_watchdog(&mut self, prescaler: u8, reload: u16) {
        // L431 default option byte is IWDG_SW=1 (software start). LSI is not
        // running after reset, and the SR busy-wait below would hang forever
        // unless LSI is up. Enable LSI explicitly first.
        let rcc = unsafe { &*crate::pac::RCC::PTR };
        rcc.csr.modify(|_, w| w.lsion().set_bit());
        while rcc.csr.read().lsirdy().bit_is_clear() {}

        let iwdg = unsafe { &*crate::pac::IWDG::PTR };
        unsafe {
            // Activate IWDG first — STM HAL pattern. Without IWDG running,
            // SR.PVU/RVU would never clear after PR/RLR writes.
            iwdg.kr.write(|w| w.bits(0xCCCC));
            iwdg.kr.write(|w| w.bits(0x5555));
            iwdg.pr.write(|w| w.bits(prescaler as u32));
            iwdg.rlr.write(|w| w.bits(reload as u32));
            while iwdg.sr.read().bits() & 0x03 != 0 {}
            iwdg.kr.write(|w| w.bits(0xAAAA));
        }
    }
    fn reload_watchdog(&mut self) {
        let iwdg = unsafe { &*crate::pac::IWDG::PTR };
        unsafe {
            iwdg.kr.write(|w| w.bits(0xAAAA));
        }
    }
    fn delay_micros(&mut self, us: u32) {
        cortex_m::asm::delay(us * 80);
    }
    fn delay_millis(&mut self, ms: u32) {
        for _ in 0..ms {
            self.delay_micros(1000);
        }
    }
}
