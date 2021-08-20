use embedded_graphics::prelude::*;

use epd_waveshare::prelude::*;

use image;
use std::convert::TryInto;

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

pub struct OctDither<P, C>(image::ImageBuffer<P, C>, OctColorMap<P>, Point)
where
    P: image::Pixel;

impl OctDither<image::Rgb<u8>, Vec<u8>> {
    pub fn new(img: image::DynamicImage, map: OctColorMap<image::Rgb<u8>>, pt: Point) -> Self {
        let mut rgb = img.into_rgb8();
        image::imageops::colorops::dither(&mut rgb, &map);
        OctDither(rgb, map, pt)
    }

    pub fn iter(
        &self,
    ) -> embedded_graphics::iterator::contiguous::IntoPixels<DitherIter<image::Rgb<u8>>> {
        DitherIter {
            iter: self.0.pixels(),
            map: &self.1,
        }
        .into_pixels(&embedded_graphics::primitives::rectangle::Rectangle {
            top_left: self.2,
            size: Size {
                height: self.0.height(),
                width: self.0.width(),
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
            .map(|p| self.map.colors[self.map.index_of(&p)].0)
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
                    (
                        *c,
                        image::Rgb::<u8>([rgb.0.into(), rgb.1.into(), rgb.2.into()]),
                    )
                })
                .collect(),
        };
        Self::new(img, map, pt)
    }
}

// impl <P, C>embedded_graphics::image::ImageDimensions for OctDither<P, C>
// where P: image::Pixel + 'static,
//     C: std::ops::Deref<Target=[P::Subpixel]>
// {
//     fn width(&self) -> u32 {
//         self.0.width()
//     }
//     fn height(&self) -> u32 {
//         self.0.height()
//     }
// }

// pub struct PixelIter<'a, P>
// where
//     P: image::Pixel,
// {
//     iter: image::buffer::EnumeratePixels<'a, P>,
//     map: &'a OctColorMap<P>,
// }

pub trait OctSubpixel:
    image::Primitive + std::cmp::Ord + std::iter::Sum + TryInto<u32> + 'static
{
}

// impl<'a, P> Iterator for PixelIter<'a, P>
// where
//     P: image::Pixel,
//     <P as image::Pixel>::Subpixel: OctSubpixel,
// {
//     type Item = Pixel<OctColor>;

//     fn next(&mut self) -> Option<Self::Item> {
//         use image::imageops::colorops::ColorMap;
//         self.iter.next().map(|(x, y, p)| {
//             Pixel(
//                 Point::new(x as i32, y as i32),
//                 self.map.colors[self.map.index_of(&p)].0,
//             )
//         })
//     }
// }

impl OctSubpixel for u8 {}
impl OctSubpixel for u16 {}
impl OctSubpixel for u32 {}

// impl<'a, P, C> embedded_graphics::image::IntoPixelIter<OctColor> for &'a OctDither<P, C>
// where P : image::Pixel + 'static,
//       P::Subpixel: OctSubpixel,
//       C: std::ops::Deref<Target=[P::Subpixel]>
// {
//     type PixelIterator = PixelIter<'a, P>;
//     fn pixel_iter(self) -> Self::PixelIterator {
//         PixelIter {
//             iter: self.0.enumerate_pixels(),
//             map: &self.1,
//         }
//     }
// }

// impl<P, Continaer> ImageDrawable for &OctDither<P, Container> {
//     type Color = P;
//     fn draw<D>(&self, target: &mut D) -> Result<(), <D as DrawTarget>::Error>
//     where
//         D: DrawTarget<Color = Self::Color> {
//         target.fill_contiguous(
//             &self.bounding_box(),

//         )
//     }
//     fn draw_sub_image<D>(
//         &self,
//         target: &mut D,
//         area: &Rectangle
//     ) -> Result<(), <D as DrawTarget>::Error>
//     where
//         D: DrawTarget<Color = Self::Color>;
// }

// impl <P, Container> Drawable for &OctDither<P, Container>
// where
// //P: image::Pixel + 'static + PixelColor,
//     P: image::Pixel + PixelColor,
//       // P::Subpixel: OctSubpixel,
//       // Container: std::ops::Deref<Target=[P::Subpixel]>
// {
//     type Color=P;
//     type Output=();
//     fn draw<D: DrawTarget<Color=P>>(&self, display: &mut D) -> Result<Self::Output, D::Error> {
//         //let image = embedded_graphics::image::ImageRaw::new(&self.2);
//         //image.draw(display)?;
//         self.draw(display)?;
//         Ok(())
//     }
// }
