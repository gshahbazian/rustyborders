use std::ptr;

use crate::drawing;
use crate::settings::{BORDER_ORDER_BELOW, BORDER_PADDING, ColorStyle, Settings};
use crate::sys::cf::{
    CFDictionaryCreate, CFNumberCreate, CFRelease, CFTypeRef, K_CF_NUMBER_CFINDEX_TYPE, OwnedCf,
    cf_string,
};
use crate::sys::geometry::{
    CGAffineTransform, CGPoint, CGRect, SpaceId, WindowId, is_more_than_half_visible,
};
use crate::sys::os::{
    CGContextClearRect, CGContextFlush, CGContextRef, CGContextRestoreGState, CGContextSaveGState,
    CGContextSetInterpolationQuality, CGContextSetLineWidth, K_CG_BACKING_STORE_BUFFERED,
    K_CG_INTERPOLATION_NONE,
};
use crate::sys::skylight::{
    CGRegionCreateEmptyRegion, CGSNewRegionWithRect, SLSClearWindowTags, SLSDisableUpdate,
    SLSFlushWindowContentRegion, SLSGetWindowBounds, SLSMainConnectionID, SLSMoveWindow,
    SLSNewConnection, SLSNewWindowWithOpaqueShapeAndContext, SLSOrderWindow, SLSReenableUpdate,
    SLSReleaseConnection, SLSReleaseWindow, SLSSetWindowAlpha, SLSSetWindowLevel,
    SLSSetWindowOpacity, SLSSetWindowResolution, SLSSetWindowShape, SLSSetWindowSubLevel,
    SLSSetWindowTags, SLSWindowFreezeWithOptions, SLSWindowIsOrderedIn,
    SLSWindowSetShadowProperties, SLSWindowThaw, SLWindowContextCreate,
};
use crate::windows::{
    WINDOW_TAG_STICKY, active_display_bounds, is_space_visible, window_level, window_send_to_space,
    window_space_id, window_sub_level, window_tags,
};

#[derive(Debug)]
pub struct Border {
    cid: i32,
    pub focused: bool,
    pub needs_redraw: bool,
    pub too_small: bool,
    pub sticky: bool,
    /// Whether the border window is currently ordered in. The cheap move path is
    /// only valid while it is, since that path never orders the window in.
    pub ordered_in: bool,
    pub sid: SpaceId,
    pub wid: Option<WindowId>,
    pub target_wid: WindowId,
    pub radius: f64,
    pub inner_radius: f64,
    pub origin: CGPoint,
    pub frame: CGRect,
    pub target_bounds: CGRect,
    /// The target window's rect expressed in the border window's own coordinate
    /// space: inset from `frame` by the border width plus padding on all four
    /// sides. Everything is drawn in this space, which is why the border needs
    /// no knowledge of which display the target is on.
    pub drawing_bounds: CGRect,
    pub context: CGContextRef,
    pub setting_override: Option<Settings>,
}

// SkyLight/CoreGraphics handles are serialized through the global app mutex.
unsafe impl Send for Border {}

impl Border {
    pub fn new() -> Self {
        let mut cid = 0;
        unsafe {
            SLSNewConnection(0, &mut cid);
        }
        if cid == 0 {
            cid = unsafe { SLSMainConnectionID() };
        }

        Self {
            cid,
            focused: false,
            needs_redraw: true,
            too_small: false,
            sticky: false,
            ordered_in: false,
            sid: SpaceId(0),
            wid: None,
            target_wid: WindowId(0),
            radius: 9.0,
            inner_radius: 10.0,
            origin: CGPoint::ZERO,
            frame: CGRect::ZERO,
            target_bounds: CGRect::ZERO,
            drawing_bounds: CGRect::ZERO,
            context: ptr::null_mut(),
            setting_override: None,
        }
    }

    pub fn settings<'a>(&'a self, global: &'a Settings) -> &'a Settings {
        self.setting_override.as_ref().unwrap_or(global)
    }

    pub fn set_override(&mut self, settings: Settings) {
        self.setting_override = Some(settings);
        self.needs_redraw = true;
    }

    pub fn update(&mut self, global_settings: &Settings, server_port: crate::sys::mach::MachPort) {
        let settings = self.settings(global_settings).clone();
        self.update_internal(&settings, server_port);
    }

