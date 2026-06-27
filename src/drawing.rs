use std::ptr;

use crate::settings::{Color, ColorSpace, ColorStyle, GradientDirection};
use crate::sys::cf::{CFArrayRef, CFRelease, OwnedCf};
use crate::sys::geometry::{CGAffineTransform, CGPoint, CGRect, CGSize};
use crate::sys::os::{
    CGColorCreate, CGColorRef, CGColorSpaceCreateWithName, CGColorSpaceRef, CGContextAddPath,
    CGContextClip, CGContextDrawLinearGradient, CGContextEOClip, CGContextFillPath, CGContextRef,
    CGContextReplacePathWithStrokedPath, CGContextSetFillColorWithColor,
    CGContextSetShadowWithColor, CGContextSetStrokeColorWithColor, CGContextStrokePath,
    CGGradientCreateWithColors, CGGradientRef, CGMutablePathRef, CGPathAddPath, CGPathAddRect,
    CGPathAddRoundedRect, CGPathCreateMutable, CGPathCreateWithRoundedRect, CGPathRef,
    kCGColorSpaceDisplayP3, kCGColorSpaceSRGB,
};
use crate::sys::os::{CGColorRelease, CGColorSpaceRelease, CGGradientRelease};

pub unsafe fn set_stroke_and_fill(context: CGContextRef, color: &Color, glow: bool) {
    let Some(color_ref) = (unsafe { create_cg_color(color) }) else {
        return;
    };
    unsafe {
        CGContextSetFillColorWithColor(context, color_ref);
        CGContextSetStrokeColorWithColor(context, color_ref);
    }

    if glow {
        let shadow_color = Color {
            alpha: 1.0,
            ..color.clone()
        };
        if let Some(shadow_color_ref) = unsafe { create_cg_color(&shadow_color) } {
            unsafe {
                CGContextSetShadowWithColor(context, CGSize::ZERO, 10.0, shadow_color_ref);
                CGColorRelease(shadow_color_ref);
            }
        }
    }
    unsafe {
        CGColorRelease(color_ref);
    }
}

pub unsafe fn clip_between_rect_and_path(context: CGContextRef, frame: CGRect, path: CGPathRef) {
    let clip_path = unsafe { CGPathCreateMutable() };
    if clip_path.is_null() {
        return;
    }
    unsafe {
        CGPathAddRect(clip_path, ptr::null(), frame);
        CGPathAddPath(clip_path, ptr::null(), path);
        CGContextAddPath(context, clip_path);
        CGContextEOClip(context);
        CFRelease(clip_path.cast_const());
    }
}

pub unsafe fn draw_rounded_rect_with_inset(
    context: CGContextRef,
    rect: CGRect,
    border_radius: f64,
    fill: bool,
) {
    unsafe {
        add_rounded_rect(context, rect, border_radius);
        if fill {
            CGContextFillPath(context);
        } else {
            CGContextStrokePath(context);
        }
    }
}

pub unsafe fn draw_rounded_gradient_with_inset(
    context: CGContextRef,
    gradient: CGGradientRef,
    direction: [CGPoint; 2],
    rect: CGRect,
    border_radius: f64,
) {
    unsafe {
        add_rounded_rect(context, rect, border_radius);
        CGContextReplacePathWithStrokedPath(context);
        CGContextClip(context);
        CGContextDrawLinearGradient(context, gradient, direction[0], direction[1], 0);
    }
}

