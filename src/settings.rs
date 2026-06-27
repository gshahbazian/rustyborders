use std::collections::HashSet;
use std::fmt;

use bitflags::bitflags;

use crate::sys::geometry::WindowId;

pub const BORDER_ORDER_BELOW: i32 = -1;
pub const BORDER_PADDING: f64 = 8.0;

bitflags! {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct UpdateMask: u32 {
        const ACTIVE = 1 << 0;
        const INACTIVE = 1 << 1;
        const ALL = Self::ACTIVE.bits() | Self::INACTIVE.bits();
        const RECREATE_ALL = 1 << 2;
        const SETTING = 1 << 3;
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ColorStyle {
    Solid(Color),
    Gradient(Gradient),
    Glow(Color),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Gradient {
    pub direction: GradientDirection,
    pub color1: Color,
    pub color2: Color,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Color {
    pub space: ColorSpace,
    pub red: f64,
    pub green: f64,
    pub blue: f64,
    pub alpha: f64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ColorSpace {
    Srgb,
    DisplayP3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GradientDirection {
    TopLeftToBottomRight,
    TopRightToBottomLeft,
}

#[derive(Clone, Debug)]
pub struct Settings {
    pub apply_to: Option<WindowId>,
    pub active_window: ColorStyle,
    pub inactive_window: ColorStyle,
    pub border_width: f64,
    pub ax_focus: bool,
    pub blacklist_enabled: bool,
    pub blacklist: HashSet<String>,
    pub whitelist_enabled: bool,
    pub whitelist: HashSet<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            apply_to: None,
            active_window: ColorStyle::Solid(Color::srgb(
                0.8823529411764706,
                0.8901960784313725,
                0.8941176470588236,
                1.0,
            )),
            inactive_window: ColorStyle::Solid(Color::srgb(0.0, 0.0, 0.0, 0.0)),
            border_width: 4.0,
            ax_focus: false,
            blacklist_enabled: false,
            blacklist: HashSet::new(),
            whitelist_enabled: false,
            whitelist: HashSet::new(),
        }
    }
}

impl Settings {
    pub fn app_allowed(&self, app_name: &str) -> bool {
        if self.whitelist_enabled && !self.whitelist.contains(app_name) {
            return false;
        }
        if self.blacklist_enabled && self.blacklist.contains(app_name) {
            return false;
        }
        true
    }

    pub fn inactive_border_visible(&self) -> bool {
        self.inactive_window.is_visible()
    }
}

impl ColorStyle {
    pub fn is_visible(&self) -> bool {
        match self {
            ColorStyle::Solid(color) | ColorStyle::Glow(color) => color.is_visible(),
            ColorStyle::Gradient(_) => true,
        }
    }
}

impl Color {
    pub const fn srgb(red: f64, green: f64, blue: f64, alpha: f64) -> Self {
        Self {
            space: ColorSpace::Srgb,
            red,
            green,
            blue,
            alpha,
        }
    }

    pub const fn display_p3(red: f64, green: f64, blue: f64, alpha: f64) -> Self {
        Self {
            space: ColorSpace::DisplayP3,
            red,
            green,
            blue,
            alpha,
        }
    }

    pub fn is_visible(&self) -> bool {
        self.alpha > 0.0
    }
}

impl ColorStyle {
    pub fn solid_color(&self) -> Option<&Color> {
        match self {
            Self::Solid(color) | Self::Glow(color) => Some(color),
            Self::Gradient(_) => None,
        }
    }

    pub fn is_glow(&self) -> bool {
        matches!(self, Self::Glow(_))
    }
}

impl fmt::Display for ColorStyle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Solid(color) => write!(formatter, "{color}"),
            Self::Glow(color) => write!(formatter, "glow({color})"),
            Self::Gradient(gradient) => match gradient.direction {
                GradientDirection::TopLeftToBottomRight => write!(
                    formatter,
                    "gradient(top_left={},bottom_right={})",
                    gradient.color1, gradient.color2
                ),
                GradientDirection::TopRightToBottomLeft => write!(
                    formatter,
                    "gradient(top_right={},bottom_left={})",
                    gradient.color1, gradient.color2
                ),
            },
        }
    }
}

impl fmt::Display for Color {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.space {
            ColorSpace::Srgb => write!(
                formatter,
                "color(srgb {:.6} {:.6} {:.6} / {:.6})",
                self.red, self.green, self.blue, self.alpha
            ),
            ColorSpace::DisplayP3 => write!(
                formatter,
                "color(display-p3 {:.6} {:.6} {:.6} / {:.6})",
                self.red, self.green, self.blue, self.alpha
            ),
        }
    }
}
