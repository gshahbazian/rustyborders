use std::ffi::c_void;
use std::ptr;

use crate::border::Border;
use crate::settings::Settings;
use crate::sys::cf::{
    CFArrayGetCount, CFArrayGetValueAtIndex, CFDictionaryGetValue, CFNumberGetValue, CFTypeRef,
    K_CF_NUMBER_SINT64_TYPE, OwnedCf, cf_string, cfarray_of_u32, cfarray_of_u64,
};
use crate::sys::geometry::{SpaceId, WindowId};
use crate::sys::mach::{
    MACH_MSG_TIMEOUT_NONE, MACH_MSG_TYPE_COPY_SEND, MACH_MSG_TYPE_MAKE_SEND_ONCE, MACH_PORT_NULL,
    MACH_RCV_MSG, MACH_RCV_SYNC_WAIT, MACH_SEND_MSG, MACH_SEND_PROPAGATE_QOS,
    MACH_SEND_SYNC_OVERRIDE, MachMsgHeader, NDR_RECORD, mach_msg, mach_msg_bits,
    mig_dealloc_special_reply_port, mig_get_special_reply_port,
};
use crate::sys::macho;
use crate::sys::os::{
    _AXUIElementGetWindow, _SLPSGetFrontProcess, AXIsProcessTrusted, AXIsProcessTrustedWithOptions,
    AXUIElementCopyAttributeValue, AXUIElementCreateApplication, CGDisplayCreateUUIDFromDisplayID,
    CGGetActiveDisplayList, ProcessSerialNumber, proc_name,
};
use crate::sys::skylight::{
    SLSConnectionGetPID, SLSCopyActiveMenuBarDisplayIdentifier, SLSCopyManagedDisplayForWindow,
    SLSCopyManagedDisplaySpaces, SLSCopyManagedDisplays, SLSCopySpacesForWindows,
    SLSCopyWindowsWithOptionsAndTags, SLSGetConnectionIDForPSN, SLSGetWindowOwner,
    SLSMainConnectionID, SLSManagedDisplayGetCurrentSpace, SLSRequestNotificationsForWindows,
    SLSWindowIteratorAdvance, SLSWindowIteratorGetAttributes, SLSWindowIteratorGetCount,
    SLSWindowIteratorGetLevel, SLSWindowIteratorGetParentID, SLSWindowIteratorGetTags,
    SLSWindowIteratorGetWindowID, SLSWindowQueryResultCopyWindows, SLSWindowQueryWindows,
};

pub const WINDOW_TAG_DOCUMENT: u64 = 1 << 0;
pub const WINDOW_TAG_FLOATING: u64 = 1 << 1;
pub const WINDOW_TAG_ATTACHED: u64 = 1 << 7;
pub const WINDOW_TAG_STICKY: u64 = 1 << 11;
pub const WINDOW_TAG_IGNORES_CYCLE: u64 = 1 << 18;
pub const WINDOW_TAG_MODAL: u64 = 1 << 31;

const PROC_PIDPATHINFO_MAXSIZE: usize = 4096;
const SKYLIGHT_IMAGE: &str =
    "/System/Library/PrivateFrameworks/SkyLight.framework/Versions/A/SkyLight";
const CGS_GET_CONNECTION_PORT_BY_ID: &str = "_CGSGetConnectionPortById";

pub fn app_name_for_window(cid: i32, wid: WindowId) -> Option<String> {
    let mut owner_cid = 0;
    unsafe {
        SLSGetWindowOwner(cid, wid.0, &mut owner_cid);
    }
    let mut pid = 0;
    unsafe {
        SLSConnectionGetPID(owner_cid, &mut pid);
    }

    let mut buffer = [0_u8; PROC_PIDPATHINFO_MAXSIZE];
    let len = unsafe {
        proc_name(
            pid,
            buffer.as_mut_ptr().cast::<c_void>(),
            buffer.len().try_into().ok()?,
        )
    };
    if len <= 0 {
        return None;
    }

    Some(String::from_utf8_lossy(&buffer[..len as usize]).into_owned())
}

pub fn is_own_window(pid: libc::pid_t, cid: i32, wid: WindowId) -> bool {
    let mut owner_cid = 0;
    unsafe {
        SLSGetWindowOwner(cid, wid.0, &mut owner_cid);
    }
    let mut owner_pid = 0;
    unsafe {
        SLSConnectionGetPID(owner_cid, &mut owner_pid);
    }
    owner_pid == pid
}

