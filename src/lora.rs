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

pub const LORA_FREQUENCY_IN_HZ: u32 = 434_000_000; // Top of the EU RF band range

pub async fn create_lora(
    ctrl1: Output<'static>,
    ctrl2: Output<'static>,
    ctrl3: Output<'static>,
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

    let use_high_power_pa = true;
    let config = sx126x::Config {
        chip: Stm32wl { use_high_power_pa },
        tcxo_ctrl: Some(TcxoCtrlVoltage::Ctrl1V7),
        use_dcdc: true,
        rx_boost: true,
    };
    let iv = Stm32wlInterfaceVariant::new(
        Irqs,
        use_high_power_pa,
        Some(ctrl1),
        Some(ctrl2),
        Some(ctrl3),
    )
    .unwrap();
    let mut lora = LoRa::new(Sx126x::new(spi, iv, config), false, Delay)
        .await
        .unwrap();

    let mdltn_params = {
        match lora.create_modulation_params(
            SpreadingFactor::_12,
            Bandwidth::_62KHz,
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
