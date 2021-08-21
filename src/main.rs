mod qr;
mod transform;
use embedded_graphics::{
    geometry::Size, prelude::*, primitives::PrimitiveStyleBuilder, primitives::Rectangle,
};
use embedded_hal::prelude::*;
use epd_waveshare::color::OctColor;
use epd_waveshare::graphics::OctDisplay;
use epd_waveshare::{epd5in65f::*, prelude::*};
use image;

mod octimage;
use octimage::OctDither;
use std::io::Result as IOResult;

use std::io::Read;
use std::path::{Path, PathBuf};

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

use display::create;

use gb;

fn open_rom<P>(rom: P) -> IOResult<Vec<u8>>
where
    P: AsRef<Path>,
{
    let rom = rom.as_ref();
    match rom.extension() {
        None => Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("Missing file extension {}", rom.display()),
        )),
        Some(ext) => match ext.to_str() {
            Some("zip") => {
                let f = std::fs::File::open(rom)?;
                let mut z = zip::ZipArchive::new(f)?;
                let mut res = None;
                for c_id in 0..z.len() {
                    if let Ok(mut c_file) = z.by_index(c_id) {
                        if c_file.name().ends_with(".gb") || c_file.name().ends_with(".gbc") {
                            let mut buf = Vec::new();
                            c_file.read_to_end(&mut buf)?;
                            res = Some(buf);
                        }
                    }
                }
                if let Some(buf) = res {
                    Ok(buf)
                } else {
                    Err(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        "No rom file found in archive",
                    ))
                }
            }
            Some("gb") | Some("gbc") => Ok(std::fs::read(rom)?),
            Some(e) => Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Unknown Extension {}", e),
            )),
            None => Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Invalid Extension"),
            )),
        },
    }
}

fn get_frames<P>(
    cart: P,
    palette: Option<usize>,
    frames: &[usize],
) -> IOResult<Vec<image::ImageBuffer<image::Rgba<u8>, Vec<u8>>>>
where
    P: AsRef<Path>,
{
    use gb::peripherals::PeripheralData;
    let cart = gb::cart::Cart::new(open_rom(cart)?);
    let trace = false;
    let boot_rom = None;
    let mut gb = gb::gb::GB::new(
        cart,
        trace,
        boot_rom,
        palette,
        Some((gb::cycles::SECOND / 65536).into()),
    );

    let mut frame_count = 0;
    let frames = frames
        .iter()
        .map(|f| {
            for _ in frame_count..*f {
                'discard_frame: loop {
                    match gb.step(None, &mut PeripheralData::new(None, None, None)) {
                        gb::gb::GBReason::VSync => {
                            frame_count += 1;
                            break 'discard_frame;
                        }
                        _ => {}
                    }
                }
            }
            let mut image = image::RgbaImage::new(160, 144);
            'frame: loop {
                match gb.step(None, &mut PeripheralData::new(Some(&mut image), None, None)) {
                    gb::gb::GBReason::VSync => {
                        frame_count += 1;
                        break 'frame;
                    }
                    _ => {}
                }
            }
            image
        })
        .collect();

    Ok(frames)
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
        let text_style = TextStyleBuilder::new()
            //.text_color(OctColor::Black)
            //.background_color(OctColor::White)
            .build();

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
    // Display updated frame
}

