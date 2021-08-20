use embedded_graphics::{
    geometry::{Dimensions, Size},
    pixelcolor::PixelColor,
    prelude::*,
    primitives::Rectangle,
    primitives::{PrimitiveStyle, PrimitiveStyleBuilder, Styled},
    transform::Transform,
    Drawable,
};

use std::marker::PhantomData;

#[derive(Clone)]
pub struct QrCode<C> {
    qr: qrcode::QrCode,
    top_left: Point,
    scale: i32,
    fg_color: C,
    bg_color: C,
}

impl<C> QrCode<C> {
    pub fn new<D>(top_left: Point, scale: i32, fg_color: C, bg_color: C, data: D) -> Self
    where
        D: AsRef<[u8]>,
    {
        QrCode {
            qr: qrcode::QrCode::new(data).unwrap(),
            top_left,
            scale,
            fg_color,
            bg_color,
        }
    }
    fn size(&self) -> Size {
        let image = self.qr.render::<char>().build();
        let width = image.lines().next().map(|l| l.len()).unwrap() as u32;
        let height = image.lines().count() as u32;
        Size {
            width: width as u32,
            height: height as u32,
        }
    }
}

// impl<C> Dimensions for QrCode<C> {
//     fn bounding_box(&self) -> Rectangle {
//         Rectangle {
//             top_left: self.top_left,
//             size: self.size(),
//         }
//     }
//     // fn top_left(&self) -> Point {
//     //     self.top_left
//     // }
//     // fn bottom_right(&self) -> Point {
//     //     self.top_left + self.size()

//     // }
//     // fn size(&self) -> Size {
//     //     let image = self.qr.render::<char>().build();
//     //     let width = image.lines().next().map(|l| l.len()).unwrap() as u32;
//     //     let height = image.lines().count() as u32;
//     //     Size { width: width * self.scale as u32, height: height * self.scale as u32}
//     // }
// }

impl<C> Transform for QrCode<C>
where
    C: Clone,
{
    fn translate(&self, by: Point) -> Self {
        let mut new = (*self).clone();
        new.top_left += by;
        new
    }
    fn translate_mut(&mut self, by: Point) -> &mut Self {
        self.top_left += by;
        self
    }
}

impl<C> Drawable for QrCode<C>
where
    C: PixelColor,
{
    type Color = C;
    type Output = ();
    fn draw<D: DrawTarget<Color = C>>(&self, display: &mut D) -> Result<Self::Output, D::Error> {
        let image = self.qr.render::<char>().build();
        // let width = image.lines().next().map(|l| l.len()).unwrap() as u32;
        // let height = image.lines().count() as u32;

        // let on_style = PrimitiveStyleBuilder::new()
        //     .stroke_color(self.fg_color)
        //     .fill_color(self.fg_color)
        //     .build();

        // let off_style = PrimitiveStyleBuilder::new()
        //     .stroke_color(self.bg_color)
        //     .fill_color(self.bg_color)
        //     .build();

        let rect = Rectangle::new(
            self.top_left,
            Size {
                height: self.scale as u32,
                width: self.scale as u32,
            },
        );

        image
            .lines()
            .enumerate()
            .flat_map(|(y, l)| l.chars().enumerate().map(move |(x, c)| (x, y, c == '█')))
            .map(|(x, y, c)| {
                let x = x as i32;
                let y = y as i32;

                display.fill_solid(
                    &rect.translate(Point::new(x * self.scale, y * self.scale)),
                    if c { self.fg_color } else { self.bg_color },
                )
            })
            .collect::<Result<_, _>>()?;
        Ok(())
    }
}