pub fn window_suitable(iterator: CFTypeRef) -> bool {
    let tags = unsafe { SLSWindowIteratorGetTags(iterator) };
    let attributes = unsafe { SLSWindowIteratorGetAttributes(iterator) };
    let parent_wid = unsafe { SLSWindowIteratorGetParentID(iterator) };

    parent_wid == 0
        && ((attributes & 0x2) != 0 || (tags & 0x400_0000_0000_0000) != 0)
        && (tags & WINDOW_TAG_ATTACHED) == 0
        && (tags & WINDOW_TAG_IGNORES_CYCLE) == 0
        && ((tags & WINDOW_TAG_DOCUMENT) != 0
            || ((tags & WINDOW_TAG_FLOATING) != 0 && (tags & WINDOW_TAG_MODAL) != 0))
}

pub fn window_tags(cid: i32, wid: WindowId) -> u64 {
    let Some(window_ref) = cfarray_of_u32(&[wid.0]) else {
        return 0;
    };

    let query = unsafe { SLSWindowQueryWindows(cid, window_ref.as_raw(), 0) };
    let Some(query) = (unsafe { OwnedCf::from_create_rule(query) }) else {
        return 0;
    };

    let iterator = unsafe { SLSWindowQueryResultCopyWindows(query.as_raw()) };
    let Some(iterator) = (unsafe { OwnedCf::from_create_rule(iterator) }) else {
        return 0;
    };

    if unsafe { SLSWindowIteratorGetCount(iterator.as_raw()) } > 0
        && unsafe { SLSWindowIteratorAdvance(iterator.as_raw()) }
    {
        unsafe { SLSWindowIteratorGetTags(iterator.as_raw()) }
    } else {
        0
    }
}

pub fn window_level(cid: i32, wid: WindowId) -> i32 {
    let Some(window_ref) = cfarray_of_u32(&[wid.0]) else {
        return 0;
    };
    let query = unsafe { SLSWindowQueryWindows(cid, window_ref.as_raw(), 0) };
    let Some(query) = (unsafe { OwnedCf::from_create_rule(query) }) else {
        return 0;
    };
    let iterator = unsafe { SLSWindowQueryResultCopyWindows(query.as_raw()) };
    let Some(iterator) = (unsafe { OwnedCf::from_create_rule(iterator) }) else {
        return 0;
    };

    if unsafe { SLSWindowIteratorAdvance(iterator.as_raw()) } {
        unsafe { SLSWindowIteratorGetLevel(iterator.as_raw()) }
    } else {
        0
    }
}

pub fn window_sub_level(server_port: crate::sys::mach::MachPort, wid: WindowId) -> i32 {
    if server_port == 0 {
        return 0;
    }
    let Ok(wid) = i32::try_from(wid.0) else {
        return 0;
    };

    let request = 0x73c3;
    let response = 0x7427;

    #[repr(C, packed(2))]
    struct Message {
        info: MessageInfo,
        payload: Payload,
        response: Response,
    }

    #[repr(C, packed(2))]
    struct MessageInfo {
        header: MachMsgHeader,
        ndr_record: crate::sys::mach::NdrRecord,
    }

    #[repr(C, packed(2))]
    struct Payload {
        wid: i32,
    }

    #[repr(C, packed(2))]
    struct Response {
        sub_level: i32,
        padding: i64,
    }

    let mut message = Message {
        info: MessageInfo {
            header: MachMsgHeader::default(),
            ndr_record: NDR_RECORD,
        },
        payload: Payload { wid },
        response: Response {
            sub_level: 0,
            padding: 0,
        },
    };

    message.info.header.msgh_remote_port = server_port;
    message.info.header.msgh_local_port = unsafe { mig_get_special_reply_port() };
    message.info.header.msgh_bits = mach_msg_bits(
        MACH_MSG_TYPE_COPY_SEND,
        MACH_MSG_TYPE_MAKE_SEND_ONCE,
        0,
        crate::sys::mach::MACH_MSGH_BITS_REMOTE_MASK,
    );
    message.info.header.msgh_id = request;
    let send_size =
        u32::try_from(std::mem::size_of::<MessageInfo>() + std::mem::size_of::<Payload>())
            .expect("sublevel request message fits in mach_msg_size_t");
    let receive_size = u32::try_from(std::mem::size_of::<Message>())
        .expect("sublevel response message fits in mach_msg_size_t");

    let error = unsafe {
        mach_msg(
            std::ptr::addr_of_mut!(message.info.header),
            MACH_SEND_MSG
                | MACH_SEND_SYNC_OVERRIDE
                | MACH_SEND_PROPAGATE_QOS
                | MACH_RCV_MSG
                | MACH_RCV_SYNC_WAIT,
            send_size,
            receive_size,
            message.info.header.msgh_local_port,
            MACH_MSG_TIMEOUT_NONE,
            MACH_PORT_NULL,
        )
    };

    if error != crate::sys::mach::KERN_SUCCESS {
        unsafe {
            mig_dealloc_special_reply_port(message.info.header.msgh_local_port);
        }
        return 0;
    }

    if message.info.header.msgh_id != response {
        unsafe {
            crate::sys::mach::mach_msg_destroy(std::ptr::addr_of_mut!(message.info.header));
        }
        return 0;
    }

    message.response.sub_level
}

