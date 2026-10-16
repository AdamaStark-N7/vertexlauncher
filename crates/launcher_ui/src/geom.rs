//! The few 2D math and colour types the skin preview's scene builder works in.

use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub, SubAssign};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pos2 {
    pub x: f32,
    pub y: f32,
}

pub const fn vec2(x: f32, y: f32) -> Vec2 {
    Vec2 { x, y }
}

pub const fn pos2(x: f32, y: f32) -> Pos2 {
    Pos2 { x, y }
}

pub fn lerp(range: std::ops::RangeInclusive<f32>, t: f32) -> f32 {
    (1.0 - t) * *range.start() + t * *range.end()
}

impl Vec2 {
    pub const ZERO: Self = vec2(0.0, 0.0);

    pub fn new(x: f32, y: f32) -> Self {
        vec2(x, y)
    }

    pub fn dot(self, other: Self) -> f32 {
        self.x * other.x + self.y * other.y
    }

    pub fn length(self) -> f32 {
        self.dot(self).sqrt()
    }

    pub fn normalized(self) -> Self {
        let length = self.length();
        if length > 0.0 && length.is_finite() { self / length } else { self }
    }

    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }
}

impl Pos2 {
    pub fn new(x: f32, y: f32) -> Self {
        pos2(x, y)
    }

    pub fn to_vec2(self) -> Vec2 {
        vec2(self.x, self.y)
    }

    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }
}

impl Add for Vec2 {
    type Output = Vec2;
    fn add(self, rhs: Vec2) -> Vec2 {
        vec2(self.x + rhs.x, self.y + rhs.y)
    }
}
impl Sub for Vec2 {
    type Output = Vec2;
    fn sub(self, rhs: Vec2) -> Vec2 {
        vec2(self.x - rhs.x, self.y - rhs.y)
    }
}
impl Mul<f32> for Vec2 {
    type Output = Vec2;
    fn mul(self, rhs: f32) -> Vec2 {
        vec2(self.x * rhs, self.y * rhs)
    }
}
impl Mul<Vec2> for f32 {
    type Output = Vec2;
    fn mul(self, rhs: Vec2) -> Vec2 {
        rhs * self
    }
}
impl Div<f32> for Vec2 {
    type Output = Vec2;
    fn div(self, rhs: f32) -> Vec2 {
        vec2(self.x / rhs, self.y / rhs)
    }
}
impl Neg for Vec2 {
    type Output = Vec2;
    fn neg(self) -> Vec2 {
        vec2(-self.x, -self.y)
    }
}
impl AddAssign for Vec2 {
    fn add_assign(&mut self, rhs: Vec2) {
        *self = *self + rhs;
    }
}
impl SubAssign for Vec2 {
    fn sub_assign(&mut self, rhs: Vec2) {
        *self = *self - rhs;
    }
}
impl Add<Vec2> for Pos2 {
    type Output = Pos2;
    fn add(self, rhs: Vec2) -> Pos2 {
        pos2(self.x + rhs.x, self.y + rhs.y)
    }
}
impl Sub<Vec2> for Pos2 {
    type Output = Pos2;
    fn sub(self, rhs: Vec2) -> Pos2 {
        pos2(self.x - rhs.x, self.y - rhs.y)
    }
}
impl Sub<Pos2> for Pos2 {
    type Output = Vec2;
    fn sub(self, rhs: Pos2) -> Vec2 {
        vec2(self.x - rhs.x, self.y - rhs.y)
    }
}
impl AddAssign<Vec2> for Pos2 {
    fn add_assign(&mut self, rhs: Vec2) {
        *self = *self + rhs;
    }
}

/// A rectangle given by its corners.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub min: Pos2,
    pub max: Pos2,
}

impl Rect {
    pub fn from_min_max(min: Pos2, max: Pos2) -> Self {
        Self { min, max }
    }

    pub fn from_min_size(min: Pos2, size: Vec2) -> Self {
        Self { min, max: min + size }
    }

    pub fn from_center_size(center: Pos2, size: Vec2) -> Self {
        Self::from_min_size(center - size * 0.5, size)
    }

    pub fn width(&self) -> f32 {
        self.max.x - self.min.x
    }

    pub fn height(&self) -> f32 {
        self.max.y - self.min.y
    }

    pub fn left_top(&self) -> Pos2 {
        self.min
    }

    pub fn right_top(&self) -> Pos2 {
        pos2(self.max.x, self.min.y)
    }

    pub fn left_bottom(&self) -> Pos2 {
        pos2(self.min.x, self.max.y)
    }

    pub fn right_bottom(&self) -> Pos2 {
        self.max
    }

    pub fn from_two_pos(a: Pos2, b: Pos2) -> Self {
        Self { min: pos2(a.x.min(b.x), a.y.min(b.y)), max: pos2(a.x.max(b.x), a.y.max(b.y)) }
    }

    pub fn size(&self) -> Vec2 {
        vec2(self.width(), self.height())
    }

    pub fn center(&self) -> Pos2 {
        pos2((self.min.x + self.max.x) * 0.5, (self.min.y + self.max.y) * 0.5)
    }
}

/// An sRGB colour with straight alpha, 8 bits per channel.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Color32(pub [u8; 4]);

impl Color32 {
    pub const WHITE: Self = Self([255, 255, 255, 255]);
    pub const TRANSPARENT: Self = Self([0, 0, 0, 0]);
    pub const BLACK: Self = Self([0, 0, 0, 255]);

    pub const fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self([r, g, b, 255])
    }

    pub const fn from_rgba_unmultiplied(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self([r, g, b, a])
    }

    pub fn from_gray(value: u8) -> Self {
        Self([value, value, value, 255])
    }

    pub fn r(&self) -> u8 {
        self.0[0]
    }
    pub fn g(&self) -> u8 {
        self.0[1]
    }
    pub fn b(&self) -> u8 {
        self.0[2]
    }
    pub fn a(&self) -> u8 {
        self.0[3]
    }

    /// The channels as 0..1 values, still in sRGB.
    pub fn to_normalized_gamma_f32(self) -> [f32; 4] {
        self.0.map(|channel| f32::from(channel) / 255.0)
    }
}
