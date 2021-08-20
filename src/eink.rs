use epd_waveshare::color::OctColor;
use rppal::gpio::Gpio;

use epd_waveshare::{epd5in65f::*, prelude::*};

pub fn create() -> (
    rppal::spi::Spi,
    rppal::hal::Delay,
    impl WaveshareDisplay<
        rppal::spi::Spi,
        rppal::gpio::OutputPin,
        rppal::gpio::InputPin,
        rppal::gpio::OutputPin,
        rppal::gpio::OutputPin,
        rppal::hal::Delay,
        DisplayColor = OctColor,
    >,
) {
    //const DIN_PIN : u8 = 10;
    //const CLK_PIN : u8 = 11;
    const CS_PIN: u8 = 8;
    const DC_PIN: u8 = 25;
    const RST_PIN: u8 = 17;
    const BUSY_PIN: u8 = 24;
    const CLOCK_SPEED: u32 = 4_000_000;
    let mut spi = rppal::spi::Spi::new(
        rppal::spi::Bus::Spi0,
        rppal::spi::SlaveSelect::Ss0,
        CLOCK_SPEED,
        rppal::spi::Mode::Mode0,
    )
    .expect("spi failure");
    let gpio = Gpio::new().expect("gpio failure");
    let cs_pin = gpio.get(CS_PIN).expect("failed to get pin").into_output();
    let busy_in = gpio.get(BUSY_PIN).expect("failed to get pin").into_input();
    let dc = gpio.get(DC_PIN).expect("failed to get pin").into_output();
    let rst = gpio.get(RST_PIN).expect("failed to get pin").into_output();
    let mut delay = rppal::hal::Delay::new();

    let disp = Epd5in65f::new(&mut spi, cs_pin, busy_in, dc, rst, &mut delay)
        .map_err(|_| ())
        .unwrap();
    (spi, delay, disp)
}
