use std::ptr;
use std::thread;
use std::time::Duration;

use crate::sys::cf::{
    CFMachPortCreateRunLoopSource, CFMachPortCreateWithPort, CFRelease, CFRunLoopAddSource,
    CFRunLoopGetCurrent, kCFRunLoopDefaultMode,
};
use crate::sys::geometry::{SpaceId, WindowId};
use crate::sys::mach::MachPort;
use crate::sys::os::{
    _CFMachPortSetOptions, CGEventRef, K_CG_ERROR_SUCCESS, SLEventCreateNextEvent,
};
use crate::sys::skylight::{SLSGetEventPort, SLSMainConnectionID, SLSRegisterNotifyProc};

pub const EVENT_WINDOW_UPDATE: u32 = 723;
pub const EVENT_WINDOW_CLOSE: u32 = 804;
pub const EVENT_WINDOW_MOVE: u32 = 806;
pub const EVENT_WINDOW_RESIZE: u32 = 807;
pub const EVENT_WINDOW_REORDER: u32 = 808;
pub const EVENT_WINDOW_LEVEL: u32 = 811;
pub const EVENT_WINDOW_UNHIDE: u32 = 815;
pub const EVENT_WINDOW_HIDE: u32 = 816;
pub const EVENT_WINDOW_TITLE: u32 = 1322;
pub const EVENT_WINDOW_CREATE: u32 = 1325;
pub const EVENT_WINDOW_DESTROY: u32 = 1326;
pub const EVENT_SPACE_CHANGE: u32 = 1401;
pub const EVENT_FRONT_CHANGE: u32 = 1508;

#[repr(C)]
#[derive(Clone, Copy)]
struct WindowSpawnData {
    sid: u64,
    wid: u32,
}

pub fn register(cid: i32) {
    for event in [
        EVENT_WINDOW_CLOSE,
        EVENT_WINDOW_MOVE,
        EVENT_WINDOW_RESIZE,
        EVENT_WINDOW_LEVEL,
        EVENT_WINDOW_UNHIDE,
        EVENT_WINDOW_HIDE,
        EVENT_WINDOW_TITLE,
        EVENT_WINDOW_REORDER,
        EVENT_WINDOW_UPDATE,
        EVENT_WINDOW_CREATE,
        EVENT_WINDOW_DESTROY,
        EVENT_SPACE_CHANGE,
        EVENT_FRONT_CHANGE,
    ] {
        unsafe {
            SLSRegisterNotifyProc(notify_callback, event, cid as isize as *mut _);
        }
    }
}

pub fn register_event_port(cid: i32) {
    let mut port: MachPort = 0;
    let err = unsafe { SLSGetEventPort(cid, &mut port) };
    if err != K_CG_ERROR_SUCCESS || port == 0 {
        return;
    }

    let cf_mach_port = unsafe {
        CFMachPortCreateWithPort(
            ptr::null(),
            port,
            event_callback,
            ptr::null_mut(),
            ptr::null_mut(),
        )
    };
    if cf_mach_port.is_null() {
        return;
    }

    unsafe {
        _CFMachPortSetOptions(cf_mach_port, 0x40);
    }

    let source = unsafe { CFMachPortCreateRunLoopSource(ptr::null(), cf_mach_port, 0) };
    if source.is_null() {
        unsafe {
            CFRelease(cf_mach_port);
        }
        return;
    }

    unsafe {
        CFRunLoopAddSource(CFRunLoopGetCurrent(), source, kCFRunLoopDefaultMode);
        CFRelease(cf_mach_port);
        CFRelease(source);
    }
}

unsafe extern "C" fn event_callback(
    _port: crate::sys::cf::CFMachPortRef,
    _message: *mut std::ffi::c_void,
    _size: isize,
    _context: *mut std::ffi::c_void,
) {
    let cid = unsafe { SLSMainConnectionID() };
    let mut event: CGEventRef = unsafe { SLEventCreateNextEvent(cid) };
    while !event.is_null() {
        unsafe {
            CFRelease(event.cast_const());
        }
        event = unsafe { SLEventCreateNextEvent(cid) };
    }
}

unsafe extern "C" fn notify_callback(
    event: u32,
    data: *mut std::ffi::c_void,
    _data_length: usize,
    context: *mut std::ffi::c_void,
) {
    let Ok(cid) = i32::try_from(context as isize) else {
        return;
    };
    match event {
        EVENT_WINDOW_CREATE | EVENT_WINDOW_DESTROY => {
            if data.is_null() {
                return;
            }
            let spawn = unsafe { data.cast::<WindowSpawnData>().read_unaligned() };
            let wid = WindowId(spawn.wid);
            let sid = SpaceId(spawn.sid);
            if wid.0 == 0 || sid.0 == 0 {
                return;
            }
            if event == EVENT_WINDOW_CREATE {
                crate::app::handle_window_create(wid, sid, cid);
            } else {
                crate::app::handle_window_destroy(wid, sid);
                crate::app::determine_and_focus_active_window();
            }
        }
        EVENT_WINDOW_MOVE | EVENT_WINDOW_RESIZE | EVENT_WINDOW_REORDER | EVENT_WINDOW_LEVEL
        | EVENT_WINDOW_UNHIDE | EVENT_WINDOW_HIDE | EVENT_WINDOW_CLOSE | EVENT_WINDOW_TITLE
        | EVENT_WINDOW_UPDATE => {
            if data.is_null() {
                return;
            }
            let wid = WindowId(unsafe { data.cast::<u32>().read_unaligned() });
            match event {
                EVENT_WINDOW_MOVE => crate::app::handle_window_move(wid, cid),
                EVENT_WINDOW_RESIZE | EVENT_WINDOW_LEVEL => {
                    crate::app::handle_window_update(wid, cid)
                }
                EVENT_WINDOW_REORDER => {
                    crate::app::handle_window_update(wid, cid);
                    delayed_focus(Duration::from_micros(10_000));
                }
                EVENT_WINDOW_TITLE | EVENT_WINDOW_UPDATE => {
                    delayed_focus(Duration::from_micros(50_000))
                }
                EVENT_WINDOW_UNHIDE => crate::app::handle_window_unhide(wid, cid),
                EVENT_WINDOW_HIDE => crate::app::handle_window_hide(wid, cid),
                EVENT_WINDOW_CLOSE => crate::app::handle_window_close(wid),
                _ => {}
            }
        }
        EVENT_FRONT_CHANGE => delayed_focus(Duration::from_micros(50_000)),
        EVENT_SPACE_CHANGE => {
            thread::spawn(|| {
                thread::sleep(Duration::from_micros(20_000));
                crate::app::draw_borders_on_current_spaces();
            });
        }
        _ => {}
    }
}

fn delayed_focus(delay: Duration) {
    thread::spawn(move || {
        thread::sleep(delay);
        crate::app::determine_and_focus_active_window();
    });
}
