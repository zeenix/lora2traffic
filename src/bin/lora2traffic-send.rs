//! This example runs on a STM32WL board, which has a builtin Semtech Sx1262 radio.
//! It demonstrates LORA P2P send functionality.
#![no_std]
#![no_main]

extern crate alloc;

#[path = "../common.rs"]
mod common;
#[path = "../iv.rs"]
mod iv;

use common::Signal;
use defmt::info;
use embassy_executor::Spawner;
use embassy_stm32::bind_interrupts;
use embassy_stm32::exti::ExtiInput;
use embassy_stm32::gpio::{Level, Output, Pin, Pull, Speed};
use embassy_stm32::spi::Spi;
use futures_util::stream::{self, SelectAll};
use {defmt_rtt as _, panic_probe as _};

use self::iv::InterruptHandler;

bind_interrupts!(struct Irqs{
    SUBGHZ_RADIO => InterruptHandler;
});

const HEAP_SIZE: usize = 1024;
static mut HEAP: [u8; HEAP_SIZE] = [0; HEAP_SIZE];

#[global_allocator]
static ALLOCATOR: embedded_alloc::Heap = embedded_alloc::Heap::empty();

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let config = common::create_stm32_config();
    let p = embassy_stm32::init(config);

    unsafe { ALLOCATOR.init(core::ptr::addr_of_mut!(HEAP) as usize, HEAP_SIZE) }

    let mut button = ExtiInput::new(p.PA0, p.EXTI0, Pull::Up);

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

    let mut st = SelectAll::new();
    st.push(stream::empty::<()>());

    let mut signal = Signal::default();
    loop {
        button.wait_for_falling_edge().await;
        info!("Button pressed");
        button.wait_for_rising_edge().await;
        info!("Button released");

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

        signal.rotate();
    }
}
