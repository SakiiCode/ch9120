use embassy_futures::select::{Either, select};
use embassy_time::{Duration, Timer};
use embedded_hal::digital::OutputPin;
use embedded_io_async::{Read, Write};
use thiserror::Error;

use crate::{Ch9120Config, UartController};

const INIT_TX: [u8; 8] = [0x57, 0xAB, 0, 0, 0, 0, 0, 0];

const CONFIG_DELAY: Duration = Duration::from_millis(200);
const CONFIG_BAUD_RATE: u32 = 9600;

#[derive(Debug, Error)]
pub enum ConfigStoreError {
    #[error("Could not set cfg or rst pin")]
    PinAssert,
    #[error("Missing config for CH9120")]
    MissingConfig,
}

pub async fn ch9120_store_config<T: Read + Write>(
    config: &Ch9120Config,
    uart: &mut T,
    cfg_pin: &mut impl OutputPin,
    rst_pin: &mut impl OutputPin,
    controller: &mut UartController<T>,
) -> Result<(), ConfigStoreError> {
    (*controller.set_baudrate)(uart, CONFIG_BAUD_RATE);

    //ch9120_start(&mut cfg_pin, &mut rst_pin);
    cfg_pin.set_low().map_err(|_| ConfigStoreError::PinAssert)?;
    rst_pin
        .set_high()
        .map_err(|_| ConfigStoreError::PinAssert)?;
    defmt::info!("Pins OK");
    delay_ms(500).await;

    defmt::info!("CH9120 started");
    ch9120_set_mode(uart, config.mode as u8).await;
    ch9120_set_local_ip(uart, &config.local_ip.octets()).await;
    ch9120_set_subnet_mask(uart, &config.subnet_mask).await;
    ch9120_set_gateway(uart, &config.gateway.octets()).await;
    ch9120_set_local_port(uart, config.local_port).await;
    ch9120_set_target_ip(uart, &config.target_ip.octets()).await;
    ch9120_set_target_port(uart, config.target_port).await;
    ch9120_set_baud_rate(uart, config.transport_baud_rate).await;
    ch9120_set_rx_timeout(uart, config.rx_timeout).await;

    ch9120_end(uart).await;

    cfg_pin
        .set_high()
        .map_err(|_| ConfigStoreError::PinAssert)?;

    (*controller.set_baudrate)(uart, config.transport_baud_rate);

    defmt::info!("CH9120 config updated");
    Ok(())
}

async fn wait_for_ack<T: Read>(uart: &mut T) {
    let mut response = [0u8; 1];
    #[allow(static_mut_refs)]
    let read_future = uart.read(&mut response);
    let timeout_future = Timer::after_secs(2);
    match select(read_future, timeout_future).await {
        Either::First(bytes) => {
            let bytes = bytes.unwrap_or_default();
            if response[0] != 0xaa {
                #[cfg(feature = "defmt")]
                defmt::error!("Invalid reponse: {=[u8]:02x} ({})", response, bytes);
            } else {
                #[cfg(feature = "defmt")]
                defmt::debug!("<- {=[u8]:02x} ({})", response, bytes);
            }
        }
        Either::Second(_) => {
            defmt::warn!("CH9120 ACK timeout");
        }
    }
}

async fn uart_puts<T: Write>(uart: &mut T, buf: &[u8]) {
    defmt::debug!("-> {=[u8]:02x}", buf);

    uart.write_all(buf).await.expect("Could not send buffer");
}

async fn ch9120_tx<T: Write + Read>(uart: &mut T, command: u8, data: &[u8]) {
    let mut tx = INIT_TX.clone();
    tx[2] = command;
    let buf_len = 3 + data.len();
    tx[3..buf_len].copy_from_slice(data);

    delay_ms(10).await;

    uart_puts(uart, &tx[0..buf_len]).await;

    delay_ms(10).await;

    wait_for_ack(uart).await;
}

// --------------------------------------------------
// High-level configuration functions
// --------------------------------------------------
async fn ch9120_set_mode<T: Write + Read>(uart: &mut T, mode: u8) {
    ch9120_tx(uart, 0x10, &[mode]).await;
    Timer::after(CONFIG_DELAY).await;
}

async fn ch9120_set_local_ip<T: Write + Read>(uart: &mut T, ip: &[u8]) {
    ch9120_tx(uart, 0x11, ip).await;
    Timer::after(CONFIG_DELAY).await;
}

async fn ch9120_set_subnet_mask<T: Write + Read>(uart: &mut T, mask: &[u8]) {
    ch9120_tx(uart, 0x12, mask).await;
    Timer::after(CONFIG_DELAY).await;
}

async fn ch9120_set_gateway<T: Write + Read>(uart: &mut T, gateway: &[u8]) {
    ch9120_tx(uart, 0x13, gateway).await;
    Timer::after(CONFIG_DELAY).await;
}

async fn ch9120_set_local_port<T: Write + Read>(uart: &mut T, port: u16) {
    let mut data = [0u8; 2];
    data[0] = (port & 0xff) as u8;
    data[1] = (port >> 8) as u8;
    ch9120_tx(uart, 0x14, &data).await;
    Timer::after(CONFIG_DELAY).await;
}

async fn ch9120_set_target_ip<T: Write + Read>(uart: &mut T, ip: &[u8]) {
    ch9120_tx(uart, 0x15, ip).await;
    Timer::after(CONFIG_DELAY).await;
}

async fn ch9120_set_target_port<T: Write + Read>(uart: &mut T, port: u16) {
    let mut data = [0u8; 2];
    data[0] = (port & 0xff) as u8;
    data[1] = (port >> 8) as u8;
    ch9120_tx(uart, 0x16, &data).await;
    Timer::after(CONFIG_DELAY).await;
}

async fn ch9120_set_baud_rate<T: Write + Read>(uart: &mut T, baud: u32) {
    let mut data = [0u8; 4];
    data[0] = (baud & 0xff) as u8;
    data[1] = ((baud >> 8) & 0xff) as u8;
    data[2] = ((baud >> 16) & 0xff) as u8;
    data[3] = (baud >> 24) as u8;
    ch9120_tx(uart, 0x21, &data).await;
    Timer::after(CONFIG_DELAY).await;
}

async fn ch9120_set_rx_timeout<T: Write + Read>(uart: &mut T, timeout: u32) {
    let mut data = [0u8; 4];
    data[0] = (timeout & 0xff) as u8;
    data[1] = ((timeout >> 8) & 0xff) as u8;
    data[2] = ((timeout >> 16) & 0xff) as u8;
    data[3] = (timeout >> 24) as u8;
    ch9120_tx(uart, 0x23, &data).await;
    Timer::after(CONFIG_DELAY).await;
}

async fn ch9120_end<T: Read + Write>(uart: &mut T) {
    ch9120_tx(uart, 0x0d, &[]).await;
    delay_ms(200).await;

    ch9120_tx(uart, 0x0e, &[]).await;
    delay_ms(200).await;

    ch9120_tx(uart, 0x5e, &[]).await;
    delay_ms(200).await;
}

#[allow(unused)]
async fn delay_ms(xms: u32) {
    Timer::after_millis(xms.into()).await
}

#[allow(unused)]
async fn delay_us(xus: u32) {
    Timer::after_micros(xus.into()).await
}