#[derive(Deserialize)]
struct RomData {
    roms: PathBuf,
    boxart: PathBuf,
}

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
enum Lang {
    En,
    Jp,
    Other,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum Country {
    USA,
    Japan,
    Other,
}

#[derive(Debug, Clone)]
struct Art {
    name: String,
    path: PathBuf,
    lang: Vec<Lang>,
    country: Vec<Country>,
}

#[derive(Debug)]
struct Rom {
    path: PathBuf,
    lang: Vec<Country>,
    boxart: Option<PathBuf>,
}

impl RomData {
    fn roms(&self) -> Vec<Rom> {
        use regex::Regex;
        use walkdir::WalkDir;

        let parens = Regex::new(r"\(([^)]+?)\)").unwrap();
        let art = WalkDir::new(&self.boxart)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| !e.file_type().is_dir())
            .map(|f| f.path().to_owned())
            .map(|p| {
                let name = p.file_stem().unwrap().to_str().unwrap();
                let data = parens
                    .captures_iter(&name)
                    .flat_map(|cap| {
                        cap[1]
                            .split(",")
                            .map(|x| x.to_string())
                            .collect::<Vec<_>>()
                            .into_iter()
                    })
                    .fold((vec![], vec![]), |mut state, attr| {
                        let attr = attr.trim();
                        let lang = match attr {
                            "En" => Some(Lang::En),
                            "Ja" => Some(Lang::Jp),
                            "Fr" | "De" | "Es" | "It" | "Nl" | "Pt" | "Sv" | "No" | "Da" | "Fi"
                            | "Zh" => Some(Lang::Other),
                            _ => None,
                        };
                        let other = match attr {
                            "GBC" | "GB Compatible" | "SGB Enhanced" | "Rev A" | "Rev B"
                            | "Beta" | "Rumble Version" | "NP" | "Sample" | "AX9P" | "AP9P"
                            | "Rev 1" | "Rev 2" | "Rev 3" | "Rev AB" | "DMG-N5" | "DMG-EM"
                            | "HAL Laboratory" | "Unl" | "Activision" => true,
                            _ => false,
                        };
                        let country = match attr {
                            "USA" => Some(Country::USA),
                            "Japan" => Some(Country::Japan),
                            "Canada" | "Sweden" | "Netherlands" | "Korea" | "World" | "Spain"
                            | "Europe" | "Australia" | "Germany" | "France" | "Italy" => {
                                Some(Country::Other)
                            }
                            _ => None,
                        };

                        assert!(
                            country.is_some() || other || lang.is_some(),
                            "Metadata for {} {}",
                            name,
                            attr
                        );
                        if let Some(lang) = lang {
                            state.0.push(lang);
                        }
                        if let Some(country) = country {
                            state.1.push(country);
                        }
                        state
                    });

                let name = parens.replace_all(&name, "");

                Art {
                    path: p.clone(),
                    name: name.trim().to_string(),
                    lang: data.0,
                    country: data.1,
                }
            })
            .collect::<Vec<_>>();

        let attr_re = Regex::new(r"\(([^)]+?)\)").unwrap();
        let junk_re = Regex::new(r"\[([^]]+?)\]").unwrap();
        let roms = WalkDir::new(&self.roms)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| !e.file_type().is_dir())
            .map(|f| f.path().to_owned())
            .map(|p| {
                let name = p.file_stem().unwrap().to_str().unwrap();
                let data = attr_re
                    .captures_iter(&name)
                    .flat_map(|cap| {
                        cap[1]
                            .split(",")
                            .map(|x| x.to_string())
                            .collect::<Vec<_>>()
                            .into_iter()
                    })
                    .fold(vec![], |mut state, attr| {
                        let lang = match attr.as_str() {
                            "J" => Some(Country::Japan),
                            "World" | "UE" | "U" => Some(Country::USA),
                            "E" | "Sw" | "G" => Some(Country::Other),
                            _ => None,
                        };
                        if let Some(lang) = lang {
                            state.push(lang);
                        }
                        state
                    });

                let search = attr_re.replace_all(&name, "");
                let search = junk_re.replace_all(&search, "");
                let search = search.trim();
                use strsim::jaro;

                let best = art
                    .iter()
                    .filter(|x| {
                        data.iter()
                            .next()
                            .map(|d| x.country.contains(&d))
                            .unwrap_or(false)
                    })
                    .map(|x| (x, jaro(&x.name, search)))
                    .filter(|x| x.1 > 0.75)
                    .max_by(|x, y| x.1.partial_cmp(&y.1).unwrap());
                Rom {
                    path: p,
                    boxart: best.map(|x| x.0.path.to_owned()),
                    lang: data,
                }
            })
            .collect::<Vec<_>>();

        roms
    }
}

use serde_derive::Deserialize;
use toml;

#[derive(Deserialize)]
struct GameboyImage {
    screen: [(f32, f32); 4],
    path: PathBuf,
    color: bool,
}

#[derive(Deserialize)]
struct Config {
    romdata: Vec<RomData>,
    gameboy: Vec<GameboyImage>,
}

use image::ImageBuffer;

