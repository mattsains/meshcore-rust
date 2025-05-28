use std::sync::Mutex;

use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::mono_font::ascii::FONT_10X20;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::{Primitive, Size};
use embedded_graphics::primitives::{PrimitiveStyle, Rectangle, StyledDrawable};
use embedded_graphics::Drawable;
use embedded_graphics::{
    mono_font::{ascii::FONT_6X10, MonoTextStyle},
    pixelcolor::Rgb565,
    prelude::{Point, RgbColor},
    text::Text,
};
use embedded_graphics_framebuf::FrameBuf;
use embedded_hal::digital::{OutputPin, PinState};
use embedded_hal::spi::{Mode, MODE_0};
use esp_idf_svc::hal::gpio::DriveStrength;
use esp_idf_svc::hal::units::Hertz;
use esp_idf_svc::hal::{
    delay::Delay,
    gpio::{AnyOutputPin, Output, Pin, PinDriver},
    prelude::Peripherals,
    spi::{
        config::{Config, DriverConfig},
        Spi, SpiDeviceDriver, SpiDriver, SpiSingleDeviceDriver,
    },
    units::MegaHertz,
};
use esp_idf_svc::sys::gpio_config;
use mipidsi::NoResetPin;
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

    display.clear(Rgb565::BLACK).unwrap();

    let style = MonoTextStyle::new(&FONT_10X20, Rgb565::WHITE);
    let blanking_style = MonoTextStyle::new(&FONT_10X20, Rgb565::BLACK);

    loop {
        for i in 0..=16 {
            println!("{i}");
            set_brightness(i, &mut display_bl, delay);
            println!("back in main");
            Text::new(i.to_string().as_str(), Point::new(50, 50), style)
                .draw(&mut display)
                .unwrap();
            delay.delay_ms(300);
            Text::new(i.to_string().as_str(), Point::new(50, 50), blanking_style)
                .draw(&mut display)
                .unwrap();
            println!("loop iter done");
        }
        for j in (0..=16).rev() {
            println!("{j}");
            set_brightness(j, &mut display_bl, delay);
            println!("back in main");
            Text::new(j.to_string().as_str(), Point::new(50, 50), style)
                .draw(&mut display)
                .unwrap();
            delay.delay_ms(300);
            Text::new(j.to_string().as_str(), Point::new(50, 50), blanking_style)
                .draw(&mut display)
                .unwrap();
        }
    }
    loop {
        print!("done");
        delay.delay_ms(1000);
    }
}

static DISPLAY_BUFFER: StaticCell<[u8; 1024]> = StaticCell::new();
static DISPLAY_LEVEL: Mutex<u8> = Mutex::new(16);

fn set_brightness(level: u8, mut bl: impl embedded_hal::digital::OutputPin, delay: Delay) {
    assert!(level < 17);

    let mut current_level = DISPLAY_LEVEL.lock().unwrap();
    if level == 16 {
        println!("want 16");
        bl.set_low().unwrap();
        println!("wait 3");
        delay.delay_ms(3); //this wait resets the brightness to max
        bl.set_high().unwrap();
        println!("reset currentlevel");
        *current_level = 16;
        println!("done");
    } else if level == 0 {
        println!("want 0");
        bl.set_low().unwrap();
        delay.delay_ms(3);
        *current_level = 0;
        println!("got 0");
    } else {
        // every time we pulse the backlight, it causes it to reduce brightness by 1
        let num_steps = (*current_level as i8 - level as i8).rem_euclid(16);
        println!(
            "current: {}, desired: {level} => go down {num_steps}",
            *current_level
        );

        bl.set_high().unwrap();
        delay.delay_us(30);
        for _ in 0..num_steps {
            bl.set_low().unwrap();
            bl.set_high().unwrap();
            delay.delay_us(30);
        }
        delay.delay_ms(3);
        *current_level = level;
    }
}

fn enable_peripheral(enable_pin: &mut impl embedded_hal::digital::OutputPin) {
    enable_pin.set_high().unwrap();
}