pub fn create_connection_server_port() -> crate::sys::mach::MachPort {
    type CgsGetConnectionPortById = unsafe extern "C" fn(i32) -> crate::sys::mach::MachPort;
    let Some(symbol) =
        (unsafe { macho::find_symbol(SKYLIGHT_IMAGE, CGS_GET_CONNECTION_PORT_BY_ID) })
    else {
        return 0;
    };
    let function: CgsGetConnectionPortById = unsafe { std::mem::transmute(symbol) };
    unsafe { function(SLSMainConnectionID()) }
}

pub fn window_send_to_space(cid: i32, wid: WindowId, sid: SpaceId) {
    if let Some(window_list) = cfarray_of_u32(&[wid.0]) {
        unsafe {
            crate::sys::skylight::SLSMoveWindowsToManagedSpace(cid, window_list.as_raw(), sid.0);
        }
    }
}

pub fn window_space_id(cid: i32, wid: WindowId) -> SpaceId {
    let Some(window_list) = cfarray_of_u32(&[wid.0]) else {
        return SpaceId(0);
    };

    let spaces = unsafe { SLSCopySpacesForWindows(cid, 0x7, window_list.as_raw()) };
    if let Some(spaces) = unsafe { OwnedCf::from_create_rule(spaces) } {
        let count = unsafe { CFArrayGetCount(spaces.as_raw()) };
        if count > 0 {
            let id_ref = unsafe { CFArrayGetValueAtIndex(spaces.as_raw(), 0) };
            let mut sid = 0_u64;
            unsafe {
                CFNumberGetValue(
                    id_ref,
                    K_CF_NUMBER_SINT64_TYPE,
                    std::ptr::addr_of_mut!(sid).cast(),
                );
            }
            if sid != 0 {
                return SpaceId(sid);
            }
        }
    }

    let uuid = unsafe { SLSCopyManagedDisplayForWindow(cid, wid.0) };
    let Some(uuid) = (unsafe { OwnedCf::from_create_rule(uuid) }) else {
        return SpaceId(0);
    };
    SpaceId(unsafe { SLSManagedDisplayGetCurrentSpace(cid, uuid.as_raw()) })
}

pub fn get_active_space_id(cid: i32) -> SpaceId {
    let mut count = 0_u32;
    unsafe {
        CGGetActiveDisplayList(0, ptr::null_mut(), &mut count);
    }

    let uuid = if count == 1 {
        let mut display = 0_u32;
        let mut display_count = 0_u32;
        unsafe {
            CGGetActiveDisplayList(1, &mut display, &mut display_count);
        }
        if display_count != 1 {
            return SpaceId(0);
        }
        let uuid = unsafe { CGDisplayCreateUUIDFromDisplayID(display) };
        let Some(uuid) = (unsafe { OwnedCf::from_create_rule(uuid) }) else {
            return SpaceId(0);
        };
        let uuid_string = unsafe { crate::sys::cf::CFUUIDCreateString(ptr::null(), uuid.as_raw()) };
        unsafe { OwnedCf::from_create_rule(uuid_string) }
    } else {
        let uuid = unsafe { SLSCopyActiveMenuBarDisplayIdentifier(cid) };
        unsafe { OwnedCf::from_create_rule(uuid) }
    };

    let Some(uuid) = uuid else {
        return SpaceId(0);
    };
    SpaceId(unsafe { SLSManagedDisplayGetCurrentSpace(cid, uuid.as_raw()) })
}

