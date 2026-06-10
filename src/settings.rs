use std::collections::HashSet;
use std::ffi::CString;
use std::fmt;
use std::ptr;
use std::sync::OnceLock;

use bitflags::bitflags;

use crate::sys::geometry::WindowId;

pub const BORDER_ORDER_ABOVE: i32 = 1;
pub const BORDER_ORDER_BELOW: i32 = -1;
pub const BORDER_STYLE_ROUND: char = 'r';
pub const BORDER_STYLE_ROUND_UNIFORM: char = 'u';
pub const BORDER_STYLE_SQUARE: char = 's';
pub const BORDER_PADDING: f64 = 8.0;
pub const BORDER_TSMN: f64 = 3.27;
pub const BORDER_TSMW_LEGACY: f64 = 8.0;
pub const BORDER_TSMW_MACOS_26: f64 = 52.0;

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ColorStyle {
    Solid(u32),
    Gradient(Gradient),
    Glow(u32),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Gradient {
    pub direction: GradientDirection,
    pub color1: u32,
    pub color2: u32,
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
    pub background: ColorStyle,
    pub border_width: f64,
    pub border_style: char,
    pub hidpi: bool,
    pub show_background: bool,
    pub border_order: i32,
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
            active_window: ColorStyle::Solid(0xffe1e3e4),
            inactive_window: ColorStyle::Solid(0x00000000),
            background: ColorStyle::Solid(0x00000000),
            border_width: 4.0,
            border_style: BORDER_STYLE_ROUND,
            hidpi: false,
            show_background: false,
            border_order: BORDER_ORDER_BELOW,
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

    pub fn has_background_alpha(&self) -> bool {
        self.background.is_visible()
    }

    pub fn inactive_border_visible(&self) -> bool {
        self.inactive_window.is_visible() || self.show_background
    }
}

impl ColorStyle {
    pub fn is_visible(&self) -> bool {
        match self {
            ColorStyle::Solid(color) | ColorStyle::Glow(color) => color & 0xff00_0000 != 0,
            ColorStyle::Gradient(_) => true,
        }
    }
}

pub fn border_tsmw() -> f64 {
    static VALUE: OnceLock<f64> = OnceLock::new();
    *VALUE.get_or_init(|| {
        if macos_major_version().is_some_and(|major| major >= 26) {
            BORDER_TSMW_MACOS_26
        } else {
            BORDER_TSMW_LEGACY
        }
    })
}

fn macos_major_version() -> Option<u32> {
    let name = CString::new("kern.osproductversion").ok()?;
    let mut size = 0_usize;
    let size_result = unsafe {
        libc::sysctlbyname(
            name.as_ptr(),
            ptr::null_mut(),
            &mut size,
            ptr::null_mut(),
            0,
        )
    };
    if size_result != 0 || size == 0 {
        return None;
    }

    let mut buffer = vec![0_u8; size];
    let value_result = unsafe {
        libc::sysctlbyname(
            name.as_ptr(),
            buffer.as_mut_ptr().cast(),
            &mut size,
            ptr::null_mut(),
            0,
        )
    };
    if value_result != 0 {
        return None;
    }

    let nul = buffer.iter().position(|byte| *byte == 0).unwrap_or(size);
    let version = std::str::from_utf8(&buffer[..nul]).ok()?;
    version.split('.').next()?.parse().ok()
}

impl ColorStyle {
    pub fn solid_color(&self) -> Option<u32> {
        match *self {
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
            Self::Solid(color) => write!(formatter, "0x{color:08x}"),
            Self::Glow(color) => write!(formatter, "glow(0x{color:08x})"),
            Self::Gradient(gradient) => match gradient.direction {
                GradientDirection::TopLeftToBottomRight => write!(
                    formatter,
                    "gradient(top_left=0x{:08x},bottom_right=0x{:08x})",
                    gradient.color1, gradient.color2
                ),
                GradientDirection::TopRightToBottomLeft => write!(
                    formatter,
                    "gradient(top_right=0x{:08x},bottom_left=0x{:08x})",
                    gradient.color1, gradient.color2
                ),
            },
        }
    }
}
