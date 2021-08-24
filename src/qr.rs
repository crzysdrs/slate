use embedded_graphics::{
    geometry::Size, pixelcolor::PixelColor, prelude::*, primitives::Rectangle,
    transform::Transform, Drawable,
};

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
}

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
            .try_for_each(|(x, y, c)| {
                let x = x as i32;
                let y = y as i32;

                display.fill_solid(
                    &rect.translate(Point::new(x * self.scale, y * self.scale)),
                    if c { self.fg_color } else { self.bg_color },
                )
            })?;
        Ok(())
    }
}
