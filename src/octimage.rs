use embedded_graphics::prelude::*;
use epd_waveshare::prelude::*;
use std::convert::TryInto;

pub trait OctSubpixel:
    image::Primitive + std::cmp::Ord + std::iter::Sum + TryInto<u32> + 'static
{
}

impl OctSubpixel for u8 {}
impl OctSubpixel for u16 {}
impl OctSubpixel for u32 {}

#[derive(Clone)]
pub struct OctColorMap<T> {
    pub colors: Vec<(OctColor, T)>,
}

impl<T> image::imageops::colorops::ColorMap for OctColorMap<T>
where
    T: image::Pixel,
    T::Subpixel: OctSubpixel,
{
    type Color = T;
    fn index_of(&self, color: &Self::Color) -> usize {
        self.colors
            .iter()
            .map(|(_, c)| c)
            .map(|c| {
                c.channels()
                    .iter()
                    .zip(color.channels())
                    .map(|(c1, c2)| {
                        (std::cmp::max(*c1, *c2) - std::cmp::min(*c1, *c2))
                            .try_into()
                            .map_err(|_| ())
                            .unwrap()
                    })
                    .sum::<u32>()
            })
            .enumerate()
            .min_by_key(|(_i, s)| *s)
            .unwrap()
            .0
    }
    fn map_color(&self, color: &mut Self::Color) {
        *color = self.colors[self.index_of(color)].1;
    }
}

pub struct OctDither<P, C>
where
    P: image::Pixel,
{
    buffer: image::ImageBuffer<P, C>,
    map: OctColorMap<P>,
    top_left: Point,
}

impl OctDither<image::Rgb<u8>, Vec<u8>> {
    pub fn new(img: image::DynamicImage, map: OctColorMap<image::Rgb<u8>>, pt: Point) -> Self {
        let mut rgb = img.into_rgb8();
        image::imageops::colorops::dither(&mut rgb, &map);
        OctDither {
            buffer: rgb,
            map,
            top_left: pt,
        }
    }

    pub fn output(&self) -> image::DynamicImage {
        use image::imageops::ColorMap;
        let mut out = self.buffer.clone();
        out.pixels_mut().for_each(|p| {
            let rgb = self.map.colors[self.map.index_of(p)].0.rgb();
            *p = image::Rgb([rgb.0, rgb.1, rgb.2]);
        });

        image::DynamicImage::ImageRgb8(out)
    }

    pub fn iter(
        &self,
    ) -> embedded_graphics::iterator::contiguous::IntoPixels<DitherIter<image::Rgb<u8>>> {
        DitherIter {
            iter: self.buffer.pixels(),
            map: &self.map,
        }
        .into_pixels(&embedded_graphics::primitives::rectangle::Rectangle {
            top_left: self.top_left,
            size: Size {
                height: self.buffer.height(),
                width: self.buffer.width(),
            },
        })
    }
}

pub struct DitherIter<'a, P>
where
    P: image::Pixel,
{
    iter: image::buffer::Pixels<'a, P>,
    map: &'a OctColorMap<P>,
}
impl<'a, P> Iterator for DitherIter<'a, P>
where
    P: image::Pixel,
    P::Subpixel: OctSubpixel,
{
    type Item = OctColor;
    fn next(&mut self) -> Option<Self::Item> {
        use image::imageops::ColorMap;
        self.iter
            .next()
            .map(|p| self.map.colors[self.map.index_of(p)].0)
    }
}

impl OctDither<image::Rgb<u8>, Vec<u8>> {
    pub fn new_default(img: image::DynamicImage, pt: Point) -> Self {
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

        let map = OctColorMap {
            colors: COLORS
                .iter()
                .map(|c| {
                    let rgb = c.rgb();
                    (*c, image::Rgb::<u8>([rgb.0, rgb.1, rgb.2]))
                })
                .collect(),
        };
        Self::new(img, map, pt)
    }
}
