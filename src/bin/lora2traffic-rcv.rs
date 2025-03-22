//! This example runs on the STM32WL board, which has a builtin Semtech Sx1262 radio.
//! It demonstrates LORA P2P receive functionality in conjunction with the lora_p2p_send example.
#![no_std]
#![no_main]

#[path = "../common.rs"]
mod common;
#[path = "../iv.rs"]
mod iv;

use defmt::{info, warn};
use embassy_executor::Spawner;
use embassy_stm32::bind_interrupts;
use embassy_stm32::gpio::{AnyPin, Level, Output, Pin, Speed};
use embassy_stm32::spi::Spi;
use embassy_stm32::time::Hertz;
use embassy_time::{Delay, Timer};
use lora_phy::sx126x::{Stm32wl, Sx126x, TcxoCtrlVoltage};
use lora_phy::{mod_params::*, sx126x};
use lora_phy::{LoRa, RxMode};
use {defmt_rtt as _, panic_probe as _};

use self::iv::{InterruptHandler, Stm32wlInterfaceVariant, SubghzSpiDevice};
use common::Signal;

bind_interrupts!(struct Irqs{
    SUBGHZ_RADIO => InterruptHandler;
});

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let mut config = embassy_stm32::Config::default();
    {
        use embassy_stm32::rcc::*;
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
    }
    let p = embassy_stm32::init(config);

    // Set CTRL1 and CTRL3 for high-power transmission, while CTRL2 acts as an RF switch between tx and rx
    let _ctrl1 = Output::new(p.PC4.degrade(), Level::Low, Speed::High);
    let ctrl2 = Output::new(p.PC5.degrade(), Level::High, Speed::High);
    let _ctrl3 = Output::new(p.PC3.degrade(), Level::High, Speed::High);

    let mut signal_control = SignalControl::new(
        p.PC6.degrade(), // Pin 12 on the board.
        p.PC0.degrade(), // Pin 14 on the board.
        p.PA8.degrade(), // Pin 16 on the board.
        Signal::Red,
    )
    .await;

    let spi = Spi::new_subghz(p.SUBGHZSPI, p.DMA1_CH1, p.DMA1_CH2);
    let spi = SubghzSpiDevice(spi);

    let config = sx126x::Config {
        chip: Stm32wl {
            use_high_power_pa: true,
        },
        tcxo_ctrl: Some(TcxoCtrlVoltage::Ctrl1V7),
        use_dcdc: true,
        rx_boost: false,
    };
    let iv = Stm32wlInterfaceVariant::new(Irqs, None, Some(ctrl2)).unwrap();
    let mut lora = LoRa::new(Sx126x::new(spi, iv, config), false, Delay)
        .await
        .unwrap();

    let mdltn_params = {
        match lora.create_modulation_params(
            SpreadingFactor::_10,
            Bandwidth::_250KHz,
            CodingRate::_4_8,
            common::LORA_FREQUENCY_IN_HZ,
        ) {
            Ok(mp) => mp,
            Err(err) => {
                warn!("Radio error = {}", err);
                return;
            }
        }
    };
    let mut buffer = [00u8; 100];

    loop {
        info!("......................LOOPING......................................");

        let rx_pkt_params = {
            match lora.create_rx_packet_params(
                4,
                false,
                buffer.len() as u8,
                true,
                false,
                &mdltn_params,
            ) {
                Ok(pp) => pp,
                Err(err) => {
                    warn!("Radio error = {}", err);
                    return;
                }
            }
        };
        match lora
            .prepare_for_rx(RxMode::Continuous, &mdltn_params, &rx_pkt_params)
            .await
        {
            Ok(()) => {}
            Err(err) => {
                warn!("Radio error = {}", err);
                return;
            }
        };
        let signal_byte = match lora.rx(&rx_pkt_params, &mut buffer).await {
            Ok((received_len, _rx_pkt_status)) => {
                info!("......................rx received something......................................");
                if (received_len == 3)
                    && (buffer[0] == common::HEADER)
                    && (buffer[2] == common::FOOTER)
                {
                    info!("rx successful");
                    buffer[1]
                } else {
                    info!("rx unknown packet");

                    continue;
                }
            }
            Err(err) => {
                warn!("rx unsuccessful = {}", err);

                continue;
            }
        };

        match Signal::from_u8(signal_byte) {
            Some(signal) => signal_control.set(signal),
            None => info!("rx unknown signal"),
        }
    }
}

struct SignalControl {
    red: Output<'static>,
    yellow: Output<'static>,
    green: Output<'static>,

    state: Signal,
}

impl SignalControl {
    async fn new(red: AnyPin, yellow: AnyPin, green: AnyPin, init_state: Signal) -> Self {
        let mut control = Self {
            red: Output::new(red, Level::Low, Speed::High),
            yellow: Output::new(yellow, Level::Low, Speed::High),
            green: Output::new(green, Level::Low, Speed::High),
            state: init_state,
        };

        // Startup checks.
        let mut signal = Signal::Red;
        for _ in 0..3 {
            control.set(signal);
            signal.rotate();
            Timer::after(embassy_time::Duration::from_secs(1)).await;
        }

        // Reset to initial state.
        control.set(init_state);

        control
    }

    fn set(&mut self, signal: Signal) {
        info!("Setting signal = {:?}", signal);
        self.state = signal;
        match signal {
            Signal::Red => {
                self.red.set_low();
                self.yellow.set_high();
                self.green.set_high();
            }
            Signal::Yellow => {
                self.red.set_high();
                self.yellow.set_low();
                self.green.set_high();
            }
            Signal::Green => {
                self.red.set_high();
                self.yellow.set_high();
                self.green.set_low();
            }
        }
    }
}

impl Signal {
    pub fn from_u8(byte: u8) -> Option<Self> {
        match byte {
            b'r' => Some(Self::Red),
            b'y' => Some(Self::Yellow),
            b'g' => Some(Self::Green),
            _ => None,
        }
    }
}