pub fn is_space_visible(cid: i32, sid: SpaceId) -> bool {
    let displays = unsafe { SLSCopyManagedDisplays(cid) };
    let Some(displays) = (unsafe { OwnedCf::from_create_rule(displays) }) else {
        return false;
    };

    let count = unsafe { CFArrayGetCount(displays.as_raw()) };
    for index in 0..count {
        let display = unsafe { CFArrayGetValueAtIndex(displays.as_raw(), index) };
        let current = unsafe { SLSManagedDisplayGetCurrentSpace(cid, display.cast()) };
        if current == sid.0 {
            return true;
        }
    }

    false
}

pub fn get_front_window(cid: i32) -> WindowId {
    let active_sid = get_active_space_id(cid);
    if active_sid.0 == 0 {
        return WindowId(0);
    }

    let mut psn = ProcessSerialNumber::default();
    unsafe {
        _SLPSGetFrontProcess(&mut psn);
    }
    let mut target_cid = 0;
    unsafe {
        SLSGetConnectionIDForPSN(cid, &mut psn, &mut target_cid);
    }
    let Ok(target_cid) = u32::try_from(target_cid) else {
        return WindowId(0);
    };

    let Some(space_list) = cfarray_of_u64(&[active_sid.0]) else {
        return WindowId(0);
    };

    let mut set_tags = 1_u64;
    let mut clear_tags = 0_u64;
    let window_list = unsafe {
        SLSCopyWindowsWithOptionsAndTags(
            cid,
            target_cid,
            space_list.as_raw(),
            0x2,
            &mut set_tags,
            &mut clear_tags,
        )
    };
    let Some(window_list) = (unsafe { OwnedCf::from_create_rule(window_list) }) else {
        return WindowId(0);
    };

    if unsafe { CFArrayGetCount(window_list.as_raw()) } <= 0 {
        return WindowId(0);
    }

    let query = unsafe { SLSWindowQueryWindows(cid, window_list.as_raw(), 0) };
    let Some(query) = (unsafe { OwnedCf::from_create_rule(query) }) else {
        return WindowId(0);
    };
    let iterator = unsafe { SLSWindowQueryResultCopyWindows(query.as_raw()) };
    let Some(iterator) = (unsafe { OwnedCf::from_create_rule(iterator) }) else {
        return WindowId(0);
    };

    while unsafe { SLSWindowIteratorAdvance(iterator.as_raw()) } {
        if window_suitable(iterator.as_raw()) {
            return WindowId(unsafe { SLSWindowIteratorGetWindowID(iterator.as_raw()) });
        }
    }

    WindowId(0)
}

pub fn ax_check_trust(prompt: bool) -> bool {
    if !prompt {
        return unsafe { AXIsProcessTrusted() != 0 };
    }

    let Some(key) = cf_string("AXTrustedCheckOptionPrompt") else {
        return false;
    };
    let key_raw = key.as_raw();
    let value = unsafe { crate::sys::cf::kCFBooleanTrue };
    let dictionary = unsafe {
        crate::sys::cf::CFDictionaryCreate(
            ptr::null(),
            std::ptr::addr_of!(key_raw).cast(),
            std::ptr::addr_of!(value).cast(),
            1,
            &raw const crate::sys::cf::kCFTypeDictionaryKeyCallBacks,
            &raw const crate::sys::cf::kCFTypeDictionaryValueCallBacks,
        )
    };
    let Some(dictionary) = (unsafe { OwnedCf::from_create_rule(dictionary) }) else {
        return false;
    };
    unsafe { AXIsProcessTrustedWithOptions(dictionary.as_raw()) != 0 }
}

