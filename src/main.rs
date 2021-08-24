mod qr;
mod roms;
mod transform;
use embedded_graphics::{
    geometry::Size, prelude::*, primitives::PrimitiveStyleBuilder, primitives::Rectangle,
};
use embedded_hal::prelude::*;
use epd_waveshare::color::OctColor;
use epd_waveshare::graphics::OctDisplay;
use epd_waveshare::{epd5in65f::*, prelude::*};
use rand::seq::SliceRandom;
mod octimage;
use anyhow::{anyhow, Result};
use display::create;
use image::imageops::FilterType;
use image::io::Reader as ImageReader;
use image::DynamicImage;
use image::GenericImageView;
use image::ImageBuffer;
use imageproc::geometric_transformations::*;
use octimage::OctDither;
use qr::QrCode;
use rand::Rng;
use roms::{get_frames, Config, GameboyImage};
use std::path::PathBuf;
use transform::{Transform, Transformable};

static COLORS: [OctColor; 8] = [
    OctColor::HiZ,
    OctColor::White,
    OctColor::Black,
    OctColor::Red,
    OctColor::Green,
    OctColor::Orange,
    OctColor::Blue,
    OctColor::Yellow,
];

cfg_if::cfg_if! {
    if #[cfg(feature="spi")] {
        mod eink;
        use eink as display;
    } else if #[cfg(feature="sim")] {
        mod sim;
        use sim as display;
    } else {
        compile_error!("Wrong feature");
    }
}

fn bars<DISP, E>(display: &mut DISP, offset: usize)
where
    DISP: OctDisplay<Error = E>,
    E: std::fmt::Debug,
{
    let width = WIDTH as usize / COLORS.len();
    for (c, l) in COLORS
        .iter()
        .cycle()
        .skip(offset)
        .take(COLORS.len())
        .zip((0..WIDTH as i32).step_by(width))
    {
        let style = PrimitiveStyleBuilder::new()
            .stroke_color(*c)
            .stroke_width(3)
            .fill_color(*c)
            .build();

        Rectangle::new(
            Point::new(l, 0),
            Size {
                width: width as u32,
                height: HEIGHT as u32,
            },
        )
        .into_styled(style)
        .draw(display)
        .expect("Valid rect");
    }
    {
        use embedded_graphics::{
            mono_font::iso_8859_16::FONT_10X20,
            mono_font::MonoTextStyle,
            prelude::*,
            text::{Text, TextStyleBuilder},
        };

        let character_style = MonoTextStyle::new(&FONT_10X20, OctColor::White);
        // Create a new text style
        let text_style = TextStyleBuilder::new().build();

        // Create a text at position (20, 30) and draw it using the previously defined style
        Text::with_text_style(
            "Hello Rust!",
            Point::new(WIDTH as i32 / 7 * 3, HEIGHT as i32 / 2),
            character_style,
            text_style,
        )
        .draw(display)
        .expect("Wrote Text");
    }
}

fn place(
    img: &GameboyImage,
    screen: &ImageBuffer<image::Rgba<u8>, Vec<u8>>,
) -> ImageBuffer<image::Rgba<u8>, Vec<u8>> {
    use imageproc::geometric_transformations::*;
    let mut gb = ImageReader::open(&img.path)
        .unwrap()
        .decode()
        .unwrap()
        .to_rgba8();
    let (x, y) = screen.dimensions();
    let (x, y) = (x as f32, y as f32);
    let dim = [(0.0, 0.0), (x, 0.0), (x, y), (0.0, y)];
    let proj = Projection::from_control_points(dim, img.screen).unwrap();
    let screen = DynamicImage::ImageRgba8(screen.clone()).to_rgba8();
    let mut gb_scratch = gb.clone();
    warp_into(
        &screen,
        &proj,
        Interpolation::Bicubic,
        image::Rgba([0, 0, 0, 0]),
        &mut gb_scratch,
    );
    image::imageops::overlay(&mut gb, &gb_scratch, 0, 0);
    gb
}

