use embassy_stm32::{gpio::Output, mode::Async, spi::Spi};
use embassy_time::Delay;
use lora_phy::{
    mod_params::{Bandwidth, CodingRate, ModulationParams, SpreadingFactor},
    sx126x::{self, Stm32wl, Sx126x, TcxoCtrlVoltage},
    LoRa,
};

use crate::{
    iv::{Stm32wlInterfaceVariant, SubghzSpiDevice},
    Irqs,
};

pub(crate) const LORA_FREQUENCY_IN_HZ: u32 = 434_000_000; // Top of the EU RF band range

#[derive(defmt::Format, Clone, Copy, PartialEq)]
#[repr(u8)]
pub(crate) enum Signal {
    Red = b'r',
    Yellow = b'y',
    Green = b'g',
    Off = b'o',
}

impl Signal {
    pub fn rotate(&mut self) {
        *self = match self {
            Self::Red => Self::Yellow,
            Self::Yellow => Self::Green,
            Self::Green => Self::Off,
            Self::Off => Self::Red,
        };
    }
}

impl Default for Signal {
    fn default() -> Self {
        Self::Off
    }
}

pub(crate) const HEADER: u8 = 117;
pub(crate) const FOOTER: u8 = 255;

pub(crate) fn create_stm32_config() -> embassy_stm32::Config {
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

pub(crate) async fn create_lora(
    ctrl2: Output<'static>,
    spi: Spi<'static, Async>,
) -> (
    LoRa<
        Sx126x<
            SubghzSpiDevice<Spi<'static, Async>>,
            Stm32wlInterfaceVariant<Output<'static>>,
            Stm32wl,
        >,
        Delay,
    >,
    ModulationParams,
) {
    let spi = SubghzSpiDevice(spi);

    let config = sx126x::Config {
        chip: Stm32wl {
            use_high_power_pa: true,
        },
        tcxo_ctrl: Some(TcxoCtrlVoltage::Ctrl1V7),
        use_dcdc: true,
        rx_boost: true,
    };
    let iv = Stm32wlInterfaceVariant::new(Irqs, None, Some(ctrl2)).unwrap();
    let mut lora = LoRa::new(Sx126x::new(spi, iv, config), false, Delay)
        .await
        .unwrap();

    let mdltn_params = {
        match lora.create_modulation_params(
            SpreadingFactor::_12,
            Bandwidth::_125KHz,
            CodingRate::_4_8,
            LORA_FREQUENCY_IN_HZ,
        ) {
            Ok(mp) => mp,
            Err(err) => {
                panic!("Radio error = {err:?}");
            }
        }
    };

    (lora, mdltn_params)
}