pub fn ax_get_front_window(cid: i32) -> WindowId {
    if !ax_check_trust(true) {
        return WindowId(0);
    }

    let mut psn = ProcessSerialNumber::default();
    unsafe {
        _SLPSGetFrontProcess(&mut psn);
    }
    let mut target_cid = 0;
    unsafe {
        SLSGetConnectionIDForPSN(cid, &mut psn, &mut target_cid);
    }

    let mut pid = 0;
    unsafe {
        SLSConnectionGetPID(target_cid, &mut pid);
    }

    let app = unsafe { AXUIElementCreateApplication(pid) };
    if app.is_null() {
        return WindowId(0);
    }
    let app = unsafe { OwnedCf::from_create_rule(app.cast_const()) };
    let Some(app) = app else {
        return WindowId(0);
    };

    let Some(focused_window_attribute) = cf_string("AXFocusedWindow") else {
        return WindowId(0);
    };
    let mut window: CFTypeRef = ptr::null();
    unsafe {
        AXUIElementCopyAttributeValue(
            app.as_raw().cast_mut(),
            focused_window_attribute.as_raw(),
            &mut window,
        );
    }
    let Some(window) = (unsafe { OwnedCf::from_create_rule(window) }) else {
        return WindowId(0);
    };

    let mut wid = 0_u32;
    unsafe {
        _AXUIElementGetWindow(window.as_raw(), &mut wid);
    }
    WindowId(wid)
}

pub fn copy_all_space_ids(cid: i32) -> Vec<SpaceId> {
    let display_spaces = unsafe { SLSCopyManagedDisplaySpaces(cid) };
    let Some(display_spaces) = (unsafe { OwnedCf::from_create_rule(display_spaces) }) else {
        return Vec::new();
    };

    let spaces_key = cf_string("Spaces");
    let id_key = cf_string("id64");
    let (Some(spaces_key), Some(id_key)) = (spaces_key, id_key) else {
        return Vec::new();
    };

    let mut result = Vec::new();
    let display_count = unsafe { CFArrayGetCount(display_spaces.as_raw()) };
    for display_index in 0..display_count {
        let display = unsafe { CFArrayGetValueAtIndex(display_spaces.as_raw(), display_index) };
        let spaces = unsafe { CFDictionaryGetValue(display.cast(), spaces_key.as_raw().cast()) };
        if spaces.is_null() {
            continue;
        }
        let spaces_count = unsafe { CFArrayGetCount(spaces.cast()) };
        for space_index in 0..spaces_count {
            let space = unsafe { CFArrayGetValueAtIndex(spaces.cast(), space_index) };
            let sid_ref = unsafe { CFDictionaryGetValue(space.cast(), id_key.as_raw().cast()) };
            if sid_ref.is_null() {
                continue;
            }
            let mut sid = 0_u64;
            unsafe {
                CFNumberGetValue(
                    sid_ref,
                    K_CF_NUMBER_SINT64_TYPE,
                    std::ptr::addr_of_mut!(sid).cast(),
                );
            }
            if sid != 0 {
                result.push(SpaceId(sid));
            }
        }
    }

    result
}

pub fn add_existing_windows(
    windows: &mut std::collections::HashMap<WindowId, Border>,
    pid: libc::pid_t,
    settings: &Settings,
    server_port: crate::sys::mach::MachPort,
) {
    let cid = unsafe { SLSMainConnectionID() };
    let space_ids = copy_all_space_ids(cid)
        .into_iter()
        .map(|sid| sid.0)
        .collect::<Vec<_>>();
    crate::rb_log!("existing window scan: spaces={space_ids:?}");
    let Some(space_list) = cfarray_of_u64(&space_ids) else {
        crate::rb_log!("existing window scan: failed to create CFArray for spaces");
        return;
    };

    let mut set_tags = 1_u64;
    let mut clear_tags = 0_u64;
    let window_list = unsafe {
        SLSCopyWindowsWithOptionsAndTags(
            cid,
            0,
            space_list.as_raw(),
            0x2,
            &mut set_tags,
            &mut clear_tags,
        )
    };
    let Some(window_list) = (unsafe { OwnedCf::from_create_rule(window_list) }) else {
        crate::rb_log!("existing window scan: SLSCopyWindowsWithOptionsAndTags returned null");
        return;
    };

    let count = unsafe { CFArrayGetCount(window_list.as_raw()) };
    crate::rb_log!("existing window scan: raw window count={count}");
    if count <= 0 {
        return;
    }

    let query = unsafe { SLSWindowQueryWindows(cid, window_list.as_raw(), 0) };
    let Some(query) = (unsafe { OwnedCf::from_create_rule(query) }) else {
        crate::rb_log!("existing window scan: SLSWindowQueryWindows returned null");
        return;
    };
    let iterator = unsafe { SLSWindowQueryResultCopyWindows(query.as_raw()) };
    let Some(iterator) = (unsafe { OwnedCf::from_create_rule(iterator) }) else {
        crate::rb_log!("existing window scan: SLSWindowQueryResultCopyWindows returned null");
        return;
    };

    let mut iterated = 0;
    let mut suitable = 0;
    let mut created = 0;
    while unsafe { SLSWindowIteratorAdvance(iterator.as_raw()) } {
        iterated += 1;
        if !window_suitable(iterator.as_raw()) {
            continue;
        }
        suitable += 1;
        let wid = WindowId(unsafe { SLSWindowIteratorGetWindowID(iterator.as_raw()) });
        if create_window_border(
            windows,
            pid,
            settings,
            server_port,
            wid,
            window_space_id(cid, wid),
        ) {
            created += 1;
        }
    }
    crate::rb_log!(
        "existing window scan: iterated={iterated} suitable={suitable} created={created} tracked={}",
        windows.len()
    );

    update_notifications(windows);
}

