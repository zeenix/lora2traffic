//! This example runs on a STM32WL board, which has a builtin Semtech Sx1262 radio.
//! It demonstrates LORA P2P send functionality.
#![no_std]
#![no_main]

use defmt::{info, warn};
use embassy_executor::Spawner;
use embassy_futures::select;
use embassy_stm32::exti::ExtiInput;
use embassy_stm32::gpio::{AnyPin, Level, Output, Pin, Pull, Speed};
use embassy_stm32::spi::Spi;
use embassy_time::Timer;
use {defmt_rtt as _, panic_probe as _};

use lora2traffic::*;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let config = create_stm32_config();
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
    let mut lora = LoraHw::new(ctrl1, ctrl2, ctrl3, spi).await;

    // Query the signal state.
    lora.send(Message::QuerySignal).await.unwrap();
    let mut signal = match lora.receive().await.unwrap() {
        Message::Signal(signal) => signal,
        _ => {
            info!("No signal received, defaulting to red");
            Signal::default()
        }
    };
    info!("Initial signal = {:?}", signal);
    indicator.set(signal);
    loop {
        let duration = signal.duration();
        // Wait for either timeout or button press.
        select::select(
            Timer::after_secs(duration),
            wait_for_button_press(&mut button),
        )
        .await;

        signal.rotate();

        let msg = Message::Signal(signal);

        match lora.send(msg).await {
            Ok(()) => {
                info!("TX DONE");
            }
            Err(err) => {
                info!("Radio error = {}", err);
                return;
            }
        };
        // Receive the ACK
        match lora.receive().await {
            Ok(Message::Signal(sig)) if sig == signal => info!("ACK received"),
            Ok(Message::Signal(sig)) => {
                warn!("ACK received with different signal: {:?}", sig);
                signal = sig;
            }
            Ok(Message::QuerySignal) => warn!("Unexpected query message received"),
            Err(e) => warn!("RX error = {}", e),
        }
        lora.sleep().await;

        indicator.set(signal);
    }
}

async fn wait_for_button_press(button: &mut ExtiInput<'_>) {
    button.wait_for_falling_edge().await;
    info!("Button pressed");
    button.wait_for_rising_edge().await;
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