    /// Handles `EVENT_WINDOW_MOVE`, the hot path while a window is dragged.
    ///
    /// A pure move leaves the border's pixels untouched — the content is drawn in
    /// window-local coordinates, so only the placement transform changes. That
    /// makes the queries and the full redraw unnecessary; anything else (a resize
    /// arriving as a move, a hidden border, a width change) falls back to the
    /// full update.
    pub fn move_border(
        &mut self,
        global_settings: &Settings,
        server_port: crate::sys::mach::MachPort,
    ) {
        let settings = self.settings(global_settings).clone();

        let Some(wid) = self.wid.filter(|_| self.ordered_in && !self.too_small) else {
            self.needs_redraw = true;
            self.update_internal(&settings, server_port);
            return;
        };

        let mut window_frame = CGRect::ZERO;
        unsafe {
            SLSGetWindowBounds(self.cid, self.target_wid.0, &mut window_frame);
        }

        let border_offset = -settings.border_width - BORDER_PADDING;
        let expected_frame = CGRect {
            origin: CGPoint::ZERO,
            size: window_frame.inset(border_offset, border_offset).size,
        };
        if expected_frame != self.frame {
            self.needs_redraw = true;
            self.update_internal(&settings, server_port);
            return;
        }

        if let Some(displays) = active_display_bounds()
            && !is_more_than_half_visible(window_frame, &displays)
        {
            self.target_bounds = window_frame;
            self.hide();
            return;
        }

        self.target_bounds = window_frame;
        self.origin = CGPoint {
            x: window_frame.origin.x + border_offset,
            y: window_frame.origin.y + border_offset,
        };
        self.commit_placement(wid, None);
    }

    pub fn hide(&mut self) {
        if let Some(wid) = self.wid {
            unsafe {
                SLSOrderWindow(self.cid, wid.0, 0, self.target_wid.0);
            }
            self.ordered_in = false;
        }
    }

    pub fn unhide(&mut self, global_settings: &Settings, server_port: crate::sys::mach::MachPort) {
        self.needs_redraw = true;
        self.update(global_settings, server_port);
    }

    fn update_internal(&mut self, settings: &Settings, server_port: crate::sys::mach::MachPort) {
        let Some(frame) = self.calculate_bounds(settings) else {
            crate::rb_log!(
                "window {}: update aborted while calculating bounds",
                self.target_wid
            );
            return;
        };
        crate::rb_log!(
            "window {}: bounds frame={:?} origin={:?}",
            self.target_wid,
            frame,
            self.origin
        );

        match active_display_bounds() {
            Some(displays) if !is_more_than_half_visible(self.target_bounds, &displays) => {
                crate::rb_log!(
                    "window {}: hiding border because at most half is visible on screen bounds={:?}",
                    self.target_wid,
                    self.target_bounds
                );
                self.hide();
                return;
            }
            None => crate::rb_log!(
                "window {}: unable to read active display bounds; skipping visibility check",
                self.target_wid
            ),
            Some(_) => {}
        }

        let tags = window_tags(self.cid, self.target_wid);
        self.sticky = tags & WINDOW_TAG_STICKY != 0;
        if !self.sticky && !is_space_visible(self.cid, self.sid) {
            crate::rb_log!(
                "window {}: update skipped because space {} is not visible tags=0x{tags:x}",
                self.target_wid,
                self.sid
            );
            return;
        }

        let mut shown = false;
        unsafe {
            SLSWindowIsOrderedIn(self.cid, self.target_wid.0, &mut shown);
        }
        if !shown {
            crate::rb_log!(
                "window {}: target not ordered in; hiding border",
                self.target_wid
            );
            self.hide();
            return;
        }

        let level = window_level(self.cid, self.target_wid);
        let sub_level = window_sub_level(server_port, self.target_wid);
        crate::rb_log!(
            "window {}: level={level} sub_level={sub_level} sticky={} shown={shown}",
            self.target_wid,
            self.sticky
        );

        if self.wid.is_none() {
            self.create_window(frame);
        }

        let Some(wid) = self.wid else {
            crate::rb_log!("window {}: failed to create border window", self.target_wid);
            return;
        };

        let disabled_update = if frame != self.frame {
            unsafe {
                SLSDisableUpdate(self.cid);
            }
            self.reshape(wid, frame);
            self.needs_redraw = true;
            self.frame = frame;
            true
        } else {
            false
        };

        if self.needs_redraw {
            self.draw(frame, settings);
        }

        if !self.focused && !settings.inactive_border_visible() {
            crate::rb_log!(
                "window {}: hiding border because it is unfocused and the inactive color is invisible",
                self.target_wid
            );
            self.hide();
            if disabled_update {
                unsafe {
                    SLSReenableUpdate(self.cid);
                }
            }
            return;
        }

        self.commit_placement(wid, Some((level, sub_level)));

        let mut set_tags = (1_u64 << 1) | (1_u64 << 9);
        let mut clear_tags = 0_u64;
        if self.sticky {
            set_tags |= WINDOW_TAG_STICKY;
            clear_tags |= 1_u64 << 45;
        }
        unsafe {
            SLSSetWindowTags(self.cid, wid.0, &mut set_tags, 0x40);
            SLSClearWindowTags(self.cid, wid.0, &mut clear_tags, 0x40);
        }

        if disabled_update {
            unsafe {
                SLSReenableUpdate(self.cid);
            }
        }
    }

