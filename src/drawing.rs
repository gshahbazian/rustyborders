use std::ptr;

use crate::settings::{ColorStyle, GradientDirection};
use crate::sys::cf::{CFArrayRef, CFRelease, OwnedCf};
use crate::sys::geometry::{CGAffineTransform, CGPoint, CGRect, CGSize};
use crate::sys::os::{
    CGColorCreateSRGB, CGColorRef, CGContextAddPath, CGContextClip, CGContextDrawLinearGradient,
    CGContextEOClip, CGContextFillPath, CGContextRef, CGContextReplacePathWithStrokedPath,
    CGContextSetRGBFillColor, CGContextSetRGBStrokeColor, CGContextSetShadowWithColor,
    CGContextStrokePath, CGGradientCreateWithColors, CGGradientRef, CGMutablePathRef,
    CGPathAddPath, CGPathAddRect, CGPathAddRoundedRect, CGPathCreateMutable, CGPathCreateWithRect,
    CGPathCreateWithRoundedRect, CGPathRef,
};
use crate::sys::os::{CGColorRelease, CGGradientRelease};

pub fn colors_from_hex(hex: u32) -> (f64, f64, f64, f64) {
    let a = f64::from((hex >> 24) & 0xff) / 255.0;
    let r = f64::from((hex >> 16) & 0xff) / 255.0;
    let g = f64::from((hex >> 8) & 0xff) / 255.0;
    let b = f64::from(hex & 0xff) / 255.0;
    (a, r, g, b)
}

pub unsafe fn set_fill(context: CGContextRef, color: u32) {
    let (a, r, g, b) = colors_from_hex(color);
    unsafe {
        CGContextSetRGBFillColor(context, r, g, b, a);
    }
}

pub unsafe fn set_stroke(context: CGContextRef, color: u32) {
    let (a, r, g, b) = colors_from_hex(color);
    unsafe {
        CGContextSetRGBStrokeColor(context, r, g, b, a);
    }
}

pub unsafe fn set_stroke_and_fill(context: CGContextRef, color: u32, glow: bool) {
    let (a, r, g, b) = colors_from_hex(color);
    unsafe {
        CGContextSetRGBFillColor(context, r, g, b, a);
        CGContextSetRGBStrokeColor(context, r, g, b, a);
    }

    if glow {
        let color_ref = unsafe { CGColorCreateSRGB(r, g, b, 1.0) };
        if !color_ref.is_null() {
            unsafe {
                CGContextSetShadowWithColor(context, CGSize::ZERO, 10.0, color_ref);
                CGColorRelease(color_ref);
            }
        }
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

pub unsafe fn draw_square_with_inset(context: CGContextRef, rect: CGRect, inset: f64) {
    unsafe {
        add_rect_with_inset(context, rect, inset);
        CGContextFillPath(context);
    }
}

pub unsafe fn draw_square_gradient_with_inset(
    context: CGContextRef,
    gradient: CGGradientRef,
    direction: [CGPoint; 2],
    rect: CGRect,
    inset: f64,
) {
    unsafe {
        add_rect_with_inset(context, rect, inset);
        CGContextClip(context);
        CGContextDrawLinearGradient(context, gradient, direction[0], direction[1], 0);
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

pub unsafe fn draw_filled_path(context: CGContextRef, path: CGPathRef, color: u32) {
    unsafe {
        set_fill(context, color);
        set_stroke(context, 0);
        CGContextAddPath(context, path);
        CGContextFillPath(context);
    }
}

pub unsafe fn create_gradient(
    style: &ColorStyle,
    transform: CGAffineTransform,
) -> Option<(CGGradientRef, [CGPoint; 2])> {
    let ColorStyle::Gradient(gradient) = style else {
        return None;
    };

    let (a1, r1, g1, b1) = colors_from_hex(gradient.color1);
    let (a2, r2, g2, b2) = colors_from_hex(gradient.color2);
    let colors: [CGColorRef; 2] = unsafe {
        [
            CGColorCreateSRGB(r1, g1, b1, a1),
            CGColorCreateSRGB(r2, g2, b2, a2),
        ]
    };
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

    let gradient_ref =
        unsafe { CGGradientCreateWithColors(ptr::null(), cf_colors.as_raw(), ptr::null()) };
    unsafe {
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

unsafe fn add_rect_with_inset(context: CGContextRef, rect: CGRect, inset: f64) {
    let square_rect = rect.inset(inset, inset);
    let square_path = unsafe { CGPathCreateWithRect(square_rect, ptr::null()) };
    if !square_path.is_null() {
        unsafe {
            CGContextAddPath(context, square_path);
            CFRelease(square_path.cast_const());
        }
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

pub unsafe fn add_rect(path: CGMutablePathRef, rect: CGRect) {
    unsafe {
        CGPathAddRect(path, ptr::null(), rect);
    }
}

pub unsafe fn add_rounded_rect_to_path(path: CGMutablePathRef, rect: CGRect, radius: f64) {
    unsafe {
        CGPathAddRoundedRect(path, ptr::null(), rect, radius, radius);
    }
}
