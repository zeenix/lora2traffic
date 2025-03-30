//! This example runs on a STM32WL board, which has a builtin Semtech Sx1262 radio.
//! It demonstrates LORA P2P send functionality.
#![no_std]
#![no_main]

#[path = "../common.rs"]
mod common;
#[path = "../iv.rs"]
mod iv;
#[path = "../signal.rs"]
mod signal;

use defmt::info;
use embassy_executor::Spawner;
use embassy_stm32::bind_interrupts;
use embassy_stm32::exti::ExtiInput;
use embassy_stm32::gpio::{AnyPin, Level, Output, Pin, Pull, Speed};
use embassy_stm32::spi::Spi;
use signal::Signal;
use {defmt_rtt as _, panic_probe as _};

use self::iv::InterruptHandler;

bind_interrupts!(struct Irqs{
    SUBGHZ_RADIO => InterruptHandler;
});

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let config = common::create_stm32_config();
    let p = embassy_stm32::init(config);

    let mut button = ExtiInput::new(p.PA0, p.EXTI0, Pull::Up);
    let mut indicator = SignalIndicator::new(
        p.PB11.degrade(), // Red LED
        p.PB9.degrade(),  // Green LED
    );

    // Set CTRL1 and CTRL3 for high-power transmission, while CTRL2 acts as an RF switch between tx and rx
    let ctrl1 = Output::new(p.PC4.degrade(), Level::Low, Speed::High);
    let ctrl2 = Output::new(p.PC5.degrade(), Level::Low, Speed::High);
    let ctrl3 = Output::new(p.PC3.degrade(), Level::High, Speed::High);

    let spi = Spi::new_subghz(p.SUBGHZSPI, p.DMA1_CH1, p.DMA1_CH2);
    let (mut lora, mdltn_params) = common::create_lora(ctrl1, ctrl2, ctrl3, spi).await;

    let mut tx_pkt_params = {
        match lora.create_tx_packet_params(4, false, true, false, &mdltn_params) {
            Ok(pp) => pp,
            Err(err) => {
                info!("Radio error = {}", err);
                return;
            }
        }
    };

    let mut signal = Signal::default();
    indicator.set(signal);
    loop {
        button.wait_for_falling_edge().await;
        info!("Button pressed");
        button.wait_for_rising_edge().await;
        info!("Button released");
        signal.rotate();

        let buffer = [common::HEADER, signal as u8, common::FOOTER];

        match lora
            .prepare_for_tx(&mdltn_params, &mut tx_pkt_params, 20, &buffer)
            .await
        {
            Ok(()) => {}
            Err(err) => {
                info!("Radio error = {}", err);
                return;
            }
        };

        match lora.tx().await {
            Ok(()) => {
                info!("TX DONE");
            }
            Err(err) => {
                info!("Radio error = {}", err);
                return;
            }
        };

        match lora.sleep(false).await {
            Ok(()) => info!("Sleep successful"),
            Err(err) => info!("Sleep unsuccessful = {}", err),
        }

        indicator.set(signal);
    }
}

struct SignalIndicator {
    red: Output<'static>,
    green: Output<'static>,
}

impl SignalIndicator {
    fn new(red: AnyPin, green: AnyPin) -> Self {
        Self {
            red: Output::new(red, Level::High, Speed::Low),
            green: Output::new(green, Level::High, Speed::Low),
        }
    }

    fn set(&mut self, signal: Signal) {
        match signal {
            Signal::Red => {
                self.red.set_high();
                self.green.set_low();
            }
            Signal::Yellow => {
                // We don't use the blue LED (on PB15) but rather both red and green simultaneously.
                self.red.set_high();
                self.green.set_high();
            }
            Signal::Green => {
                self.red.set_low();
                self.green.set_high();
            }
            Signal::Off => {
                self.red.set_low();
                self.green.set_low();
            }
        }
    }
}