use std::marker::PhantomData;
struct Controller<SPI, CS, BUSY, DC, RST, DELAY, DISP>
where
    DISP: WaveshareDisplay<SPI, CS, BUSY, DC, RST, DELAY, DisplayColor = OctColor>,
    SPI: Write<u8>,
    CS: OutputPin,
    BUSY: InputPin,
    DC: OutputPin,
    RST: OutputPin,
    DELAY: DelayMs<u8>,
    <SPI as _embedded_hal_blocking_spi_Write<u8>>::Error: std::fmt::Debug,
{
    display: Display5in65f,
    epd: DISP,
    spi: SPI,
    pub delay: DELAY,
    frames_since_clear: usize,
    _phantom: PhantomData<(RST, CS, DC, BUSY)>,
}

use embedded_hal::{
    blocking::delay::*,
    blocking::spi::Write,
    digital::v2::{InputPin, OutputPin},
};
impl<SPI, CS, BUSY, DC, RST, DELAY, DISP> Controller<SPI, CS, BUSY, DC, RST, DELAY, DISP>
where
    DISP: WaveshareDisplay<SPI, CS, BUSY, DC, RST, DELAY, DisplayColor = OctColor>,
    SPI: Write<u8>,
    CS: OutputPin,
    BUSY: InputPin,
    DC: OutputPin,
    RST: OutputPin,
    DELAY: DelayMs<u8>,
    <SPI as _embedded_hal_blocking_spi_Write<u8>>::Error: std::error::Error + Send + Sync + 'static,
{
    fn new(epd: DISP, spi: SPI, delay: DELAY) -> Result<Self> {
        let mut display = Display5in65f::default();
        display.set_rotation(DisplayRotation::Rotate270);
        let mut new = Self {
            display,
            epd,
            spi,
            delay,
            frames_since_clear: 0,
            _phantom: PhantomData,
        };
        new.wipe()?;
        Ok(new)
    }

    fn wipe(&mut self) -> Result<()> {
        self.epd.set_background_color(OctColor::HiZ);
        self.epd.clear_frame(&mut self.spi, &mut self.delay)?;
        self.frames_since_clear = 0;
        Ok(())
    }

    fn draw<F>(&mut self, f: F) -> Result<()>
    where
        F: FnOnce(&mut Display5in65f) -> Result<()>,
    {
        self.frames_since_clear += 1;
        if self.frames_since_clear > 10 {
            self.wipe()?;
            self.frames_since_clear = 0;
        }
        self.epd.set_background_color(OctColor::White);
        f(&mut self.display)?;
        self.epd
            .update_and_display_frame(&mut self.spi, self.display.buffer(), &mut self.delay)?;
        Ok(())
    }
}

impl<SPI, CS, BUSY, DC, RST, DELAY, DISP> Drop for Controller<SPI, CS, BUSY, DC, RST, DELAY, DISP>
where
    DISP: WaveshareDisplay<SPI, CS, BUSY, DC, RST, DELAY, DisplayColor = OctColor>,
    SPI: Write<u8>,
    CS: OutputPin,
    BUSY: InputPin,
    DC: OutputPin,
    RST: OutputPin,
    DELAY: DelayMs<u8>,
    <SPI as _embedded_hal_blocking_spi_Write<u8>>::Error: std::fmt::Debug,
{
    fn drop(&mut self) {
        self.epd
            .sleep(&mut self.spi, &mut self.delay)
            .expect("Couldn't sleep device");
    }
}

#[cfg(feature = "web")]
#[rocket::main]
async fn rocket() {
    println!("Rocket Launching");
    rocket::build()
        .mount("/gameboy", rocket::fs::FileServer::from("gameboy"))
        .launch()
        .await;
}

