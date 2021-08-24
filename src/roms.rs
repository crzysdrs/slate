use serde_derive::Deserialize;
use std::io::Read;
use std::io::Result as IOResult;
use std::path::{Path, PathBuf};

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
                "Invalid Extension".to_string(),
            )),
        },
    }
}

pub fn get_frames<P>(
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
        Some(gb::cycles::SECOND / 65536),
    );

    let mut frame_count = 0;
    let frames = frames
        .iter()
        .flat_map(|f| {
            let timeout = Some(gb::cycles::SECOND);
            let skip_frames = frame_count..*f;
            for _ in skip_frames {
                match gb.step(timeout, &mut PeripheralData::new(None, None, None)) {
                    gb::gb::GBReason::VSync => {
                        frame_count += 1;
                    }
                    gb::gb::GBReason::Timeout | gb::gb::GBReason::Dead => {
                        return None;
                    }
                }
            }
            let mut image = image::RgbaImage::new(160, 144);
            match gb.step(
                timeout,
                &mut PeripheralData::new(Some(&mut image), None, None),
            ) {
                gb::gb::GBReason::VSync => {
                    frame_count += 1;
                }
                gb::gb::GBReason::Timeout | gb::gb::GBReason::Dead => {
                    return None;
                }
            }
            Some(image)
        })
        .collect();

    Ok(frames)
}

#[derive(Deserialize)]
pub struct RomData {
    pub roms: PathBuf,
    boxart: PathBuf,
}

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
enum Lang {
    En,
    Jp,
    Other,
}

#[allow(clippy::upper_case_acronyms)]
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
pub struct Rom {
    pub path: PathBuf,
    lang: Vec<Country>,
    pub boxart: Option<PathBuf>,
}

impl RomData {
    pub fn roms(&self) -> Vec<Rom> {
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
                    .captures_iter(name)
                    .flat_map(|cap| {
                        cap[1]
                            .split(',')
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
                        let other = matches!(
                            attr,
                            "GBC"
                                | "GB Compatible"
                                | "SGB Enhanced"
                                | "Rev A"
                                | "Rev B"
                                | "Beta"
                                | "Rumble Version"
                                | "NP"
                                | "Sample"
                                | "AX9P"
                                | "AP9P"
                                | "Rev 1"
                                | "Rev 2"
                                | "Rev 3"
                                | "Rev AB"
                                | "DMG-N5"
                                | "DMG-EM"
                                | "HAL Laboratory"
                                | "Unl"
                                | "Activision"
                        );
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

                let name = parens.replace_all(name, "");

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
        WalkDir::new(&self.roms)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| !e.file_type().is_dir())
            .map(|f| f.path().to_owned())
            .map(|p| {
                let name = p.file_stem().unwrap().to_str().unwrap();
                let data = attr_re
                    .captures_iter(name)
                    .flat_map(|cap| {
                        cap[1]
                            .split(',')
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

                let search = attr_re.replace_all(name, "");
                let search = junk_re.replace_all(&search, "");
                let search = search.trim();
                use strsim::jaro;

                let best = art
                    .iter()
                    .filter(|x| data.get(0).map(|d| x.country.contains(d)).unwrap_or(false))
                    .map(|x| (x, jaro(&x.name, search)))
                    .filter(|x| x.1 > 0.75)
                    .max_by(|x, y| x.1.partial_cmp(&y.1).unwrap());
                Rom {
                    path: p,
                    boxart: best.map(|x| x.0.path.to_owned()),
                    lang: data,
                }
            })
            .collect::<Vec<_>>()
    }
}

#[derive(Deserialize)]
pub struct GameboyImage {
    pub screen: [(f32, f32); 4],
    pub path: PathBuf,
    pub color: bool,
}

#[derive(Deserialize)]
pub struct Config {
    pub romdata: Vec<RomData>,
    pub gameboy: Vec<GameboyImage>,
}