fn place(
    img: &GameboyImage,
    screen: &ImageBuffer<image::Rgba<u8>, Vec<u8>>,
) -> ImageBuffer<image::Rgba<u8>, Vec<u8>> {
    use image::io::Reader as ImageReader;
    use image::DynamicImage;
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

fn main() -> Result<(), ()> {
    println!("Hello, world!");
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

    let rom =
        "/home/crzysdrs/roms/cgb/Legend of Zelda, The - Link's Awakening DX (U) (V1.2) [C][!].zip";
    let frames = get_frames(
        &rom,
        None,
        &(0usize..10).map(|f| f * 60).collect::<Vec<_>>(),
    )
    .unwrap();

    let (mut spi, mut delay, mut epd) = create();

    println!("Acquired SPI");
    println!("Pins acquired");
    // Use display graphics from embedded-graphics
    let mut display = Display5in65f::default();

    // for skip in 0..1 {
    //     epd.set_background_color(OctColor::HiZ);
    //     epd.clear_frame(&mut spi, &mut delay)
    //         .expect("cleared frame");
    //     epd.set_background_color(*COLORS.iter().cycle().skip(skip).next().unwrap());
    //     bars(&mut display, skip);
    //     epd.clear_frame(&mut spi, &mut delay)
    //         .expect("cleared frame");
    //     epd.update_frame(&mut spi, &display.buffer(), &mut delay)
    //         .map_err(|_| ())?;
    //     epd.display_frame(&mut spi, &mut delay).map_err(|_| ())?;
    //     delay.delay_ms(1_000u32);
    //     //return Ok(());
    // }

    use image::io::Reader as ImageReader;

    use image::imageops::FilterType;
    use image::DynamicImage;

    use imageproc::geometric_transformations::*;
    use rand::Rng;
    use std::f32::consts::PI;
    use std::io::Cursor;
    use transform::{Transform, Transformable};
    let mut rng = rand::thread_rng();

    display.set_rotation(DisplayRotation::Rotate270);
    // for i in 0..20 {
    //     let img = ImageReader::new(Cursor::new(include_bytes!(
    //         "/home/crzysdrs/downloads/metroid.png"
    //     )))
    //     .with_guessed_format()
    //     .unwrap()
    //     .decode()
    //     .unwrap();

    //     let img = img.resize(HEIGHT, WIDTH, FilterType::Gaussian);
    //     let mut new = DynamicImage::new_rgba8(HEIGHT, WIDTH);
    //     image::imageops::overlay(&mut new, &img, 0, 0);
    //     let img = new;
    //     let transforms = (0..rng.gen_range(1..10))
    //         .map(|_| Transform::random(&mut rng, HEIGHT, WIDTH))
    //         .collect::<Vec<_>>();

    //     let mut transformable = Transformable::new(img);
    //     for t in transforms {
    //         transformable.transform(t);
    //     }
    //     let img = transformable.into_inner();
    //     //let resized = img.resize(HEIGHT, WIDTH, FilterType::Gaussian);

    //     let dither = OctDither::new_default(img, Point::zero());
    //     dither.iter().draw(&mut display).unwrap();

    //     epd.update_and_display_frame(&mut spi, &display.buffer(), &mut delay)
    //         .map_err(|_| ())?;
    //     delay.delay_ms(5_000u32);
    // }
    epd.set_background_color(OctColor::HiZ);
    epd.clear_frame(&mut spi, &mut delay)
        .expect("cleared frame");

    epd.set_background_color(OctColor::White);
    for _ in 0..20 {
        let mut base = DynamicImage::new_rgba8(HEIGHT, WIDTH);
        let bg = if rng.gen() {
            image::imageops::vertical_gradient
        } else {
            image::imageops::horizontal_gradient
        };

        bg(&mut base, &transform::rgba(&mut rng), &transform::rgba(&mut rng));
        
        let use_gb = true;
        for (i, f) in frames.iter().enumerate() {
            let f = f.clone();
            let img = if use_gb {
                let mut img = place(&cfg.gameboy[rng.gen_range(0..cfg.gameboy.len())], &f);
                let mut img = DynamicImage::ImageRgba8(img);
                let mut img = img.resize(HEIGHT, WIDTH, FilterType::Gaussian);
                img
            } else {
                DynamicImage::ImageRgba8(f)
            };
            let transforms = (0..rng.gen_range(1..10))
                .map(|_| Transform::random(&mut rng, HEIGHT, WIDTH))
                .collect::<Vec<_>>();

            let mut transformable = Transformable::new(img);
            for t in transforms {
                transformable.transform(t);
            }
            let img = transformable.into_inner();
            use image::GenericImageView;
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
        let dither = OctDither::new_default(base, Point::zero());
        dither.iter().draw(&mut display).unwrap();
        epd.update_frame(&mut spi, &display.buffer(), &mut delay)
            .map_err(|_| ())?;
        epd.display_frame(&mut spi, &mut delay).map_err(|_| ())?;
        delay.delay_ms(2000u32);
    }
    use qr::QrCode;

    let code = QrCode::new(
        Point::new(0, 0),
        2,
        OctColor::Black,
        OctColor::White,
        b"https://crzysdrs.net",
    );

    Drawable::draw(&code, &mut display).unwrap();

    epd.update_frame(&mut spi, &display.buffer(), &mut delay)
        .map_err(|_| ())?;
    println!("Updated Frame");
    epd.display_frame(&mut spi, &mut delay).map_err(|_| ())?;

    //Set the EPD to sleep
    epd.sleep(&mut spi, &mut delay).map_err(|_| ())?;
    println!("Sleep");
    Ok(())
}
