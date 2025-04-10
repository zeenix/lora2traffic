#![no_std]

use embassy_stm32::bind_interrupts;

mod iv;
pub use iv::*;
mod lora;
pub use lora::*;
mod signal;
pub use signal::*;
mod protocol;
pub use protocol::*;

bind_interrupts!(struct Irqs{
    SUBGHZ_RADIO => InterruptHandler;
});

pub fn create_stm32_config() -> embassy_stm32::Config {
    let mut config = embassy_stm32::Config::default();
    {
        use embassy_stm32::{rcc::*, time::Hertz};
        config.rcc.hse = Some(Hse {
            freq: Hertz(32_000_000),
            mode: HseMode::Bypass,
            prescaler: HsePrescaler::DIV1,
        });
        config.rcc.sys = Sysclk::PLL1_R;
        config.rcc.pll = Some(Pll {
            source: PllSource::HSE,
            prediv: PllPreDiv::DIV2,
            mul: PllMul::MUL6,
            divp: None,
            divq: Some(PllQDiv::DIV2), // PLL1_Q clock (32 / 2 * 6 / 2), used for RNG
            divr: Some(PllRDiv::DIV2), // sysclk 48Mhz clock (32 / 2 * 6 / 2)
        });

        config
    }
}