    fn calculate_bounds(&mut self, settings: &Settings) -> Option<CGRect> {
        let mut window_frame = CGRect::ZERO;
        unsafe {
            SLSGetWindowBounds(self.cid, self.target_wid.0, &mut window_frame);
        }

        self.target_bounds = window_frame;
        self.too_small = self.check_too_small(window_frame);
        if self.too_small {
            crate::rb_log!(
                "window {}: target too small for radius {} bounds={:?}",
                self.target_wid,
                self.inner_radius,
                window_frame
            );
            self.hide();
            return None;
        }

        let border_offset = -settings.border_width - BORDER_PADDING;
        let mut frame = window_frame.inset(border_offset, border_offset);
        self.origin = frame.origin;
        frame.origin = CGPoint::ZERO;

        self.drawing_bounds = CGRect {
            origin: CGPoint {
                x: -border_offset,
                y: -border_offset,
            },
            size: window_frame.size,
        };

        Some(frame)
    }

    fn check_too_small(&self, window_frame: CGRect) -> bool {
        let smallest_rect = window_frame.inset(1.0, 1.0);
        smallest_rect.size.width < 2.0 * self.inner_radius
            || smallest_rect.size.height < 2.0 * self.inner_radius
    }

    /// Applies position (and optionally level/sublevel/order) in one atomic
    /// compositor transaction.
    ///
    /// These used to be issued as separate `SLS*` calls, which let the
    /// compositor present a frame with the new position but the previous
    /// transform — a visible flash while a window was being resized, since the
    /// transform is what places the content.
    fn commit_placement(&mut self, wid: WindowId, ordering: Option<(i32, i32)>) {
        let transform = CGAffineTransform::translation(-self.origin.x, -self.origin.y);
        let transaction = unsafe { crate::sys::skylight::SLSTransactionCreate(self.cid) };
        if transaction.is_null() {
            crate::rb_log!(
                "window {}: SLSTransactionCreate failed; applying placement directly",
                self.target_wid
            );
            unsafe {
                SLSMoveWindow(self.cid, wid.0, std::ptr::addr_of!(self.origin));
                crate::sys::skylight::SLSSetWindowTransform(self.cid, wid.0, transform);
                if let Some((level, sub_level)) = ordering {
                    SLSSetWindowLevel(self.cid, wid.0, level);
                    SLSSetWindowSubLevel(self.cid, wid.0, sub_level);
                    SLSOrderWindow(self.cid, wid.0, BORDER_ORDER_BELOW, self.target_wid.0);
                }
            }
            if ordering.is_some() {
                self.ordered_in = true;
            }
            return;
        }

        unsafe {
            crate::sys::skylight::SLSTransactionMoveWindowWithGroup(
                transaction,
                wid.0,
                self.origin,
            );
            crate::sys::skylight::SLSTransactionSetWindowTransform(
                transaction,
                wid.0,
                0,
                0,
                transform,
            );
            if let Some((level, sub_level)) = ordering {
                crate::sys::skylight::SLSTransactionSetWindowLevel(transaction, wid.0, level);
                crate::sys::skylight::SLSTransactionSetWindowSubLevel(
                    transaction,
                    wid.0,
                    sub_level,
                );
                crate::sys::skylight::SLSTransactionOrderWindow(
                    transaction,
                    wid.0,
                    BORDER_ORDER_BELOW,
                    self.target_wid.0,
                );
            }
            crate::sys::skylight::SLSTransactionCommit(transaction, 0);
            CFRelease(transaction);
        }

        if ordering.is_some() {
            self.ordered_in = true;
        }
    }

