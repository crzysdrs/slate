use image;
use std::convert::TryInto;

struct OctColorMap<T>
where T: image::Primitive
{
    colors : Vec<image::Rgb<T>>
}

impl <T> image::imageops::colorops::ColorMap for OctColorMap<T>
where T: image::Primitive+ std::cmp::Ord + std::iter::Sum + TryInto<u32> + 'static
{
    type Color = image::Rgb<T>;
    fn index_of(&self, color: &Self::Color) -> usize {
        use image::Pixel;            
        self.colors.iter().map(
            |c| c.channels().iter().zip(color.channels())
                .map(|(c1, c2)| (std::cmp::max(*c1, *c2) - std::cmp::min(*c1, *c2)).try_into().map_err(|_| ()).unwrap())
                .sum::<u32>()
        ).enumerate()
            .min_by_key(|(_i, s)| *s)
            .unwrap().0
    }
    fn map_color(&self, color: &mut Self::Color) {
        *color = self.colors[self.index_of(color)];
    }    
}

struct RgbWrap(image::RgbImage, OctColorMap<u8>);

impl embedded_graphics::image::ImageDimensions for RgbWrap {
    fn width(&self) -> u32 {
        self.0.width()
    }
    fn height(&self) -> u32 {
        self.0.height()
    }
}

struct PixelIter<'a, P>
where P : image::Pixel
{
    iter: image::buffer::EnumeratePixels<'a, P>,
    map: &'a OctColorMap<<P as image::Pixel>::Subpixel>
}


trait OctSubpixel : image::Primitive+ std::cmp::Ord + std::iter::Sum + TryInto<u32> + 'static {}

impl <'a, P> Iterator for PixelIter<'a, P>
where P : image::Pixel,
<P as image::Pixel>::Subpixel : OctSubpixel
{       
    type Item =  Pixel<OctColor>;

    fn next(&mut self) -> Option<Self::Item> {
        use image::imageops::colorops::ColorMap;
        self.iter.next().map(
            |(x, y, p)|
            Pixel(
                Point::new(x as i32, y as i32),
                COLORS[self.map.index_of(&p.to_rgb())]
            )
        )
    }
}
impl OctSubpixel for u8 {}
impl <'a> embedded_graphics::image::IntoPixelIter<OctColor> for &'a RgbWrap
{
    type PixelIterator = PixelIter<'a, image::Rgb<u8>>;
    fn pixel_iter(self) -> Self::PixelIterator {
        PixelIter {
            iter: self.0.enumerate_pixels(),
            map: &self.1,
        }
    }
}
