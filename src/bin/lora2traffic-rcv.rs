//! This example runs on the STM32WL board, which has a builtin Semtech Sx1262 radio.
//! It demonstrates LORA P2P receive functionality in conjunction with the lora_p2p_send example.
#![no_std]
#![no_main]

use defmt::{info, warn};
use embassy_executor::Spawner;
use embassy_stm32::gpio::{AnyPin, Level, Output, Pin, Speed};
use embassy_stm32::spi::Spi;
use embassy_time::Timer;
use {defmt_rtt as _, panic_probe as _};

use lora2traffic::*;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let config = create_stm32_config();
    let p = embassy_stm32::init(config);

    // Set CTRL1 and CTRL3 for high-power transmission, while CTRL2 acts as an RF switch between tx and rx
    let ctrl1 = Output::new(p.PC4.degrade(), Level::Low, Speed::High);
    let ctrl2 = Output::new(p.PC5.degrade(), Level::Low, Speed::High);
    let ctrl3 = Output::new(p.PC3.degrade(), Level::High, Speed::High);

    let mut signal_control = SignalControl::new(
        p.PC6.degrade(), // Pin 12 on the board.
        p.PC0.degrade(), // Pin 14 on the board.
        p.PA8.degrade(), // Pin 16 on the board.
    )
    .await;
    let spi = Spi::new_subghz(p.SUBGHZSPI, p.DMA1_CH1, p.DMA1_CH2);
    let mut lora = LoraHw::new(ctrl1, ctrl2, ctrl3, spi).await;

    loop {
        match lora.receive().await {
            Ok(Message::QuerySignal) => {
                info!("rx query signal");
                // TODO: Send the current signal state.
            }
            Ok(Message::Signal(signal)) => {
                info!("rx signal = {:?}", signal);
                signal_control.set(signal);
            }
            Err(err) => {
                warn!("rx unsuccessful = {}", err);

                continue;
            }
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
    async fn new(red: AnyPin, yellow: AnyPin, green: AnyPin) -> Self {
        let state = Signal::default();
        let mut control = Self {
            red: Output::new(red, Level::Low, Speed::High),
            yellow: Output::new(yellow, Level::Low, Speed::High),
            green: Output::new(green, Level::Low, Speed::High),
            state,
        };

        // Startup checks.
        let mut signal = Signal::Red;
        for _ in 0..3 {
            control.set(signal);
            signal.rotate();
            Timer::after(embassy_time::Duration::from_secs(1)).await;
        }

        // Set the initial state.
        control.set(state);

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
            Signal::Off => {
                self.red.set_high();
                self.yellow.set_high();
                self.green.set_high();
            }
        }
    }
}
