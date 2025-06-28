use std::sync::Mutex;

use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::mono_font::ascii::FONT_10X20;
use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::prelude::{Dimensions, Point, Size, WebColors};
use embedded_graphics::primitives::{PrimitiveStyle, Rectangle, StyledDrawable};
use embedded_graphics::text::Text;
use embedded_graphics::Drawable;
use embedded_graphics::{pixelcolor::Rgb565, prelude::RgbColor};
use embedded_hal::spi::MODE_0;
use esp_idf_svc::hal::i2c::{I2cConfig, I2cDriver};
use esp_idf_svc::hal::units::Hertz;
use esp_idf_svc::hal::{
    delay::Delay,
    gpio::PinDriver,
    prelude::Peripherals,
    spi::{
        config::{Config, DriverConfig},
        SpiSingleDeviceDriver,
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

#[cxx::bridge]
mod ffi {

}

fn main() {
    println!("hello");
    // It is necessary to call this function once. Otherwise some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_svc::sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();

    log::info!("Hello,world!");

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
    set_brightness(15, &mut display_bl, delay);

    let mut display = Builder::new(ST7789, di)
        .display_size(W, H)
        .invert_colors(ColorInversion::Inverted)
        .orientation(Orientation::new().rotate(mipidsi::options::Rotation::Deg90))
        .init(&mut delay)
        .unwrap();

    display.clear(Rgb565::BLACK).unwrap();
    set_brightness(13, &mut display_bl, delay);

    let kb_addr = 0x55;
    let _kb_brightness_cmd = 0x01;
    let _kb_alt_b_brightness_cmd = 0x02;

    let i2c_sda = peripherals.pins.gpio18;
    let i2c_scl = peripherals.pins.gpio8;

    let config = I2cConfig::new().baudrate(Hertz(100_000));
    let mut i2c = I2cDriver::new(peripherals.i2c0, i2c_sda, i2c_scl, &config).unwrap();

    let mut buf: [u8; 1] = [0];

    let mut style = PrimitiveStyle::new();
    style.fill_color = Some(Rgb565::CSS_DARK_GRAY);
    let mut erase_style = style;
    erase_style.fill_color = Some(Rgb565::BLACK);
    let text_style = MonoTextStyle::new(&FONT_10X20, Rgb565::WHITE);

    println!("waiting for data");
    let mut cursor = Point::new(30, 30);

    let cursor_tl = |c: &Point| *c - Point::new(0, 15);

    let mut message = String::new();
    let mut cursor_rect = Rectangle::new(cursor_tl(&cursor), Size::new(2, 20));
    cursor_rect.draw_styled(&style, &mut display).unwrap();
    loop {
        if i2c.read(kb_addr, &mut buf, 100000).is_ok() && buf[0] > 0 {
            cursor_rect.draw_styled(&erase_style, &mut display).unwrap();
            let c = buf[0] as char;
            match c {
                '\x08' => {
                    if let Some(c) = message.chars().last() {
                        let c_str = c.to_string();
                        if c == '\n' {
                            message.pop();
                            let last_line = message.lines().last().unwrap_or("");
                            let c_txt_bounds = Text::new(last_line, Point::default(), text_style)
                                .bounding_box()
                                .size;
                            cursor.x = (c_txt_bounds.width + 30) as i32;
                            cursor.y -= 20;
                        } else {
                            let mut c_txt = Text::new(&c_str, Point::default(), text_style);
                            cursor.x -= c_txt.bounding_box().size.width as i32; // no way a character is going to be 2M pixels wide
                            c_txt.position = cursor;
                            c_txt.character_style.text_color = Some(Rgb565::BLACK);
                            c_txt.draw(&mut display).unwrap();
                            message.pop();
                        }
                    }
                }
                '\x0d' => {
                    println!("newline");
                    message.push('\n');
                    cursor.y += 20;
                    cursor.x = 30;
                }
                _ => {
                    message.push(c);
                    cursor = Text::new(&c.to_string(), cursor, text_style)
                        .draw(&mut display)
                        .unwrap();
                }
            }
            cursor_rect.top_left = cursor_tl(&cursor);
            cursor_rect.draw_styled(&style, &mut display).unwrap();
        }

        delay.delay_ms(100);
    }
}

static DISPLAY_BUFFER: StaticCell<[u8; 1024]> = StaticCell::new();
static DISPLAY_LEVEL: Mutex<u8> = Mutex::new(16);

/// 0: display off
/// 16: maximum brightness
fn set_brightness(level: u8, bl: &mut impl embedded_hal::digital::OutputPin, delay: Delay) {
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
