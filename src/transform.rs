use image::GenericImageView;
use image::{DynamicImage, Rgba};
use imageproc::geometric_transformations::Projection;
use rand::prelude::*;
use rand::Rng;

#[derive(Copy, Clone)]
pub enum Transform {
    Edges(f32, f32, Rgba<u8>, Rgba<u8>),
    Noise(f64, f64, u64),
    Threshold(u8, Rgba<u8>, Rgba<u8>),
    Blur(f32),
    Warp(Projection, Rgba<u8>),
}

pub fn rgba<R>(mut rng: R) -> Rgba<u8>
where
    R: Rng,
{
    Rgba([rng.gen(), rng.gen(), rng.gen(), rng.gen()])
}

pub fn mask(target: &mut image::RgbaImage, mask: &DynamicImage) {
    target
        .pixels_mut()
        .map(|p| p)
        .zip(mask.pixels().map(|(_, _, p)| p))
        .for_each(|(p, m)| {
            p[3] = m[3];
        });
}

pub fn projection<R>(mut rng: R, img_dim: (u32, u32), screen_dim: (u32, u32)) -> Projection
where
    R: Rng,
{
    //projections applied in reverse order
    Projection::translate(
        rng.gen_range(-(img_dim.0 as f32)..screen_dim.0 as f32),
        rng.gen_range(-(img_dim.1 as f32)..screen_dim.1 as f32),
    ) * Projection::translate(img_dim.0 as f32 / 2.0, img_dim.1 as f32 / 2.0)
        * Projection::rotate(rng.gen_range(0.0..2.0 * std::f32::consts::PI))
        * Projection::scale(rng.gen_range(0.5..1.5), rng.gen_range(0.5..1.5))
        * Projection::translate(-(img_dim.0 as f32) / 2.0, -(img_dim.1 as f32) / 2.0)
}

impl Transform {
    pub fn random<R>(mut rng: R, height: u32, width: u32) -> Transform
    where
        R: Rng,
    {
        let v: Vec<(_, Box<Fn(&mut R) -> _>)> = vec![
            (
                1,
                Box::new(|mut rng| {
                    Transform::Edges(
                        rng.gen_range(0.0..30.0),
                        rng.gen_range(70.0..100.0),
                        rgba(&mut rng),
                        rgba(&mut rng),
                    )
                }),
            ),
            (
                5,
                Box::new(|rng| {
                    Transform::Noise(rng.gen_range(0.0..5.0), rng.gen_range(0.0..3.0), rng.gen())
                }),
            ),
            (
                1,
                Box::new(|mut rng| {
                    Transform::Threshold(rng.gen_range(128..200), rgba(&mut rng), rgba(&mut rng))
                }),
            ),
            (1, Box::new(|rng| Transform::Blur(rng.gen_range(0.0..10.0)))),
        ];
        (v.choose_weighted(&mut rng, |e| e.0).expect("valid").1)(&mut rng)
    }
}

pub struct Transformable {
    image: DynamicImage,
}

impl Transformable {
    pub fn new(image: DynamicImage) -> Self {
        Self { image }
    }
    pub fn into_inner(self) -> DynamicImage {
        self.image
    }
    pub fn transform(&mut self, t: Transform) {
        match t {
            Transform::Edges(low, high, fg_color, bg_color) => {
                let gray = self.image.to_luma8();
                let tmp = DynamicImage::ImageLuma8(imageproc::edges::canny(&gray, low, high));
                let mut rgb8 = tmp.to_rgba8();
                rgb8.pixels_mut().for_each(|p| {
                    if *p == Rgba([0, 0, 0, 0xff]) {
                        *p = bg_color;
                    } else {
                        *p = fg_color;
                    }
                });
                mask(&mut rgb8, &self.image);
                self.image = DynamicImage::ImageRgba8(rgb8);
            }
            Transform::Noise(mean, stddev, seed) => {
                let mut image = self.image.to_rgba8();
                imageproc::noise::gaussian_noise_mut(&mut image, mean, stddev, seed);
                mask(&mut image, &self.image);
                self.image = DynamicImage::ImageRgba8(image);
            }
            Transform::Threshold(threshold, fg_color, bg_color) => {
                let mut image = self.image.to_luma8();
                imageproc::contrast::threshold_mut(&mut image, threshold);
                let mut rgb8 = DynamicImage::ImageLuma8(image).into_rgba8();
                rgb8.pixels_mut().for_each(|p| {
                    if *p == Rgba([0, 0, 0, 0xff]) {
                        *p = bg_color;
                    } else {
                        *p = fg_color;
                    }
                });
                mask(&mut rgb8, &self.image);
                self.image = DynamicImage::ImageRgba8(rgb8);
            }
            Transform::Blur(sigma) => {
                let mut image = self.image.to_rgba8();
                let image = imageproc::filter::gaussian_blur_f32(&image, sigma);
                self.image = DynamicImage::ImageRgba8(image);
            }
            Transform::Warp(projection, default_color) => {
                let mut image = self.image.to_rgba8();
                let image = imageproc::geometric_transformations::warp(
                    &image,
                    &projection,
                    imageproc::geometric_transformations::Interpolation::Bicubic,
                    default_color,
                );
                self.image = DynamicImage::ImageRgba8(image);
            }
        }
    }
}
