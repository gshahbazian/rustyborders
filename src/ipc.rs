use std::ffi::CString;
use std::ptr;

use crate::parser::parse_settings;
use crate::settings::{Settings, UpdateMask};
use crate::sys::cf::{
    CFMachPortContext, CFMachPortCreateRunLoopSource, CFMachPortCreateWithPort, CFRelease,
    CFRunLoopAddSource, CFRunLoopGetMain, kCFRunLoopDefaultMode,
};
use crate::sys::mach::{
    KERN_SUCCESS, MACH_MSG_OOL_DESCRIPTOR, MACH_MSG_TIMEOUT_NONE, MACH_MSG_TYPE_COPY_SEND,
    MACH_MSG_VIRTUAL_COPY, MACH_MSGH_BITS_COMPLEX, MACH_PORT_LIMITS_INFO,
    MACH_PORT_LIMITS_INFO_COUNT, MACH_PORT_NULL, MACH_PORT_QLIMIT_LARGE, MACH_PORT_RIGHT_RECEIVE,
    MACH_SEND_MSG, MachMsgHeader, MachMsgOolDescriptor, MachPort, MachPortLimits,
    TASK_BOOTSTRAP_PORT, bootstrap_look_up, bootstrap_register, mach_msg, mach_msg_bits,
    mach_port_allocate, mach_port_insert_right, mach_port_set_attributes, mach_task_self,
    task_get_special_port,
};

pub const BS_NAME: &str = "git.felix.borders";

#[derive(Debug, Default)]
pub struct MachServer {
    pub is_running: bool,
    pub port: MachPort,
}

#[repr(C)]
#[derive(Default)]
struct MachMessage {
    header: MachMsgHeader,
    descriptor_count: u32,
    descriptor: MachMsgOolDescriptor,
}

pub fn lookup_server_port() -> MachPort {
    let task = unsafe { mach_task_self() };
    let mut bs_port = 0;
    if unsafe { task_get_special_port(task, TASK_BOOTSTRAP_PORT, &mut bs_port) } != KERN_SUCCESS {
        return 0;
    }

    let Ok(name) = CString::new(BS_NAME) else {
        return 0;
    };
    let mut port = 0;
    if unsafe { bootstrap_look_up(bs_port, name.as_ptr(), &mut port) } != KERN_SUCCESS {
        return 0;
    }
    port
}

pub fn send_args_to_server(port: MachPort, arguments: &[String]) {
    if port == 0 || arguments.is_empty() {
        return;
    }

    let mut message = Vec::new();
    for argument in arguments {
        message.extend_from_slice(argument.as_bytes());
        message.push(0);
    }
    message.push(0);
    send_message(port, &message);
}

pub fn decode_message(data: *const u8, len: usize) -> Vec<String> {
    if data.is_null() || len == 0 {
        return Vec::new();
    }

    let bytes = unsafe { std::slice::from_raw_parts(data, len) };
    bytes
        .split(|byte| *byte == 0)
        .filter(|part| !part.is_empty())
        .map(|part| String::from_utf8_lossy(part).into_owned())
        .collect()
}

pub fn parse_message_settings(
    current: &Settings,
    arguments: &[String],
) -> Result<(Settings, UpdateMask), crate::parser::ParseError> {
    let mut settings = current.clone();
    let update_mask = parse_settings(&mut settings, arguments)?;
    Ok((settings, update_mask))
}

fn send_message(port: MachPort, bytes: &[u8]) {
    let mut msg = MachMessage::default();
    msg.header.msgh_remote_port = port;
    msg.header.msgh_bits = mach_msg_bits(
        MACH_MSG_TYPE_COPY_SEND & crate::sys::mach::MACH_MSGH_BITS_REMOTE_MASK,
        0,
        0,
        MACH_MSGH_BITS_COMPLEX,
    );
    msg.header.msgh_size = std::mem::size_of::<MachMessage>() as u32;
    msg.descriptor_count = 1;
    msg.descriptor.address = bytes.as_ptr().cast_mut().cast();
    msg.descriptor.size = bytes.len() as u32;
    msg.descriptor.copy = MACH_MSG_VIRTUAL_COPY;
    msg.descriptor.deallocate = false;
    msg.descriptor.descriptor_type = MACH_MSG_OOL_DESCRIPTOR;

    unsafe {
        mach_msg(
            &mut msg.header,
            MACH_SEND_MSG,
            std::mem::size_of::<MachMessage>() as u32,
            0,
            MACH_PORT_NULL,
            MACH_MSG_TIMEOUT_NONE,
            MACH_PORT_NULL,
        );
    }
}

pub fn begin_server(server: &mut MachServer) -> bool {
    let task = unsafe { mach_task_self() };
    let mut port = 0;
    if unsafe { mach_port_allocate(task, MACH_PORT_RIGHT_RECEIVE, &mut port) } != KERN_SUCCESS {
        return false;
    }

    let mut limits = MachPortLimits {
        mpl_qlimit: MACH_PORT_QLIMIT_LARGE,
    };
    if unsafe {
        mach_port_set_attributes(
            task,
            port,
            MACH_PORT_LIMITS_INFO,
            std::ptr::addr_of_mut!(limits).cast(),
            MACH_PORT_LIMITS_INFO_COUNT,
        )
    } != KERN_SUCCESS
    {
        return false;
    }

    if unsafe {
        mach_port_insert_right(task, port, port, crate::sys::mach::MACH_MSG_TYPE_MAKE_SEND)
    } != KERN_SUCCESS
    {
        return false;
    }

    if !register_port(port) {
        return false;
    }

    let mut context = CFMachPortContext {
        version: 0,
        info: ptr::null_mut(),
        retain: ptr::null(),
        release: ptr::null(),
        copy_description: ptr::null(),
    };
    let cf_mach_port = unsafe {
        CFMachPortCreateWithPort(
            ptr::null(),
            port,
            mach_message_callback,
            &mut context,
            ptr::null_mut(),
        )
    };
    if cf_mach_port.is_null() {
        return false;
    }

    let source = unsafe { CFMachPortCreateRunLoopSource(ptr::null(), cf_mach_port, 0) };
    if source.is_null() {
        unsafe {
            CFRelease(cf_mach_port);
        }
        return false;
    }

    unsafe {
        CFRunLoopAddSource(CFRunLoopGetMain(), source, kCFRunLoopDefaultMode);
        CFRelease(source);
        CFRelease(cf_mach_port);
    }

    server.port = port;
    server.is_running = true;
    true
}

fn register_port(port: MachPort) -> bool {
    let task = unsafe { mach_task_self() };
    let mut bs_port = 0;
    if unsafe { task_get_special_port(task, TASK_BOOTSTRAP_PORT, &mut bs_port) } != KERN_SUCCESS {
        return false;
    }

    let Ok(name) = CString::new(BS_NAME) else {
        return false;
    };
    (unsafe { bootstrap_register(bs_port, name.as_ptr(), port) }) == KERN_SUCCESS
}

unsafe extern "C" fn mach_message_callback(
    _port: crate::sys::cf::CFMachPortRef,
    data: *mut std::ffi::c_void,
    _size: isize,
    _context: *mut std::ffi::c_void,
) {
    let message = data.cast::<MachMessage>();
    if message.is_null() {
        return;
    }
    let descriptor = unsafe { &(*message).descriptor };
    crate::app::handle_ipc_message(descriptor.address.cast::<u8>(), descriptor.size as usize);
    unsafe {
        crate::sys::mach::mach_msg_destroy(&mut (*message).header);
    }
}