fn main() -> Result<()> {
    let path = PathBuf::from("gameboy");
    if !path.exists() {
        std::fs::create_dir(&path).expect("Directory created");
    }

    let host = gethostname::gethostname();
    let port = 7777;

    #[cfg(feature = "web")]
    let child = std::thread::spawn(move || {
        println!("Rocket Launching");
        rocket();
    });

    println!("Roms searching!");
    let toml_path = PathBuf::from("assets.toml");
    let cfg: Config = toml::from_str(&std::fs::read_to_string(&toml_path).unwrap()).unwrap();

    let roms = cfg
        .romdata
        .iter()
        .flat_map(|x| x.roms())
        .collect::<Vec<_>>();
    println!(
        "Boxart found: {}",
        roms.iter().filter(|x| x.boxart.is_some()).count()
    );
    println!("Total Roms: {}", roms.len());

    let (spi, delay, epd) = create();
    let mut controller = Controller::new(epd, spi, delay)?;

    for skip in 0..8 {
        controller.draw(|display| {
            display.set_rotation(DisplayRotation::Rotate0);
            bars(display, skip);
            Ok(())
        })?;
        controller.delay.delay_ms(1_000u32);
    }

    let mut rng = rand::thread_rng();
    loop {
        let (rom, frames) = 'has_frames: loop {
            let rom = roms.choose(&mut rng).unwrap();
            let frame_count = 10;
            let frames = std::panic::catch_unwind(|| {
                get_frames(
                    &rom.path,
                    None,
                    &(0usize..frame_count).map(|f| f * 60).collect::<Vec<_>>(),
                )
                .unwrap()
            })
            .unwrap_or_else(|_| vec![]);

            if frames.len() == frame_count {
                break 'has_frames (rom, frames);
            }
        };

        controller.draw(|display| {
            display.set_rotation(DisplayRotation::Rotate270);
            let mut base = DynamicImage::new_rgba8(HEIGHT, WIDTH);
            let bg = if rng.gen() {
                image::imageops::vertical_gradient
            } else {
                image::imageops::horizontal_gradient
            };

            let start = transform::rgba(&mut rng, Some(0xff));
            let end = transform::rgba(&mut rng, Some(0xff));
            bg(&mut base, &start, &end);

            let mut images = frames
                .iter()
                .map(|f| {
                    let f = f.clone();
                    let img = place(&cfg.gameboy[rng.gen_range(0..cfg.gameboy.len())], &f);
                    let img = DynamicImage::ImageRgba8(img);
                    img.resize(HEIGHT, WIDTH, FilterType::Gaussian)
                })
                .chain(
                    rom.boxart
                        .as_ref()
                        .map(|boxart| -> Result<DynamicImage> {
                            ImageReader::new(std::io::Cursor::new(std::fs::read(boxart).unwrap()))
                                .with_guessed_format()
                                .map_err(|e| anyhow!("{}", e))?
                                .decode()
                                .map_err(|e| anyhow!("{}", e))
                        })
                        .transpose()
                        .ok()
                        .flatten()
                        .into_iter(),
                )
                .collect::<Vec<_>>();

            images.shuffle(&mut rng);
            for img in images.into_iter() {
                let transforms = (0..rng.gen_range(1..10))
                    .map(|_| Transform::random(&mut rng, HEIGHT, WIDTH))
                    .collect::<Vec<_>>();

                let mut transformable = Transformable::new(img);
                for t in transforms {
                    transformable.transform(t);
                }
                let img = transformable.into_inner();
                let projection = transform::projection(&mut rng, img.dimensions(), (HEIGHT, WIDTH));
                use image::Rgba;
                let mut scratch = base.clone();
                imageproc::geometric_transformations::warp_into(
                    &img.into_rgba8(),
                    &projection,
                    Interpolation::Bicubic,
                    Rgba([0, 0, 0, 0]),
                    scratch.as_mut_rgba8().unwrap(),
                );
                image::imageops::overlay(&mut base, &scratch, 0, 0);
            }
            use sha2::Digest;
            let mut sha = sha2::Sha256::new();
            sha.update(base.as_bytes());
            let result = sha.finalize();
            let png_name = format!("{:x}.png", result);
            let output = path.join(&png_name);
            let uri = format!(
                "http://{}:{}/{}",
                host.to_string_lossy(),
                port,
                output.display()
            );
            println!("Target URL {}", uri);

            let dither = OctDither::new_default(base, Point::zero());
            let image = dither.output();
            use std::os::unix::fs::symlink;
            image.save(&output)?;
            let symlink_file = path.join("latest.png");
            std::fs::remove_file(&symlink_file)?;
            symlink(&png_name, &symlink_file)?;
            dither.iter().draw(display).unwrap();
            let code = QrCode::new(
                Point::new(0, 0),
                2,
                OctColor::Black,
                OctColor::White,
                uri.as_bytes(),
            );

            Drawable::draw(&code, display).unwrap();
            Ok(())
        })?;
        controller.delay.delay_ms(120_000u32);
    }
}
