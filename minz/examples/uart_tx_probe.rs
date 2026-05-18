#![no_std]
#![no_main]

use core::fmt::Write;

use cortex_m_rt::entry;
use minz::hal::delay::Delay;
use minz::hal::prelude::*;
use minz::hal::rcc::{MsiFreq, PllConfig, PllDivider, PllSource};
use minz::hal::serial::{Config, Serial};
use minz::hal::stm32;
use minz::panic;
use rtt_target::rprintln;

#[entry]
fn main() -> ! {
    let cp = cortex_m::Peripherals::take().unwrap();
    let dp = stm32::Peripherals::take().unwrap();

    let mut flash = dp.FLASH.constrain();
    let mut rcc = dp.RCC.constrain();
    let mut pwr = dp.PWR.constrain(&mut rcc.apb1r1);
    let pll_cfg = PllConfig::new(6, 40, PllDivider::Div4);
    let clocks = rcc
        .cfgr
        .msi(MsiFreq::RANGE48M)
        .pll_source(PllSource::MSI)
        .sysclk_with_pll(80.MHz(), pll_cfg)
        .pclk1(80.MHz())
        .pclk2(80.MHz())
        .freeze(&mut flash.acr, &mut pwr);

    panic::ensure_rtt();
    rprintln!(
        "uart_tx_probe: USART1 TX on PB6 @ 9600, clocks sysclk={} pclk1={} pclk2={}",
        clocks.sysclk().raw(),
        clocks.pclk1().raw(),
        clocks.pclk2().raw(),
    );

    // Half-duplex USART1: PB6 only, AF7 open-drain + internal pull-up.
    // Frees PB7 for other uses (analog / comparator). Half-duplex
    // needs the line idle-high — the pull-up keeps it there when
    // the L431 isn't actively driving a start bit.
    let mut gpiob = dp.GPIOB.split(&mut rcc.ahb2);
    let mut tx = gpiob.pb6.into_alternate_open_drain::<7>(
        &mut gpiob.moder,
        &mut gpiob.otyper,
        &mut gpiob.afrl,
    );
    tx.internal_pull_up(&mut gpiob.pupdr, true);

    let serial = Serial::usart1(
        dp.USART1,
        (tx,),
        Config::default().baudrate(9_600.bps()),
        clocks,
        &mut rcc.apb2,
    );
    let (mut tx, _) = serial.split();

    let mut delay = Delay::new(cp.SYST, clocks);
    let mut n = 0u32;
    loop {
        n = n.wrapping_add(1);
        writeln!(&mut tx, "uart_tx_probe {} @9600 on PB6", n).ok();
        rprintln!("sent line {}", n);
        delay.delay_ms(500_u32);
    }
}
