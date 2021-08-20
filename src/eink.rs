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
    // DIN    ->    10(SPI0_MOSI)
    // CLK    ->    11(SPI0_SCK)
    // CS     ->    8(SPI0_CS0)
    // DC     ->    25
    // RST    ->    17
    // BUSY   ->    24
    let mut spi = rppal::spi::Spi::new(
        rppal::spi::Bus::Spi0,
        rppal::spi::SlaveSelect::Ss0,
        /*clock speed */ 4_000_000,
        rppal::spi::Mode::Mode0,
    )
    .expect("spi failure");
    let gpio = Gpio::new().expect("gpio failure");
    let cs_pin = gpio.get(8).expect("failed to get pin").into_output();
    let busy_in = gpio.get(24).expect("failed to get pin").into_input();
    let dc = gpio.get(25).expect("failed to get pin").into_output();
    let rst = gpio.get(17).expect("failed to get pin").into_output();
    let mut delay = rppal::hal::Delay::new();

    let disp = Epd5in65f::new(&mut spi, cs_pin, busy_in, dc, rst, &mut delay)
        .map_err(|_| ())
        .unwrap();
    (spi, delay, disp)
}
