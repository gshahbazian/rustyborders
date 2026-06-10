use std::fmt;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CGPoint {
    pub x: f64,
    pub y: f64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CGSize {
    pub width: f64,
    pub height: f64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CGRect {
    pub origin: CGPoint,
    pub size: CGSize,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CGAffineTransform {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub tx: f64,
    pub ty: f64,
}

impl Default for CGAffineTransform {
    fn default() -> Self {
        Self::identity()
    }
}

impl CGAffineTransform {
    pub const fn identity() -> Self {
        Self {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            tx: 0.0,
            ty: 0.0,
        }
    }

    pub const fn scale(width: f64, height: f64) -> Self {
        Self {
            a: width,
            b: 0.0,
            c: 0.0,
            d: height,
            tx: 0.0,
            ty: 0.0,
        }
    }
}

impl CGPoint {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    pub fn apply(self, transform: CGAffineTransform) -> Self {
        Self {
            x: self.x * transform.a + self.y * transform.c + transform.tx,
            y: self.x * transform.b + self.y * transform.d + transform.ty,
        }
    }
}

impl CGSize {
    pub const ZERO: Self = Self {
        width: 0.0,
        height: 0.0,
    };
}

impl CGRect {
    pub const ZERO: Self = Self {
        origin: CGPoint::ZERO,
        size: CGSize::ZERO,
    };

    pub fn inset(self, dx: f64, dy: f64) -> Self {
        Self {
            origin: CGPoint {
                x: self.origin.x + dx,
                y: self.origin.y + dy,
            },
            size: CGSize {
                width: self.size.width - 2.0 * dx,
                height: self.size.height - 2.0 * dy,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct WindowId(pub u32);

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SpaceId(pub u64);

impl fmt::Display for WindowId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl fmt::Display for SpaceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_graphics_geometry_layout_matches_64_bit_abi() {
        assert_eq!(std::mem::size_of::<CGPoint>(), 16);
        assert_eq!(std::mem::size_of::<CGSize>(), 16);
        assert_eq!(std::mem::size_of::<CGRect>(), 32);
        assert_eq!(std::mem::size_of::<CGAffineTransform>(), 48);
    }
}
