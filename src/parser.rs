use std::collections::HashSet;

use thiserror::Error;

use crate::settings::{Color, ColorStyle, Gradient, GradientDirection, Settings, UpdateMask};
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
        return parse_color_value(inner).map(ColorStyle::Glow);
    }

    if let Some(inner) = value
        .strip_prefix("gradient(")
        .and_then(|v| v.strip_suffix(')'))
    {
        if let Some((left, right)) = inner.split_once(",bottom_right=") {
            let color1 = left
                .strip_prefix("top_left=")
                .ok_or_else(|| ParseError::Color(value.to_owned()))
                .and_then(parse_color_value)?;
            let color2 = parse_color_value(right)?;
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
                .and_then(parse_color_value)?;
            let color2 = parse_color_value(left)?;
            return Ok(ColorStyle::Gradient(Gradient {
                direction: GradientDirection::TopRightToBottomLeft,
                color1,
                color2,
            }));
        }
    }

    parse_color_value(value).map(ColorStyle::Solid)
}

fn parse_color_value(value: &str) -> Result<Color, ParseError> {
    let value = value.trim();
    if let Some(inner) = value
        .strip_prefix("color(")
        .and_then(|v| v.strip_suffix(')'))
    {
        return parse_css_color(inner, value);
    }
    if let Some(inner) = value
        .strip_prefix("oklch(")
        .and_then(|v| v.strip_suffix(')'))
    {
        return parse_oklch(inner, value);
    }
    parse_hex(value)
}

fn parse_hex(value: &str) -> Result<Color, ParseError> {
    if let Some(raw) = value.strip_prefix("0x") {
        let color =
            u32::from_str_radix(raw, 16).map_err(|_| ParseError::Color(value.to_owned()))?;
        return Ok(Color::srgb(
            f64::from((color >> 16) & 0xff) / 255.0,
            f64::from((color >> 8) & 0xff) / 255.0,
            f64::from(color & 0xff) / 255.0,
            f64::from((color >> 24) & 0xff) / 255.0,
        ));
    }

    if let Some(raw) = value.strip_prefix('#') {
        return match raw.len() {
            6 => {
                let color = u32::from_str_radix(raw, 16)
                    .map_err(|_| ParseError::Color(value.to_owned()))?;
                Ok(Color::srgb(
                    f64::from((color >> 16) & 0xff) / 255.0,
                    f64::from((color >> 8) & 0xff) / 255.0,
                    f64::from(color & 0xff) / 255.0,
                    1.0,
                ))
            }
            8 => {
                let color = u32::from_str_radix(raw, 16)
                    .map_err(|_| ParseError::Color(value.to_owned()))?;
                Ok(Color::srgb(
                    f64::from((color >> 16) & 0xff) / 255.0,
                    f64::from((color >> 8) & 0xff) / 255.0,
                    f64::from(color & 0xff) / 255.0,
                    f64::from((color >> 24) & 0xff) / 255.0,
                ))
            }
            _ => Err(ParseError::Color(value.to_owned())),
        };
    }

    Err(ParseError::Color(value.to_owned()))
}

fn parse_css_color(inner: &str, original: &str) -> Result<Color, ParseError> {
    let (values, alpha) = split_color_values(inner, original)?;
    if values.len() != 4 {
        return Err(ParseError::Color(original.to_owned()));
    }

    let color_space = values[0];
    let red = parse_unit_component(values[1], original)?;
    let green = parse_unit_component(values[2], original)?;
    let blue = parse_unit_component(values[3], original)?;
    let alpha = alpha
        .map(|value| parse_alpha(value, original))
        .transpose()?
        .unwrap_or(1.0);

    match color_space {
        "display-p3" => Ok(Color::display_p3(red, green, blue, alpha)),
        "srgb" => Ok(Color::srgb(red, green, blue, alpha)),
        _ => Err(ParseError::Color(original.to_owned())),
    }
}

fn parse_oklch(inner: &str, original: &str) -> Result<Color, ParseError> {
    let (values, alpha) = split_color_values(inner, original)?;
    if values.len() != 3 {
        return Err(ParseError::Color(original.to_owned()));
    }

    let lightness = parse_lightness(values[0], original)?;
    let chroma = parse_nonnegative_component(values[1], original)?;
    let hue = parse_hue(values[2], original)?;
    let alpha = alpha
        .map(|value| parse_alpha(value, original))
        .transpose()?
        .unwrap_or(1.0);
    let (red, green, blue) = oklch_to_display_p3(lightness, chroma, hue, original)?;
    Ok(Color::display_p3(red, green, blue, alpha))
}

fn split_color_values<'a>(
    inner: &'a str,
    original: &str,
) -> Result<(Vec<&'a str>, Option<&'a str>), ParseError> {
    let tokens = inner.split_whitespace().collect::<Vec<_>>();
    let Some(slash) = tokens.iter().position(|token| *token == "/") else {
        return Ok((tokens, None));
    };
    if slash == 0 || slash + 2 != tokens.len() {
        return Err(ParseError::Color(original.to_owned()));
    }
    Ok((tokens[..slash].to_vec(), Some(tokens[slash + 1])))
}

fn parse_unit_component(value: &str, original: &str) -> Result<f64, ParseError> {
    let component = parse_number_or_percent(value, original)?;
    if (0.0..=1.0).contains(&component) {
        Ok(component)
    } else {
        Err(ParseError::Color(original.to_owned()))
    }
}