    fn reshape(&mut self, wid: WindowId, frame: CGRect) {
        let mut region: CFTypeRef = ptr::null();
        unsafe {
            CGSNewRegionWithRect(&frame, &mut region);
        }
        if region.is_null() {
            crate::rb_log!(
                "window {}: failed to build shape region for {frame:?}",
                self.target_wid
            );
            return;
        }

        unsafe {
            SLSWindowFreezeWithOptions(self.cid, wid.0, ptr::null());
            let err = SLSSetWindowShape(
                self.cid,
                wid.0,
                frame.origin.x as f32,
                frame.origin.y as f32,
                region,
            );
            CFRelease(region);
            if err != 0 {
                crate::rb_log!(
                    "window {}: SLSSetWindowShape failed err={err}",
                    self.target_wid
                );
            }
        }
    }

    fn create_window(&mut self, frame: CGRect) {
        let wid = create_sls_window(self.cid, frame);
        let Some(wid) = wid else {
            crate::rb_log!(
                "window {}: create_sls_window returned None frame={:?}",
                self.target_wid,
                frame
            );
            return;
        };

        self.wid = Some(wid);
        self.frame = frame;
        self.needs_redraw = true;
        self.context = unsafe { SLWindowContextCreate(self.cid, wid.0, ptr::null()) };
        if !self.context.is_null() {
            unsafe {
                CGContextSetInterpolationQuality(self.context, K_CG_INTERPOLATION_NONE);
            }
        } else {
            crate::rb_log!(
                "window {}: SLWindowContextCreate returned null for border window {}",
                self.target_wid,
                wid
            );
        }

        if self.sid.0 == 0 {
            self.sid = window_space_id(self.cid, self.target_wid);
        }
        window_send_to_space(self.cid, wid, self.sid);
        crate::rb_log!(
            "window {}: created border window {} sid={} context_null={}",
            self.target_wid,
            wid,
            self.sid,
            self.context.is_null()
        );
    }

    fn draw(&mut self, frame: CGRect, settings: &Settings) {
        if self.context.is_null() {
            crate::rb_log!(
                "window {}: draw skipped because context is null",
                self.target_wid
            );
            return;
        }

        unsafe {
            CGContextSaveGState(self.context);
        }
        self.needs_redraw = false;
        let color_style = if self.focused {
            settings.active_window.clone()
        } else {
            settings.inactive_window.clone()
        };

        let gradient = if matches!(color_style, ColorStyle::Gradient(_)) {
            unsafe {
                drawing::create_gradient(
                    &color_style,
                    CGAffineTransform::scale(frame.size.width, frame.size.height),
                )
            }
        } else {
            if let Some(color) = color_style.solid_color() {
                unsafe {
                    drawing::set_stroke_and_fill(self.context, color, color_style.is_glow());
                }
            }
            None
        };

        unsafe {
            CGContextSetLineWidth(self.context, settings.border_width);
            CGContextClearRect(self.context, frame);
        }

        let path_rect = self.drawing_bounds;
        let Some(inner_clip_path) = (unsafe { drawing::new_mutable_path() }) else {
            unsafe {
                CGContextRestoreGState(self.context);
            }
            return;
        };

        unsafe {
            drawing::add_rounded_rect_to_path(
                inner_clip_path,
                path_rect.inset(1.0, 1.0),
                self.inner_radius,
            );
        }

        unsafe {
            drawing::clip_between_rect_and_path(self.context, frame, inner_clip_path.cast());
        }

        let corner_radius = self.radius;
        if let Some((gradient_ref, direction)) = gradient {
            unsafe {
                drawing::draw_rounded_gradient_with_inset(
                    self.context,
                    gradient_ref,
                    direction,
                    path_rect,
                    corner_radius,
                );
                drawing::release_gradient(gradient_ref);
            }
        } else {
            unsafe {
                drawing::draw_rounded_rect_with_inset(
                    self.context,
                    path_rect,
                    corner_radius,
                    false,
                );
            }
        }

        unsafe {
            CFRelease(inner_clip_path.cast_const());
            CGContextFlush(self.context);
            CGContextRestoreGState(self.context);
        }

        if let Some(wid) = self.wid {
            unsafe {
                SLSFlushWindowContentRegion(self.cid, wid.0, ptr::null_mut());
                SLSWindowThaw(self.cid, wid.0);
            }
            crate::rb_log!(
                "window {}: drew border window {} focused={} color={}",
                self.target_wid,
                wid,
                self.focused,
                color_style
            );
        }
    }

    pub fn destroy(&mut self) {
        self.hide();
        self.destroy_window();
    }

