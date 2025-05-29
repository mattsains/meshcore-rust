use std::sync::Mutex;

use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::{
    pixelcolor::Rgb565,
    prelude::RgbColor,
};
use embedded_hal::spi::MODE_0;
use esp_idf_svc::hal::i2c::{I2cConfig, I2cDriver};
use esp_idf_svc::hal::units::Hertz;
use esp_idf_svc::hal::{
    delay::Delay,
    gpio::PinDriver,
    prelude::Peripherals,
    spi::{
        config::{Config, DriverConfig}, SpiSingleDeviceDriver,
    },
};
use mipidsi::{
    interface::SpiInterface,
    models::ST7789,
    options::{ColorInversion, Orientation},
    Builder,
};
use static_cell::StaticCell;

const W: u16 = 240;
const H: u16 = 320;

fn main() {
    println!("hello");
    // It is necessary to call this function once. Otherwise some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_svc::sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();

    log::info!("Hello, world!");

    let peripherals = Peripherals::take().unwrap();

    let mut delay = Delay::new_default();

    let mut boardpower = PinDriver::output(peripherals.pins.gpio10).unwrap();
    enable_peripheral(&mut boardpower);

    let sck = peripherals.pins.gpio40;
    let mosi = peripherals.pins.gpio41;
    let miso = peripherals.pins.gpio38;
    let display_cs = peripherals.pins.gpio12;
    let display_dc = peripherals.pins.gpio11;

    let mut display_bl = PinDriver::output(peripherals.pins.gpio42).unwrap();
    display_bl.set_low().unwrap();

    let spi_device_driver = SpiSingleDeviceDriver::new_single(
        peripherals.spi2,
        sck,
        mosi,
        Some(miso),
        Some(display_cs),
        &DriverConfig::new(),
        &Config::new().baudrate(Hertz(40_000_000)).data_mode(MODE_0),
    )
    .unwrap();

    let di = SpiInterface::new(
        spi_device_driver,
        PinDriver::output(display_dc).unwrap(),
        DISPLAY_BUFFER.init([0_u8; 1024]),
    );
    println!("made it here");

    let mut display = Builder::new(ST7789, di)
        .display_size(W as u16, H as u16)
        .invert_colors(ColorInversion::Inverted)
        .orientation(Orientation::new().rotate(mipidsi::options::Rotation::Deg90))
        .init(&mut delay)
        .unwrap();

    display.clear(Rgb565::RED).unwrap();
    set_brightness(15, display_bl, delay);

    let kb_addr = 0x55;
    let kb_brightness_cmd = 0x01;
    let kb_alt_b_brightness_cmd = 0x02;

    let i2c_sda = peripherals.pins.gpio18;
    let i2c_scl = peripherals.pins.gpio8;

    let config = I2cConfig::new().baudrate(Hertz(100_000));
    let mut i2c = I2cDriver::new(peripherals.i2c0, i2c_sda, i2c_scl, &config).unwrap();

    let mut buf: [u8; 1] = [0];

    
    
    println!("waiting for data");
    loop {
        if i2c.read(kb_addr, &mut buf, 100000).is_ok() && buf[0] > 0{
            print!("{}", buf[0] as char);
        }
        
        delay.delay_ms(100);
    }
}

static DISPLAY_BUFFER: StaticCell<[u8; 1024]> = StaticCell::new();
static DISPLAY_LEVEL: Mutex<u8> = Mutex::new(16);

/// 0: display off
/// 16: maximum brightness
fn set_brightness(level: u8, mut bl: impl embedded_hal::digital::OutputPin, delay: Delay) {
    assert!(level < 17);

    let mut current_level = DISPLAY_LEVEL.lock().unwrap();

    if level == 0 {
        bl.set_low().unwrap();
        delay.delay_ms(3);
    } else {
        // every time we pulse the backlight, it causes it to reduce brightness by 1
        let num_steps = (*current_level as i8 - level as i8).rem_euclid(16);

        bl.set_high().unwrap();
        delay.delay_us(30);
        for _ in 0..num_steps {
            bl.set_low().unwrap();
            bl.set_high().unwrap();
            delay.delay_us(30);
        }
        delay.delay_ms(3);
    }

    *current_level = level;
}

fn enable_peripheral(enable_pin: &mut impl embedded_hal::digital::OutputPin) {
    enable_pin.set_high().unwrap();
}
