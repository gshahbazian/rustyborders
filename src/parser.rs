use std::collections::HashSet;

use thiserror::Error;

use crate::settings::{
    BORDER_ORDER_ABOVE, BORDER_ORDER_BELOW, BORDER_STYLE_ROUND, BORDER_STYLE_ROUND_UNIFORM,
    BORDER_STYLE_SQUARE, ColorStyle, Gradient, GradientDirection, Settings, UpdateMask,
};
use crate::sys::geometry::WindowId;

#[derive(Debug, Error, Eq, PartialEq)]
pub enum ParseError {
    #[error("invalid argument '{0}'")]
    Argument(String),
    #[error("invalid color argument '{0}'")]
    Color(String),
    #[error("invalid width '{0}'")]
    Width(String),
    #[error("invalid apply-to window id '{0}'")]
    WindowId(String),
}

pub fn parse_settings(
    settings: &mut Settings,
    arguments: &[String],
) -> Result<UpdateMask, ParseError> {
    let mut update_mask = UpdateMask::empty();

    for argument in arguments {
        if let Some(value) = argument.strip_prefix("active_color") {
            settings.active_window = parse_color(value)?;
            update_mask |= UpdateMask::ACTIVE;
        } else if let Some(value) = argument.strip_prefix("inactive_color") {
            settings.inactive_window = parse_color(value)?;
            update_mask |= UpdateMask::INACTIVE;
        } else if let Some(value) = argument.strip_prefix("background_color") {
            settings.background = parse_color(value)?;
            settings.show_background = settings.has_background_alpha();
            update_mask |= UpdateMask::ALL;
        } else if let Some(value) = argument.strip_prefix("blacklist=") {
            settings.blacklist = parse_list(value);
            settings.blacklist_enabled = !settings.blacklist.is_empty();
            update_mask |= UpdateMask::RECREATE_ALL;
        } else if let Some(value) = argument.strip_prefix("whitelist=") {
            settings.whitelist = parse_list(value);
            settings.whitelist_enabled = !settings.whitelist.is_empty();
            update_mask |= UpdateMask::RECREATE_ALL;
        } else if let Some(value) = argument.strip_prefix("width=") {
            settings.border_width = value
                .parse::<f64>()
                .map_err(|_| ParseError::Width(value.to_owned()))?;
            update_mask |= UpdateMask::ALL;
        } else if let Some(value) = argument.strip_prefix("order=") {
            settings.border_order = if value.starts_with('a') {
                BORDER_ORDER_ABOVE
            } else {
                BORDER_ORDER_BELOW
            };
            update_mask |= UpdateMask::ALL;
        } else if let Some(value) = argument.strip_prefix("style=") {
            settings.border_style = parse_style(value)?;
            update_mask |= UpdateMask::ALL;
        } else if argument == "hidpi=on" {
            settings.hidpi = true;
            update_mask |= UpdateMask::RECREATE_ALL;
        } else if argument == "hidpi=off" {
            settings.hidpi = false;
            update_mask |= UpdateMask::RECREATE_ALL;
        } else if argument == "ax_focus=on" {
            settings.ax_focus = true;
            update_mask |= UpdateMask::SETTING;
        } else if argument == "ax_focus=off" {
            settings.ax_focus = false;
            update_mask |= UpdateMask::SETTING;
        } else if let Some(value) = argument.strip_prefix("apply-to=") {
            let wid = value
                .parse::<u32>()
                .map_err(|_| ParseError::WindowId(value.to_owned()))?;
            settings.apply_to = Some(WindowId(wid));
            update_mask |= UpdateMask::SETTING;
        } else {
            return Err(ParseError::Argument(argument.clone()));
        }
    }

    Ok(update_mask)
}

fn parse_style(value: &str) -> Result<char, ParseError> {
    match value.chars().next() {
        Some('r') => Ok(BORDER_STYLE_ROUND),
        Some('s') => Ok(BORDER_STYLE_SQUARE),
        Some('u') => Ok(BORDER_STYLE_ROUND_UNIFORM),
        _ => Err(ParseError::Argument(format!("style={value}"))),
    }
}

fn parse_list(value: &str) -> HashSet<String> {
    value
        .split(',')
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn parse_color(value: &str) -> Result<ColorStyle, ParseError> {
    let value = value
        .strip_prefix('=')
        .ok_or_else(|| ParseError::Color(value.to_owned()))?;

    if let Some(inner) = value
        .strip_prefix("glow(")
        .and_then(|v| v.strip_suffix(')'))
    {
        return parse_hex(inner).map(ColorStyle::Glow);
    }

    if let Some(inner) = value
        .strip_prefix("gradient(")
        .and_then(|v| v.strip_suffix(')'))
    {
        if let Some((left, right)) = inner.split_once(",bottom_right=") {
            let color1 = left
                .strip_prefix("top_left=")
                .ok_or_else(|| ParseError::Color(value.to_owned()))
                .and_then(parse_hex)?;
            let color2 = parse_hex(right)?;
            return Ok(ColorStyle::Gradient(Gradient {
                direction: GradientDirection::TopLeftToBottomRight,
                color1,
                color2,
            }));
        }

        if let Some((right, left)) = inner.split_once(",bottom_left=") {
            let color1 = right
                .strip_prefix("top_right=")
                .ok_or_else(|| ParseError::Color(value.to_owned()))
                .and_then(parse_hex)?;
            let color2 = parse_hex(left)?;
            return Ok(ColorStyle::Gradient(Gradient {
                direction: GradientDirection::TopRightToBottomLeft,
                color1,
                color2,
            }));
        }
    }

    parse_hex(value).map(ColorStyle::Solid)
}

fn parse_hex(value: &str) -> Result<u32, ParseError> {
    let raw = value
        .strip_prefix("0x")
        .ok_or_else(|| ParseError::Color(value.to_owned()))?;
    u32::from_str_radix(raw, 16).map_err(|_| ParseError::Color(value.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_color_styles() {
        let mut settings = Settings::default();
        let mask = parse_settings(
            &mut settings,
            &[
                "active_color=0xff00ff00".to_owned(),
                "inactive_color=glow(0x880000ff)".to_owned(),
                "background_color=gradient(top_left=0xff000000,bottom_right=0xffffffff)".to_owned(),
            ],
        )
        .unwrap();

        assert!(mask.contains(UpdateMask::ACTIVE));
        assert!(mask.contains(UpdateMask::INACTIVE));
        assert_eq!(settings.active_window, ColorStyle::Solid(0xff00ff00));
        assert_eq!(settings.inactive_window, ColorStyle::Glow(0x880000ff));
        assert!(matches!(settings.background, ColorStyle::Gradient(_)));
    }

    #[test]
    fn parses_window_filters_and_scalars() {
        let mut settings = Settings::default();
        let mask = parse_settings(
            &mut settings,
            &[
                "width=6.5".to_owned(),
                "order=a".to_owned(),
                "style=square".to_owned(),
                "hidpi=on".to_owned(),
                "blacklist=Safari,kitty".to_owned(),
                "apply-to=42".to_owned(),
            ],
        )
        .unwrap();

        assert!(mask.contains(UpdateMask::RECREATE_ALL));
        assert_eq!(settings.border_width, 6.5);
        assert_eq!(settings.border_order, BORDER_ORDER_ABOVE);
        assert_eq!(settings.border_style, BORDER_STYLE_SQUARE);
        assert!(settings.hidpi);
        assert!(settings.blacklist.contains("Safari"));
        assert_eq!(settings.apply_to, Some(WindowId(42)));
    }
}
