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

    pub fn intersection(self, other: Self) -> Option<Self> {
        let min_x = self.origin.x.max(other.origin.x);
        let min_y = self.origin.y.max(other.origin.y);
        let max_x = (self.origin.x + self.size.width).min(other.origin.x + other.size.width);
        let max_y = (self.origin.y + self.size.height).min(other.origin.y + other.size.height);

        (max_x > min_x && max_y > min_y).then_some(Self {
            origin: CGPoint { x: min_x, y: min_y },
            size: CGSize {
                width: max_x - min_x,
                height: max_y - min_y,
            },
        })
    }

    pub fn area(self) -> f64 {
        if self.size.width <= 0.0 || self.size.height <= 0.0 {
            return 0.0;
        }
        self.size.width * self.size.height
    }
}

pub fn is_more_than_half_visible(window: CGRect, displays: &[CGRect]) -> bool {
    let window_area = window.area();
    if window_area == 0.0 {
        return false;
    }

    let intersections = displays
        .iter()
        .filter_map(|display| window.intersection(*display))
        .collect::<Vec<_>>();
    let mut x_coordinates = intersections
        .iter()
        .flat_map(|rect| [rect.origin.x, rect.origin.x + rect.size.width])
        .collect::<Vec<_>>();
    x_coordinates.sort_by(f64::total_cmp);
    x_coordinates.dedup();

    let mut visible_area = 0.0;
    for x_pair in x_coordinates.windows(2) {
        let [min_x, max_x] = x_pair else {
            continue;
        };
        if max_x <= min_x {
            continue;
        }

        let mut y_intervals = intersections
            .iter()
            .filter(|rect| rect.origin.x < *max_x && rect.origin.x + rect.size.width > *min_x)
            .map(|rect| (rect.origin.y, rect.origin.y + rect.size.height))
            .collect::<Vec<_>>();
        y_intervals.sort_by(|left, right| left.0.total_cmp(&right.0));

        let mut covered_height = 0.0;
        let mut current_interval: Option<(f64, f64)> = None;
        for (min_y, max_y) in y_intervals {
            match current_interval {
                Some((current_min, current_max)) if min_y <= current_max => {
                    current_interval = Some((current_min, current_max.max(max_y)));
                }
                Some((current_min, current_max)) => {
                    covered_height += current_max - current_min;
                    current_interval = Some((min_y, max_y));
                }
                None => current_interval = Some((min_y, max_y)),
            }
        }
        if let Some((min_y, max_y)) = current_interval {
            covered_height += max_y - min_y;
        }

        visible_area += (max_x - min_x) * covered_height;
    }

    visible_area * 2.0 > window_area
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

    fn rect(x: f64, y: f64, width: f64, height: f64) -> CGRect {
        CGRect {
            origin: CGPoint { x, y },
            size: CGSize { width, height },
        }
    }

    #[test]
    fn core_graphics_geometry_layout_matches_64_bit_abi() {
        assert_eq!(std::mem::size_of::<CGPoint>(), 16);
        assert_eq!(std::mem::size_of::<CGSize>(), 16);
        assert_eq!(std::mem::size_of::<CGRect>(), 32);
        assert_eq!(std::mem::size_of::<CGAffineTransform>(), 48);
    }

    #[test]
    fn visibility_requires_strictly_more_than_half_the_window() {
        let display = rect(0.0, 0.0, 100.0, 100.0);

        assert!(is_more_than_half_visible(
            rect(10.0, 10.0, 50.0, 50.0),
            &[display]
        ));
        assert!(!is_more_than_half_visible(
            rect(95.0, 95.0, 100.0, 100.0),
            &[display]
        ));
        assert!(!is_more_than_half_visible(
            rect(50.0, 0.0, 100.0, 100.0),
            &[display]
        ));
        assert!(is_more_than_half_visible(
            rect(49.0, 0.0, 100.0, 100.0),
            &[display]
        ));
    }

    #[test]
    fn visibility_uses_the_union_of_all_displays() {
        let displays = [rect(0.0, 0.0, 100.0, 100.0), rect(100.0, 0.0, 100.0, 100.0)];
        assert!(is_more_than_half_visible(
            rect(50.0, 0.0, 100.0, 100.0),
            &displays
        ));
    }

    #[test]
    fn overlapping_display_bounds_are_not_counted_twice() {
        let display = rect(0.0, 0.0, 40.0, 100.0);
        assert!(!is_more_than_half_visible(
            rect(0.0, 0.0, 100.0, 100.0),
            &[display, display]
        ));
    }
}