pub unsafe fn create_gradient(
    style: &ColorStyle,
    transform: CGAffineTransform,
) -> Option<(CGGradientRef, [CGPoint; 2])> {
    let ColorStyle::Gradient(gradient) = style else {
        return None;
    };

    let color1 = unsafe { create_cg_color(&gradient.color1)? };
    let Some(color2) = (unsafe { create_cg_color(&gradient.color2) }) else {
        unsafe {
            CGColorRelease(color1);
        }
        return None;
    };
    let colors = [color1, color2];
    if colors.iter().any(|color| color.is_null()) {
        for color in colors {
            if !color.is_null() {
                unsafe {
                    CGColorRelease(color);
                }
            }
        }
        return None;
    }

    let cf_colors = unsafe {
        OwnedCf::from_create_rule(crate::sys::cf::CFArrayCreate(
            ptr::null(),
            colors.as_ptr().cast::<*const std::ffi::c_void>(),
            2,
            &raw const crate::sys::cf::kCFTypeArrayCallBacks,
        ))
    };

    let cf_colors: OwnedCf<CFArrayRef> = match cf_colors {
        Some(value) => value,
        None => {
            unsafe {
                CGColorRelease(colors[0]);
                CGColorRelease(colors[1]);
            }
            return None;
        }
    };

    let Some(color_space) = (unsafe { create_cg_color_space(gradient_space(style)) }) else {
        unsafe {
            CGColorRelease(colors[0]);
            CGColorRelease(colors[1]);
        }
        return None;
    };
    let gradient_ref =
        unsafe { CGGradientCreateWithColors(color_space, cf_colors.as_raw(), ptr::null()) };
    unsafe {
        CGColorSpaceRelease(color_space);
        CGColorRelease(colors[0]);
        CGColorRelease(colors[1]);
    }
    if gradient_ref.is_null() {
        return None;
    }

    let mut direction = match gradient.direction {
        GradientDirection::TopRightToBottomLeft => [CGPoint { x: 1.0, y: 1.0 }, CGPoint::ZERO],
        GradientDirection::TopLeftToBottomRight => {
            [CGPoint { x: 0.0, y: 1.0 }, CGPoint { x: 1.0, y: 0.0 }]
        }
    };
    direction[0] = direction[0].apply(transform);
    direction[1] = direction[1].apply(transform);

    Some((gradient_ref, direction))
}

pub unsafe fn release_gradient(gradient: CGGradientRef) {
    if !gradient.is_null() {
        unsafe {
            CGGradientRelease(gradient);
        }
    }
}

unsafe fn create_cg_color(color: &Color) -> Option<CGColorRef> {
    let color_space = unsafe { create_cg_color_space(color.space)? };
    let components = [color.red, color.green, color.blue, color.alpha];
    let color_ref = unsafe { CGColorCreate(color_space, components.as_ptr()) };
    unsafe {
        CGColorSpaceRelease(color_space);
    }
    (!color_ref.is_null()).then_some(color_ref)
}

unsafe fn create_cg_color_space(space: ColorSpace) -> Option<CGColorSpaceRef> {
    let name = match space {
        ColorSpace::Srgb => unsafe { kCGColorSpaceSRGB },
        ColorSpace::DisplayP3 => unsafe { kCGColorSpaceDisplayP3 },
    };
    let color_space = unsafe { CGColorSpaceCreateWithName(name) };
    (!color_space.is_null()).then_some(color_space)
}

fn gradient_space(style: &ColorStyle) -> ColorSpace {
    let ColorStyle::Gradient(gradient) = style else {
        return ColorSpace::Srgb;
    };
    if gradient.color1.space == ColorSpace::DisplayP3
        || gradient.color2.space == ColorSpace::DisplayP3
    {
        ColorSpace::DisplayP3
    } else {
        ColorSpace::Srgb
    }
}

unsafe fn add_rounded_rect(context: CGContextRef, rect: CGRect, border_radius: f64) {
    let stroke_path =
        unsafe { CGPathCreateWithRoundedRect(rect, border_radius, border_radius, ptr::null()) };
    if !stroke_path.is_null() {
        unsafe {
            CGContextAddPath(context, stroke_path);
            CFRelease(stroke_path.cast_const());
        }
    }
}

pub unsafe fn new_mutable_path() -> Option<CGMutablePathRef> {
    let path = unsafe { CGPathCreateMutable() };
    (!path.is_null()).then_some(path)
}

pub unsafe fn add_rounded_rect_to_path(path: CGMutablePathRef, rect: CGRect, radius: f64) {
    unsafe {
        CGPathAddRoundedRect(path, ptr::null(), rect, radius, radius);
    }
}
