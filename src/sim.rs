use embedded_graphics::prelude::*;
use embedded_hal::{
    blocking::delay::*,
    blocking::spi::Write,
    digital::v2::{InputPin, OutputPin},
};
use embedded_hal_mock::{delay::*, pin::Mock as PinMock, spi::Mock as SpiMock};
use epd_waveshare::color::OctColor;
use epd_waveshare::{epd5in65f::*, prelude::*};

use std::sync::mpsc;
use std::thread;

struct OctSimDisplay {
    child: thread::JoinHandle<()>,
    tx: mpsc::Sender<WindowMessage>,
    color: OctColor,
}

impl Drop for OctSimDisplay {
    fn drop(&mut self) {
        self.tx
            .send(WindowMessage::Shutdown)
            .expect("Shutdown failed");
    }
}

enum WindowMessage {
    Update(Vec<u8>),
    Refresh,
    Shutdown,
}

use embedded_graphics_simulator::{
    OutputSettingsBuilder, SimulatorDisplay, SimulatorEvent, Window,
};

impl<SPI, CS, BUSY, DC, RST, DELAY> WaveshareDisplay<SPI, CS, BUSY, DC, RST, DELAY>
    for OctSimDisplay
where
    SPI: Write<u8>,
    CS: OutputPin,
    BUSY: InputPin,
    DC: OutputPin,
    RST: OutputPin,
    DELAY: DelayMs<u8>,
{
    type DisplayColor = OctColor;
    fn new(
        _spi: &mut SPI,
        _cs: CS,
        _busy: BUSY,
        _dc: DC,
        _rst: RST,
        _delay: &mut DELAY,
    ) -> Result<Self, SPI::Error> {
        let (tx, rx) = mpsc::channel();
        let color = OctColor::White;
        let child = std::thread::spawn(move || {
            let output_settings = OutputSettingsBuilder::new().build();
            let mut window = Window::new("Simulator", &output_settings);

            let mut display = SimulatorDisplay::<OctColor>::with_default_color(
                Size {
                    width: WIDTH,
                    height: HEIGHT,
                },
                color,
            );

            window.update(&display);
            'running: loop {
                while let Ok(msg) = rx.try_recv() {
                    match msg {
                        WindowMessage::Refresh => window.update(&display),
                        WindowMessage::Shutdown => {
                            break 'running;
                        }
                        WindowMessage::Update(v) => {
                            use embedded_graphics::{image::Image, prelude::*};

                            let image =
                                embedded_graphics::image::ImageRaw::<OctColor>::new(&v, WIDTH);
                            let image = Image::new(&image, Point::zero());
                            image.draw(&mut display).unwrap();
                            window.update(&display);
                        }
                    }
                    /* do message */
                }

                for event in window.events() {
                    match event {
                        SimulatorEvent::MouseButtonUp { point, .. } => {
                            println!("Click event at ({}, {})", point.x, point.y);
                        }
                        SimulatorEvent::Quit => break 'running,
                        _ => {}
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
        });
        let new = Self { child, tx, color };
        Ok(new)
    }

    fn wake_up(&mut self, _spi: &mut SPI, _delay: &mut DELAY) -> Result<(), SPI::Error> {
        Ok(())
    }

    fn sleep(&mut self, _spi: &mut SPI, _delay: &mut DELAY) -> Result<(), SPI::Error> {
        std::thread::sleep(std::time::Duration::from_millis(1000));
        Ok(())
    }

    fn update_frame(
        &mut self,
        _spi: &mut SPI,
        buffer: &[u8],
        _delay: &mut DELAY,
    ) -> Result<(), SPI::Error> {
        self.tx
            .send(WindowMessage::Update(buffer.to_vec()))
            .unwrap();
        Ok(())
    }

    fn update_partial_frame(
        &mut self,
        _spi: &mut SPI,
        _buffer: &[u8],
        _x: u32,
        _y: u32,
        _width: u32,
        _height: u32,
    ) -> Result<(), SPI::Error> {
        unimplemented!();
    }

    fn display_frame(&mut self, _spi: &mut SPI, _delay: &mut DELAY) -> Result<(), SPI::Error> {
        self.tx.send(WindowMessage::Refresh).unwrap();
        Ok(())
    }

    fn update_and_display_frame(
        &mut self,
        spi: &mut SPI,
        buffer: &[u8],
        delay: &mut DELAY,
    ) -> Result<(), SPI::Error> {
        /* why is rust requiring me to specify the full type parameter version of these calls ? */
        WaveshareDisplay::<SPI, CS, BUSY, DC, RST, DELAY>::update_frame(self, spi, buffer, delay)?;
        WaveshareDisplay::<SPI, CS, BUSY, DC, RST, DELAY>::display_frame(self, spi, delay)
    }

    fn clear_frame(&mut self, _spi: &mut SPI, _delay: &mut DELAY) -> Result<(), SPI::Error> {
        Ok(())
    }

    fn set_background_color(&mut self, color: OctColor) {
        self.color = color;
    }

    fn background_color(&self) -> &OctColor {
        &self.color
    }

    fn width(&self) -> u32 {
        WIDTH
    }

    fn height(&self) -> u32 {
        HEIGHT
    }

    fn set_lut(
        &mut self,
        _spi: &mut SPI,
        _refresh_rate: Option<RefreshLut>,
    ) -> Result<(), SPI::Error> {
        unimplemented!();
    }

    fn is_busy(&self) -> bool {
        false
    }
}

pub fn create() -> (
    SpiMock,
    StdSleep,
    impl WaveshareDisplay<
        SpiMock,
        PinMock,
        PinMock,
        PinMock,
        PinMock,
        embedded_hal_mock::delay::StdSleep,
        DisplayColor = OctColor,
    >,
) {
    let mut spi = SpiMock::new(&[]);
    let cs_pin = PinMock::new(&[]);
    let busy_in = PinMock::new(&[]);
    let dc = PinMock::new(&[]);
    let rst = PinMock::new(&[]);
    let mut delay = embedded_hal_mock::delay::StdSleep::new();
    let disp = OctSimDisplay::new(&mut spi, cs_pin, busy_in, dc, rst, &mut delay)
        .map_err(|_| ())
        .unwrap();
    (spi, delay, disp)
}
