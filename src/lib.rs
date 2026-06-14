#![no_std]

use core::net::Ipv4Addr;

use alloc::boxed::Box;
use derive_builder::Builder;
use embassy_time::Duration;
use embedded_hal::digital::OutputPin;
use embedded_io_async::{Read, Write};

use crate::{config::ConfigStoreError, transport::TimeoutBuffer};

extern crate alloc;

mod config;
mod transport;

#[repr(u8)]
#[derive(Debug, Clone, Copy, Default)]
pub enum Mode {
    #[default]
    TcpServer = 0,
    TcpClient = 1,
    UdpServer = 2,
    UdpClient = 3,
}

#[derive(Debug, Builder)]
#[builder(no_std, build_fn(error(validation_error = false)), default)]
pub struct Ch9120Config {
    pub mode: Mode,
    pub local_ip: Ipv4Addr,
    pub gateway: Ipv4Addr,
    pub subnet_mask: [u8; 4],
    pub local_port: u16,
    pub target_ip: Ipv4Addr,
    pub target_port: u16,
    pub transport_baud_rate: u32,
    pub rx_timeout: u32,
}

impl Default for Ch9120Config {
    fn default() -> Self {
        Self {
            mode: Mode::TcpClient,
            local_ip: Ipv4Addr::from_octets([192, 168, 1, 200]),
            gateway: Ipv4Addr::from_octets([192, 168, 1, 1]),
            subnet_mask: [255, 255, 255, 0],
            local_port: 2000,
            target_ip: Ipv4Addr::from_octets([192, 168, 1, 100]),
            target_port: 1000,
            transport_baud_rate: 9600,
            rx_timeout: 0,
        }
    }
}

pub struct Ch9120Driver<T: Read + Write, CFG: OutputPin, RST: OutputPin> {
    config: Option<Ch9120Config>,
    uart: TimeoutBuffer<T>,
    cfg_pin: CFG,
    rst_pin: RST,
    controller: UartController<T>,
}

impl<T: Read + Write, CFG: OutputPin, RST: OutputPin> Ch9120Driver<T, CFG, RST> {
    pub fn new(
        config: Ch9120Config,
        uart: T,
        cfg_pin: CFG,
        rst_pin: RST,
        timeout: Duration,
        set_baudrate: impl FnMut(&mut T, u32) + 'static,
    ) -> Self {
        Self {
            config: Some(config),
            uart: TimeoutBuffer::new(uart, timeout),
            cfg_pin,
            rst_pin,
            controller: UartController {
                set_baudrate: Box::new(set_baudrate),
            },
        }
    }
    pub fn new_without_config(
        uart: T,
        cfg_pin: CFG,
        rst_pin: RST,
        timeout: Duration,
        set_baudrate: impl FnMut(&mut T, u32) + 'static,
    ) -> Self {
        Self {
            config: None,
            uart: TimeoutBuffer::new(uart, timeout),
            cfg_pin,
            rst_pin,
            controller: UartController {
                set_baudrate: Box::new(set_baudrate),
            },
        }
    }
    pub async fn store_config(&mut self) -> Result<(), ConfigStoreError> {
        if let Some(config) = &self.config {
            config::ch9120_store_config(
                config,
                self.uart.inner(),
                &mut self.cfg_pin,
                &mut self.rst_pin,
                &mut self.controller,
            )
            .await
        } else {
            Err(ConfigStoreError::MissingConfig)
        }
    }

    pub fn inner(&mut self) -> &mut T {
        &mut self.uart
    }

    pub fn config(&self) -> Option<&Ch9120Config> {
        self.config.as_ref()
    }
}

pub struct UartController<T> {
    pub set_baudrate: Box<dyn FnMut(&mut T, u32)>,
}

pub trait Baudrate {
    fn set_baudrate(&mut self, value: u32);
}