    fn destroy_window(&mut self) {
        if !self.context.is_null() {
            unsafe {
                CFRelease(self.context.cast_const());
            }
            self.context = ptr::null_mut();
        }
        if let Some(wid) = self.wid.take() {
            unsafe {
                SLSReleaseWindow(self.cid, wid.0);
            }
        }
        self.ordered_in = false;
    }
}

impl Drop for Border {
    fn drop(&mut self) {
        self.destroy_window();
        if self.cid != unsafe { SLSMainConnectionID() } {
            unsafe {
                SLSReleaseConnection(self.cid);
            }
        }
    }
}

/// Creates the border window. `frame` is its shape and the coordinate space
/// drawing happens in; the window is placed on the desktop by the caller's
/// move + transform.
fn create_sls_window(cid: i32, frame: CGRect) -> Option<WindowId> {
    let mut frame_region: CFTypeRef = ptr::null();
    unsafe {
        let err = CGSNewRegionWithRect(&frame, &mut frame_region);
        if err != 0 {
            crate::rb_log!("CGSNewRegionWithRect failed err={err} frame={frame:?}");
        }
    }
    if frame_region.is_null() {
        return None;
    }
    let frame_region = unsafe { OwnedCf::from_create_rule(frame_region) }?;

    let mut id = 0_u32;
    let mut set_tags = (1_u64 << 1) | (1_u64 << 9);
    let mut clear_tags = 0_u64;

    crate::rb_log!("creating border window frame={frame:?}");
    let empty_region = unsafe { CGRegionCreateEmptyRegion() };
    let empty_region = (unsafe { OwnedCf::from_create_rule(empty_region) })?;
    unsafe {
        let err = SLSNewWindowWithOpaqueShapeAndContext(
            cid,
            K_CG_BACKING_STORE_BUFFERED,
            frame_region.as_raw(),
            empty_region.as_raw(),
            13 | (1 << 18),
            &mut set_tags,
            -9999.0_f32,
            -9999.0_f32,
            64,
            &mut id,
            ptr::null_mut(),
        );
        if err != 0 {
            crate::rb_log!("SLSNewWindowWithOpaqueShapeAndContext failed err={err}");
        }
        SLSSetWindowAlpha(cid, id, 0.0_f32);
    }

    if id == 0 {
        crate::rb_log!("SLSNewWindowWithOpaqueShapeAndContext returned id=0 frame={frame:?}");
        return None;
    }

    unsafe {
        SLSSetWindowResolution(cid, id, 2.0);
        let shape_err = SLSSetWindowShape(
            cid,
            id,
            frame.origin.x as f32,
            frame.origin.y as f32,
            frame_region.as_raw(),
        );
        if shape_err != 0 {
            crate::rb_log!("SLSSetWindowShape initial failed err={shape_err} wid={id}");
        }
        SLSSetWindowTags(cid, id, &mut set_tags, 64);
        SLSClearWindowTags(cid, id, &mut clear_tags, 64);
        SLSSetWindowOpacity(cid, id, false);
        let alpha_err = SLSSetWindowAlpha(cid, id, 1.0_f32);
        if alpha_err != 0 {
            crate::rb_log!("SLSSetWindowAlpha(1.0) failed err={alpha_err} wid={id}");
        }
    }

    clear_shadow(id);
    Some(WindowId(id))
}

fn clear_shadow(wid: u32) {
    let Some(key) = cf_string("com.apple.WindowShadowDensity") else {
        return;
    };
    let density: isize = 0;
    let density_ref = unsafe {
        OwnedCf::from_create_rule(CFNumberCreate(
            ptr::null(),
            K_CF_NUMBER_CFINDEX_TYPE,
            std::ptr::addr_of!(density).cast(),
        ))
    };
    let Some(density_ref) = density_ref else {
        return;
    };

    let key_raw = key.as_raw();
    let density_raw = density_ref.as_raw();
    let dictionary = unsafe {
        OwnedCf::from_create_rule(CFDictionaryCreate(
            ptr::null(),
            std::ptr::addr_of!(key_raw).cast(),
            std::ptr::addr_of!(density_raw).cast(),
            1,
            &raw const crate::sys::cf::kCFTypeDictionaryKeyCallBacks,
            &raw const crate::sys::cf::kCFTypeDictionaryValueCallBacks,
        ))
    };
    if let Some(dictionary) = dictionary {
        unsafe {
            SLSWindowSetShadowProperties(wid, dictionary.as_raw());
        }
    }
}