fn parse_nonnegative_component(value: &str, original: &str) -> Result<f64, ParseError> {
    let component = parse_number_or_percent(value, original)?;
    if component >= 0.0 {
        Ok(component)
    } else {
        Err(ParseError::Color(original.to_owned()))
    }
}

fn parse_lightness(value: &str, original: &str) -> Result<f64, ParseError> {
    parse_unit_component(value, original)
}

fn parse_alpha(value: &str, original: &str) -> Result<f64, ParseError> {
    parse_unit_component(value, original)
}

fn parse_number_or_percent(value: &str, original: &str) -> Result<f64, ParseError> {
    if let Some(percent) = value.strip_suffix('%') {
        let value = percent
            .parse::<f64>()
            .map_err(|_| ParseError::Color(original.to_owned()))?
            / 100.0;
        return Ok(value);
    }
    value
        .parse::<f64>()
        .map_err(|_| ParseError::Color(original.to_owned()))
}

fn parse_hue(value: &str, original: &str) -> Result<f64, ParseError> {
    let raw = value.strip_suffix("deg").unwrap_or(value);
    raw.parse::<f64>()
        .map_err(|_| ParseError::Color(original.to_owned()))
}

fn oklch_to_display_p3(
    lightness: f64,
    chroma: f64,
    hue: f64,
    original: &str,
) -> Result<(f64, f64, f64), ParseError> {
    let hue = hue.to_radians();
    let a = chroma * hue.cos();
    let b = chroma * hue.sin();

    let l_prime = lightness + 0.396_337_777_4 * a + 0.215_803_757_3 * b;
    let m_prime = lightness - 0.105_561_345_8 * a - 0.063_854_172_8 * b;
    let s_prime = lightness - 0.089_484_177_5 * a - 1.291_485_548_0 * b;

    let l = l_prime.powi(3);
    let m = m_prime.powi(3);
    let s = s_prime.powi(3);

    let x = 1.227_013_851_1 * l - 0.557_799_980_7 * m + 0.281_256_149_0 * s;
    let y = -0.040_580_178_4 * l + 1.112_256_869_6 * m - 0.071_676_678_7 * s;
    let z = -0.076_381_284_5 * l - 0.421_481_978_4 * m + 1.586_163_220_4 * s;

    let red = 2.493_496_911_9 * x - 0.931_383_617_9 * y - 0.402_710_784_5 * z;
    let green = -0.829_488_969_6 * x + 1.762_664_060_3 * y + 0.023_624_685_8 * z;
    let blue = 0.035_845_830_2 * x - 0.076_172_389_3 * y + 0.956_884_524_0 * z;

    let red = encode_p3_component(red, original)?;
    let green = encode_p3_component(green, original)?;
    let blue = encode_p3_component(blue, original)?;
    Ok((red, green, blue))
}

fn encode_p3_component(component: f64, original: &str) -> Result<f64, ParseError> {
    const EPSILON: f64 = 0.000_001;
    if !(-EPSILON..=1.0 + EPSILON).contains(&component) {
        return Err(ParseError::Color(original.to_owned()));
    }
    let component = component.clamp(0.0, 1.0);
    if component <= 0.003_130_8 {
        Ok(12.92 * component)
    } else {
        Ok(1.055 * component.powf(1.0 / 2.4) - 0.055)
    }
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
            ],
        )
        .unwrap();

        assert!(mask.contains(UpdateMask::ACTIVE));
        assert!(mask.contains(UpdateMask::INACTIVE));
        assert_eq!(
            settings.active_window,
            ColorStyle::Solid(Color::srgb(0.0, 1.0, 0.0, 1.0))
        );
        assert_eq!(
            settings.inactive_window,
            ColorStyle::Glow(Color::srgb(0.0, 0.0, 1.0, 136.0 / 255.0))
        );
    }

    #[test]
    fn parses_display_p3_and_oklch_colors() {
        let mut settings = Settings::default();
        parse_settings(
            &mut settings,
            &[
                "active_color=color(display-p3 0.059 0.978 0.355 / 1)".to_owned(),
                "inactive_color=oklch(84% 0.32 150 / 0.5)".to_owned(),
            ],
        )
        .unwrap();

        assert_eq!(
            settings.active_window,
            ColorStyle::Solid(Color::display_p3(0.059, 0.978, 0.355, 1.0))
        );

        let ColorStyle::Solid(color) = settings.inactive_window else {
            panic!("expected solid OKLCH color");
        };
        assert_eq!(color.space, crate::settings::ColorSpace::DisplayP3);
        assert_close(color.red, 0.059_158_901_7);
        assert_close(color.green, 0.977_722_558_2);
        assert_close(color.blue, 0.354_968_220_6);
        assert_close(color.alpha, 0.5);
    }

    #[test]
    fn parses_window_filters_and_scalars() {
        let mut settings = Settings::default();
        let mask = parse_settings(
            &mut settings,
            &[
                "width=6.5".to_owned(),
                "blacklist=Safari,kitty".to_owned(),
                "apply-to=42".to_owned(),
            ],
        )
        .unwrap();

        assert!(mask.contains(UpdateMask::RECREATE_ALL));
        assert_eq!(settings.border_width, 6.5);
        assert!(settings.blacklist.contains("Safari"));
        assert_eq!(settings.apply_to, Some(WindowId(42)));
    }

    fn assert_close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 0.000_000_1,
            "{actual} != {expected}"
        );
    }
}
