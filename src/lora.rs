use defmt::{info, warn};
use embassy_stm32::{gpio::Output, mode::Async, spi::Spi};
use embassy_time::Delay;
use lora_phy::{
    mod_params::{Bandwidth, CodingRate, ModulationParams, RadioError, SpreadingFactor},
    sx126x::{self, Stm32wl, Sx126x, TcxoCtrlVoltage},
    LoRa, RxMode,
};

use crate::{
    iv::{Stm32wlInterfaceVariant, SubghzSpiDevice},
    Irqs, Message,
};

pub const LORA_FREQUENCY_IN_HZ: u32 = 434_000_000; // Top of the EU RF band range

pub struct LoraHw {
    lora: LoRa<
        Sx126x<
            SubghzSpiDevice<Spi<'static, Async>>,
            Stm32wlInterfaceVariant<Output<'static>>,
            Stm32wl,
        >,
        Delay,
    >,
    mod_params: ModulationParams,
}

impl LoraHw {
    pub async fn new(
        ctrl1: Output<'static>,
        ctrl2: Output<'static>,
        ctrl3: Output<'static>,
        spi: Spi<'static, Async>,
    ) -> Self {
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

        let mod_params = {
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

        Self { lora, mod_params }
    }

    pub async fn receive(&mut self) -> Result<Message, ()> {
        let mut buffer = [00u8; crate::MSG_SIZE];

        loop {
            let rx_pkt_params = {
                match self.lora.create_rx_packet_params(
                    4,
                    false,
                    buffer.len() as u8,
                    true,
                    false,
                    &self.mod_params,
                ) {
                    Ok(pp) => pp,
                    Err(err) => {
                        warn!("Radio error = {}", err);
                        return Err(());
                    }
                }
            };
            match self
                .lora
                .prepare_for_rx(RxMode::Continuous, &self.mod_params, &rx_pkt_params)
                .await
            {
                Ok(()) => {}
                Err(err) => {
                    warn!("Radio error = {}", err);
                    return Err(());
                }
            };
            match self.lora.rx(&rx_pkt_params, &mut buffer).await {
                Ok((received_len, rx_pkt_status)) => {
                    info!(
                        "rx received something. SNR = {}, RSSI = {}",
                        rx_pkt_status.snr, rx_pkt_status.rssi
                    );
                    match Message::from_bytes(&buffer[..received_len as usize]) {
                        Some(msg) => {
                            info!("rx message = {:?}", msg);
                            return Ok(msg);
                        }
                        None => info!("rx unknown packet. Ignoring..."),
                    }
                }
                Err(err) => {
                    warn!("rx unsuccessful = {}", err);

                    continue;
                }
            }
        }
    }

    pub async fn send(&mut self, msg: Message) -> Result<(), RadioError> {
        let buffer = msg.to_bytes();

        let mut tx_pkt_params =
            self.lora
                .create_tx_packet_params(4, false, true, false, &self.mod_params)?;

        self.lora
            .prepare_for_tx(&self.mod_params, &mut tx_pkt_params, 20, &buffer)
            .await?;

        self.lora.tx().await?;

        self.lora.sleep(false).await?;

        Ok(())
    }
}