pub fn create_window_border(
    windows: &mut std::collections::HashMap<WindowId, Border>,
    pid: libc::pid_t,
    settings: &Settings,
    server_port: crate::sys::mach::MachPort,
    wid: WindowId,
    sid: SpaceId,
) -> bool {
    let cid = unsafe { SLSMainConnectionID() };
    if is_own_window(pid, cid, wid) {
        crate::rb_log!("window {wid}: skipped own process window");
        return false;
    }
    if let Some(app_name) = app_name_for_window(cid, wid)
        && !settings.app_allowed(&app_name)
    {
        crate::rb_log!("window {wid}: skipped by app filter app={app_name}");
        return false;
    }

    let Some(target_ref) = cfarray_of_u32(&[wid.0]) else {
        crate::rb_log!("window {wid}: failed to create target CFArray");
        return false;
    };
    let query = unsafe { SLSWindowQueryWindows(cid, target_ref.as_raw(), 0) };
    let Some(query) = (unsafe { OwnedCf::from_create_rule(query) }) else {
        crate::rb_log!("window {wid}: SLSWindowQueryWindows returned null");
        return false;
    };
    let iterator = unsafe { SLSWindowQueryResultCopyWindows(query.as_raw()) };
    let Some(iterator) = (unsafe { OwnedCf::from_create_rule(iterator) }) else {
        crate::rb_log!("window {wid}: SLSWindowQueryResultCopyWindows returned null");
        return false;
    };

    if unsafe { SLSWindowIteratorGetCount(iterator.as_raw()) } <= 0
        || !unsafe { SLSWindowIteratorAdvance(iterator.as_raw()) }
        || !window_suitable(iterator.as_raw())
    {
        crate::rb_log!("window {wid}: query iterator had no suitable window");
        return false;
    }

    let created = !windows.contains_key(&wid);
    let border = windows.entry(wid).or_insert_with(Border::new);
    let radius = crate::app::corner_radius_for_iterator(iterator.as_raw()).unwrap_or(9.0);
    border.radius = radius;
    border.inner_radius = radius + 1.0;
    border.target_wid = wid;
    border.sid = sid;
    crate::rb_log!("window {wid}: updating border created={created} sid={sid} radius={radius}");
    border.update(settings, server_port);
    crate::rb_log!(
        "window {wid}: border window={:?} focused={} too_small={} frame={:?}",
        border.wid,
        border.focused,
        border.too_small,
        border.frame
    );

    update_notifications(windows);
    created
}

pub fn update_notifications(windows: &std::collections::HashMap<WindowId, Border>) {
    let mut window_list = windows.keys().map(|wid| wid.0).collect::<Vec<_>>();
    let Ok(window_count) = i32::try_from(window_list.len()) else {
        return;
    };
    unsafe {
        SLSRequestNotificationsForWindows(
            SLSMainConnectionID(),
            window_list.as_mut_ptr(),
            window_count,
        );
    }
}
