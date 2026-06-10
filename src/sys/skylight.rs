#![allow(dead_code, non_snake_case)]

use std::os::raw::{c_int, c_void};

use crate::sys::cf::{CFArrayRef, CFDictionaryRef, CFStringRef, CFTypeRef};
use crate::sys::geometry::{CGAffineTransform, CGPoint, CGRect};
use crate::sys::mach::MachPort;
use crate::sys::os::{CGContextRef, CGError, CGEventRef, ProcessSerialNumber};

pub type SlsNotifyProc = unsafe extern "C" fn(u32, *mut c_void, usize, *mut c_void);

#[link(name = "SkyLight", kind = "framework")]
unsafe extern "C" {
    pub fn SLSMainConnectionID() -> c_int;
    pub fn SLSWindowManagementBridgeSetDelegate(delegate: *mut c_void) -> CGError;
    pub fn SLSGetEventPort(cid: c_int, port_out: *mut MachPort) -> CGError;
    pub fn SLSRegisterNotifyProc(
        handler: SlsNotifyProc,
        event: u32,
        context: *mut c_void,
    ) -> CGError;
    pub fn SLSGetWindowOwner(cid: c_int, wid: u32, out_cid: *mut c_int) -> CGError;
    pub fn SLSConnectionGetPID(cid: c_int, pid: *mut libc::pid_t) -> CGError;
    pub fn SLSRequestNotificationsForWindows(
        cid: c_int,
        window_list: *mut u32,
        window_count: c_int,
    ) -> CGError;

    pub fn SLSNewConnection(zero: c_int, cid: *mut c_int) -> CGError;
    pub fn SLSReleaseConnection(cid: c_int) -> CGError;
    pub fn SLSWindowIsOrderedIn(cid: c_int, wid: u32, shown: *mut bool) -> CGError;
    pub fn SLSGetWindowBounds(cid: c_int, wid: u32, frame: *mut CGRect) -> CGError;
    pub fn CGSNewRegionWithRect(rect: *const CGRect, out_region: *mut CFTypeRef) -> CGError;
    pub fn CGRegionCreateEmptyRegion() -> CFTypeRef;
    pub fn SLSNewWindow(
        cid: c_int,
        window_type: c_int,
        x: f32,
        y: f32,
        region: CFTypeRef,
        wid: *mut u32,
    ) -> CGError;
    pub fn SLSNewWindowWithOpaqueShapeAndContext(
        cid: c_int,
        window_type: c_int,
        region: CFTypeRef,
        opaque_shape: CFTypeRef,
        options: c_int,
        tags: *mut u64,
        x: f32,
        y: f32,
        tag_size: c_int,
        wid: *mut u32,
        context: *mut c_void,
    ) -> CGError;
    pub fn SLSReleaseWindow(cid: c_int, wid: u32) -> CGError;
    pub fn SLSSetWindowTags(cid: c_int, wid: u32, tags: *mut u64, tag_size: c_int) -> CGError;
    pub fn SLSClearWindowTags(cid: c_int, wid: u32, tags: *mut u64, tag_size: c_int) -> CGError;
    pub fn SLSSetWindowShape(
        cid: c_int,
        wid: u32,
        x_offset: f32,
        y_offset: f32,
        shape: CFTypeRef,
    ) -> CGError;
    pub fn SLSSetWindowResolution(cid: c_int, wid: u32, res: f64) -> CGError;
    pub fn SLSSetWindowOpacity(cid: c_int, wid: u32, is_opaque: bool) -> CGError;
    pub fn SLSSetWindowAlpha(cid: c_int, wid: u32, alpha: f32) -> CGError;
    pub fn SLSMoveWindow(cid: c_int, wid: u32, point: *const CGPoint) -> CGError;
    pub fn SLSOrderWindow(cid: c_int, wid: u32, order: c_int, rel_wid: u32) -> CGError;
    pub fn SLSSetWindowLevel(cid: c_int, wid: u32, level: c_int) -> CGError;
    pub fn SLSSetWindowSubLevel(cid: c_int, wid: u32, level: c_int) -> CGError;
    pub fn SLSSetWindowBackgroundBlurRadius(cid: c_int, wid: u32, radius: u32) -> CGError;
    pub fn SLSSetWindowShadowParameters(
        cid: c_int,
        wid: u32,
        std: f32,
        density: f32,
        x_offset: c_int,
        y_offset: c_int,
    ) -> CGError;
    pub fn SLSGetWindowTransform(
        cid: c_int,
        wid: u32,
        transform: *mut CGAffineTransform,
    ) -> CGError;
    pub fn SLSSetWindowTransform(cid: c_int, wid: u32, transform: CGAffineTransform) -> CGError;

    pub fn SLSWindowSetShadowProperties(wid: u32, properties: CFDictionaryRef) -> CGError;
    pub fn SLSGetWindowLevel(cid: c_int, wid: u32, level_out: *mut i64) -> CGError;
    pub fn SLSGetWindowSubLevel(cid: c_int, wid: u32) -> i32;
    pub fn SLSMoveWindowsToManagedSpace(cid: c_int, window_list: CFArrayRef, sid: u64) -> CGError;
    pub fn SLWindowContextCreate(cid: c_int, wid: u32, options: CFDictionaryRef) -> CGContextRef;

    pub fn SLSTransactionCreate(cid: c_int) -> CFTypeRef;
    pub fn SLSTransactionSetWindowLevel(transaction: CFTypeRef, wid: u32, level: c_int) -> CGError;
    pub fn SLSTransactionSetWindowSubLevel(
        transaction: CFTypeRef,
        wid: u32,
        level: c_int,
    ) -> CGError;
    pub fn SLSTransactionSetWindowShape(
        transaction: CFTypeRef,
        wid: u32,
        x_offset: f32,
        y_offset: f32,
        shape: CFTypeRef,
    ) -> CGError;
    pub fn SLSTransactionMoveWindowWithGroup(
        transaction: CFTypeRef,
        wid: u32,
        point: CGPoint,
    ) -> CGError;
    pub fn SLSTransactionOrderWindow(
        transaction: CFTypeRef,
        wid: u32,
        order: c_int,
        rel_wid: u32,
    ) -> CGError;
    pub fn SLSTransactionSetWindowAlpha(transaction: CFTypeRef, wid: u32, alpha: f32) -> CGError;
    pub fn SLSTransactionSetWindowSystemAlpha(
        transaction: CFTypeRef,
        wid: u32,
        alpha: f32,
    ) -> CGError;
    pub fn SLSTransactionSetWindowTransform(
        transaction: CFTypeRef,
        wid: u32,
        not: c_int,
        important: c_int,
        transform: CGAffineTransform,
    ) -> CGError;
    pub fn SLSTransactionCommit(transaction: CFTypeRef, synchronous: c_int) -> CGError;
    pub fn SLSTransactionCommitUsingMethod(transaction: CFTypeRef, method: u32) -> CGError;

    pub fn SLSWindowFreezeWithOptions(cid: c_int, wid: u32, options: CFTypeRef) -> CGError;
    pub fn SLSWindowThaw(cid: c_int, wid: u32) -> CGError;
    pub fn SLSCopySpacesForWindows(
        cid: c_int,
        selector: c_int,
        window_list: CFArrayRef,
    ) -> CFArrayRef;
    pub fn SLSDisableUpdate(cid: c_int) -> CGError;
    pub fn SLSReenableUpdate(cid: c_int) -> CGError;

    pub fn SLSGetConnectionIDForPSN(
        cid: c_int,
        psn: *mut ProcessSerialNumber,
        psn_cid: *mut c_int,
    ) -> CGError;
    pub fn SLSCopyConnectionProperty(
        cid: c_int,
        target_cid: c_int,
        key: CFStringRef,
        value: *mut CFTypeRef,
    ) -> CGError;

    pub fn SLSCopyWindowsWithOptionsAndTags(
        cid: c_int,
        owner: u32,
        spaces: CFArrayRef,
        options: u32,
        set_tags: *mut u64,
        clear_tags: *mut u64,
    ) -> CFArrayRef;

    pub fn SLSWindowQueryWindows(cid: c_int, windows: CFArrayRef, options: u32) -> CFTypeRef;
    pub fn SLSWindowQueryResultCopyWindows(window_query: CFTypeRef) -> CFTypeRef;
    pub fn SLSWindowIteratorGetCount(iterator: CFTypeRef) -> c_int;
    pub fn SLSWindowIteratorAdvance(iterator: CFTypeRef) -> bool;
    pub fn SLSWindowIteratorGetParentID(iterator: CFTypeRef) -> u32;
    pub fn SLSWindowIteratorGetWindowID(iterator: CFTypeRef) -> u32;
    pub fn SLSWindowIteratorGetTags(iterator: CFTypeRef) -> u64;
    pub fn SLSWindowIteratorGetAttributes(iterator: CFTypeRef) -> u64;
    pub fn SLSWindowIteratorGetLevel(iterator: CFTypeRef) -> c_int;

    pub fn SLSCopyManagedDisplays(cid: c_int) -> CFArrayRef;
    pub fn SLSCopyManagedDisplaySpaces(cid: c_int) -> CFArrayRef;
    pub fn SLSCopyManagedDisplayForWindow(cid: c_int, wid: u32) -> CFStringRef;
    pub fn SLSManagedDisplayGetCurrentSpace(cid: c_int, uuid: CFStringRef) -> u64;
    pub fn SLSCopyActiveMenuBarDisplayIdentifier(cid: c_int) -> CFStringRef;
    pub fn SLSFlushWindowContentRegion(cid: c_int, wid: u32, dirty: *mut c_void) -> CGError;
}

pub type CornerRadiiFn = unsafe extern "C" fn(CFTypeRef) -> CFArrayRef;

#[allow(dead_code)]
pub fn _event_ref_type(_: CGEventRef) {}
