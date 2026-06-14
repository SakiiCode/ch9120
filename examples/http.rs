#![no_std]
#![no_main]

use core::net::Ipv4Addr;
use core::panic::PanicInfo;

use alloc::string::ToString;
use ch9120::{Ch9120ConfigBuilder, Ch9120Driver};
use cortex_m_rt::entry;
use defmt::Display2Format;
use embassy_executor::Spawner;
use embassy_rp::bind_interrupts;
use embassy_rp::gpio::Level;
use embassy_rp::{
    Peri,
    gpio::Output,
    peripherals::*,
    uart::{self, Blocking, BufferedUart, Uart},
};
use embassy_time::Duration;
use embedded_alloc::LlffHeap as Heap;
use reqwless::request::RequestBuilder;
use reqwless::{
    request::{Method, Request},
    response::Response,
};
use static_cell::StaticCell;

extern crate alloc;

#[global_allocator]
static HEAP: Heap = Heap::empty();

bind_interrupts!(struct Irqs{
    UART0_IRQ => uart::InterruptHandler::<UART0>;
    UART1_IRQ => uart::BufferedInterruptHandler::<UART1>;
});

#[panic_handler]
pub fn panic(info: &PanicInfo) -> ! {
    defmt::error!("{}", Display2Format(info));
    loop {}
}

#[embassy_executor::main(executor = "embassy_rp::executor::Executor")]
#[entry]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    unsafe {
        embedded_alloc::init!(HEAP, 300 * 1024);
    }

    let (tx_pin, rx_pin, uart) = (p.PIN_0, p.PIN_1, p.UART0);

    let uart = uart::Uart::new_blocking(uart, tx_pin, rx_pin, uart::Config::default());

    static SERIAL: StaticCell<Uart<'static, Blocking>> = StaticCell::new();
    defmt_serial::defmt_serial(SERIAL.init(uart));

    defmt::info!("Defmt OK");

    let mut driver = setup_ch9120(p.UART1, p.PIN_20, p.PIN_21, p.PIN_18, p.PIN_19).await;
    driver.store_config().await.unwrap();

    let path = "/";
    let config = driver.config().unwrap();
    let host = config.target_ip.to_string();
    let port = config.target_port;

    defmt::info!(
        "Sending GET request to http://{}:{}{}",
        host.as_str(),
        port,
        path
    );

    let mut response_buf = [0u8; 3072];

    let request = Request::new(Method::GET, path).host(&host).build();
    request.write_header(driver.inner()).await.unwrap();
    let mut headers = [0u8; 1024];
    let response = Response::read(driver.inner(), Method::GET, &mut headers)
        .await
        .expect("Response read error");

    let response_len = response
        .body()
        .reader()
        .read_to_end(&mut response_buf)
        .await
        .unwrap();
    let response_string = core::str::from_utf8(&response_buf[..response_len]).unwrap();
    defmt::info!("Response:\n{}", response_string);
}

async fn setup_ch9120(
    uart: Peri<'static, UART1>,
    tx_pin: Peri<'static, PIN_20>,
    rx_pin: Peri<'static, PIN_21>,
    cfg_pin: Peri<'static, PIN_18>,
    rst_pin: Peri<'static, PIN_19>,
) -> Ch9120Driver<BufferedUart, Output<'static>, Output<'static>> {
    static TX_BUF: StaticCell<[u8; 128]> = StaticCell::new();
    let tx_buf = TX_BUF.init([0; 128]);
    static RX_BUF: StaticCell<[u8; 128]> = StaticCell::new();
    let rx_buf = RX_BUF.init([0; 128]);

    let mut uart_config = uart::Config::default();
    uart_config.baudrate = 9600;
    let uart = BufferedUart::new(uart, tx_pin, rx_pin, Irqs, tx_buf, rx_buf, uart_config);

    defmt::info!("CH9120 initialized");

    let driver_config = Ch9120ConfigBuilder::default()
        .local_ip(Ipv4Addr::from_octets([192, 168, 50, 40]))
        .gateway(Ipv4Addr::from_octets([255, 255, 255, 0]))
        .subnet_mask([255, 255, 255, 0])
        .local_port(1000)
        .target_ip(Ipv4Addr::from_octets([192, 168, 50, 128]))
        .target_port(2000)
        .transport_baud_rate(9600)
        .build()
        .unwrap();

    let cfg_pin = Output::new(cfg_pin, Level::Low);
    let rst_pin = Output::new(rst_pin, Level::High);

    let driver = Ch9120Driver::new(
        driver_config,
        uart,
        cfg_pin,
        rst_pin,
        Duration::from_secs(2),
        |uart, baudrate| {
            uart.set_baudrate(baudrate);
        },
    );

    driver
}
