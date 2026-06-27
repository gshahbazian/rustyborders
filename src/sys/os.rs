#![allow(dead_code, non_snake_case, non_upper_case_globals)]

use std::os::raw::{c_int, c_uint, c_void};

use crate::sys::cf::{Boolean, CFArrayRef, CFDictionaryRef, CFStringRef, CFTypeRef, CFUUIDRef};
use crate::sys::geometry::{CGPoint, CGRect, CGSize};
use crate::sys::mach::MachPort;

pub type CGError = c_int;
pub type CGDirectDisplayID = c_uint;
pub type CGContextRef = *mut c_void;
pub type CGPathRef = *mut c_void;
pub type CGMutablePathRef = *mut c_void;
pub type CGColorRef = *mut c_void;
pub type CGColorSpaceRef = *mut c_void;
pub type CGGradientRef = *mut c_void;
pub type CGEventRef = *mut c_void;
pub type AXUIElementRef = *mut c_void;

pub const K_CG_ERROR_SUCCESS: CGError = 0;
pub const K_CG_BACKING_STORE_BUFFERED: c_int = 2;
pub const K_CG_INTERPOLATION_NONE: c_int = 1;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct ProcessSerialNumber {
    pub high_long_of_psn: u32,
    pub low_long_of_psn: u32,
}

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    pub fn CGGetActiveDisplayList(
        max_displays: u32,
        active_displays: *mut CGDirectDisplayID,
        display_count: *mut u32,
    ) -> CGError;
    pub fn CGMainDisplayID() -> CGDirectDisplayID;
    pub fn CGDisplayBounds(display: CGDirectDisplayID) -> CGRect;
    pub fn CGDisplayCreateUUIDFromDisplayID(display: CGDirectDisplayID) -> CFUUIDRef;

    pub fn AXIsProcessTrusted() -> Boolean;
    pub fn AXIsProcessTrustedWithOptions(options: CFDictionaryRef) -> Boolean;
    pub fn AXUIElementCreateApplication(pid: libc::pid_t) -> AXUIElementRef;
    pub fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: *mut CFTypeRef,
    ) -> CGError;
    pub fn _AXUIElementGetWindow(window: CFTypeRef, wid: *mut u32);
}

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    pub static kCGColorSpaceDisplayP3: CFStringRef;
    pub static kCGColorSpaceSRGB: CFStringRef;

    pub fn CGContextSaveGState(context: CGContextRef);
    pub fn CGContextRestoreGState(context: CGContextRef);
    pub fn CGContextSetFillColorWithColor(context: CGContextRef, color: CGColorRef);
    pub fn CGContextSetStrokeColorWithColor(context: CGContextRef, color: CGColorRef);
    pub fn CGContextSetShadowWithColor(
        context: CGContextRef,
        offset: CGSize,
        blur: f64,
        color: CGColorRef,
    );
    pub fn CGContextSetLineWidth(context: CGContextRef, width: f64);
    pub fn CGContextClearRect(context: CGContextRef, rect: CGRect);
    pub fn CGContextAddPath(context: CGContextRef, path: CGPathRef);
    pub fn CGContextEOClip(context: CGContextRef);
    pub fn CGContextFillPath(context: CGContextRef);
    pub fn CGContextStrokePath(context: CGContextRef);
    pub fn CGContextReplacePathWithStrokedPath(context: CGContextRef);
    pub fn CGContextClip(context: CGContextRef);
    pub fn CGContextDrawLinearGradient(
        context: CGContextRef,
        gradient: CGGradientRef,
        start_point: CGPoint,
        end_point: CGPoint,
        options: u32,
    );
    pub fn CGContextFlush(context: CGContextRef);
    pub fn CGContextSetInterpolationQuality(context: CGContextRef, quality: c_int);

    pub fn CGPathCreateMutable() -> CGMutablePathRef;
    pub fn CGPathAddRect(path: CGMutablePathRef, transform: *const c_void, rect: CGRect);
    pub fn CGPathAddPath(path1: CGMutablePathRef, transform: *const c_void, path2: CGPathRef);
    pub fn CGPathAddRoundedRect(
        path: CGMutablePathRef,
        transform: *const c_void,
        rect: CGRect,
        corner_width: f64,
        corner_height: f64,
    );
    pub fn CGPathCreateWithRoundedRect(
        rect: CGRect,
        corner_width: f64,
        corner_height: f64,
        transform: *const c_void,
    ) -> CGPathRef;

    pub fn CGColorCreate(color_space: CGColorSpaceRef, components: *const f64) -> CGColorRef;
    pub fn CGColorRelease(color: CGColorRef);
    pub fn CGColorSpaceCreateWithName(name: CFStringRef) -> CGColorSpaceRef;
    pub fn CGColorSpaceRelease(color_space: CGColorSpaceRef);
    pub fn CGGradientCreateWithColors(
        color_space: CGColorSpaceRef,
        colors: CFArrayRef,
        locations: *const f64,
    ) -> CGGradientRef;
    pub fn CGGradientRelease(gradient: CGGradientRef);
}

#[link(name = "System", kind = "dylib")]
unsafe extern "C" {
    pub fn proc_name(pid: libc::pid_t, buffer: *mut c_void, buffersize: u32) -> c_int;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    pub fn _CFMachPortSetOptions(mach_port: crate::sys::cf::CFMachPortRef, options: c_int);
}

#[link(name = "SkyLight", kind = "framework")]
unsafe extern "C" {
    pub fn SLSServerPort(zero: *mut c_void) -> MachPort;
    pub fn SLEventCreateNextEvent(cid: c_int) -> CGEventRef;
    pub fn _SLPSGetFrontProcess(psn: *mut ProcessSerialNumber) -> c_int;
}
