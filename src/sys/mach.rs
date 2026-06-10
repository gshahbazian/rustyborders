#![allow(dead_code)]

use std::os::raw::{c_char, c_int, c_uint, c_void};

pub type MachPort = c_uint;
pub type KernReturn = c_int;
pub type MachMsgBits = c_uint;
pub type MachMsgSize = c_uint;
pub type MachMsgId = c_int;
pub type IpcSpace = MachPort;
pub type MachMsgTypeName = c_uint;
pub type MachMsgCopyOptions = c_uint;

pub const KERN_SUCCESS: KernReturn = 0;
pub const MACH_PORT_NULL: MachPort = 0;
pub const TASK_BOOTSTRAP_PORT: c_int = 4;
pub const MACH_PORT_RIGHT_RECEIVE: c_int = 1;
pub const MACH_MSG_TYPE_MAKE_SEND: MachMsgTypeName = 20;
pub const MACH_MSG_TYPE_COPY_SEND: MachMsgTypeName = 19;
pub const MACH_MSG_TYPE_MAKE_SEND_ONCE: MachMsgTypeName = 21;
pub const MACH_MSGH_BITS_REMOTE_MASK: MachMsgBits = 0x0000_00ff;
pub const MACH_MSGH_BITS_COMPLEX: MachMsgBits = 0x8000_0000;
pub const MACH_SEND_MSG: c_int = 0x0000_0001;
pub const MACH_RCV_MSG: c_int = 0x0000_0002;
pub const MACH_SEND_SYNC_OVERRIDE: c_int = 0x0000_0040;
pub const MACH_SEND_PROPAGATE_QOS: c_int = 0x0000_1000;
pub const MACH_RCV_SYNC_WAIT: c_int = 0x0000_0400;
pub const MACH_MSG_TIMEOUT_NONE: c_uint = 0;
pub const MACH_PORT_QLIMIT_LARGE: c_uint = 1024;
pub const MACH_PORT_LIMITS_INFO: c_int = 1;
pub const MACH_PORT_LIMITS_INFO_COUNT: c_uint = 1;
pub const MACH_MSG_OOL_DESCRIPTOR: u8 = 0x01;
pub const MACH_MSG_VIRTUAL_COPY: MachMsgCopyOptions = 1;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct MachMsgHeader {
    pub msgh_bits: MachMsgBits,
    pub msgh_size: MachMsgSize,
    pub msgh_remote_port: MachPort,
    pub msgh_local_port: MachPort,
    pub msgh_voucher_port: MachPort,
    pub msgh_id: MachMsgId,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct MachMsgOolDescriptor {
    pub address: *mut c_void,
    pub deallocate: bool,
    pub copy: MachMsgCopyOptions,
    pub size: MachMsgSize,
    pub descriptor_type: u8,
}

impl Default for MachMsgOolDescriptor {
    fn default() -> Self {
        Self {
            address: std::ptr::null_mut(),
            deallocate: false,
            copy: 0,
            size: 0,
            descriptor_type: 0,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct MachPortLimits {
    pub mpl_qlimit: c_uint,
}

pub type MachPortInfo = *mut c_int;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct NdrRecord {
    pub mig_vers: u8,
    pub if_vers: u8,
    pub reserved1: u8,
    pub mig_encoding: u8,
    pub int_rep: u8,
    pub char_rep: u8,
    pub float_rep: u8,
    pub reserved2: u8,
}

pub const NDR_RECORD: NdrRecord = NdrRecord {
    mig_vers: 0,
    if_vers: 0,
    reserved1: 0,
    mig_encoding: 0,
    int_rep: 1,
    char_rep: 0,
    float_rep: 0,
    reserved2: 0,
};

pub const fn mach_msg_bits(
    remote: MachMsgBits,
    local: MachMsgBits,
    voucher: MachMsgBits,
    other: MachMsgBits,
) -> MachMsgBits {
    remote | (local << 8) | (voucher << 16) | other
}

#[link(name = "System", kind = "dylib")]
unsafe extern "C" {
    pub fn mach_task_self() -> MachPort;
    pub fn pid_for_task(task: MachPort, pid: *mut libc::pid_t) -> KernReturn;
    pub fn task_get_special_port(
        task: MachPort,
        which_port: c_int,
        special_port: *mut MachPort,
    ) -> KernReturn;
    pub fn mach_port_allocate(task: IpcSpace, right: c_int, name: *mut MachPort) -> KernReturn;
    pub fn mach_port_set_attributes(
        task: IpcSpace,
        name: MachPort,
        flavor: c_int,
        port_info: MachPortInfo,
        count: c_uint,
    ) -> KernReturn;
    pub fn mach_port_insert_right(
        task: IpcSpace,
        name: MachPort,
        poly: MachPort,
        poly_poly: MachMsgTypeName,
    ) -> KernReturn;
    pub fn mach_msg(
        msg: *mut MachMsgHeader,
        option: c_int,
        send_size: MachMsgSize,
        rcv_size: MachMsgSize,
        rcv_name: MachPort,
        timeout: c_uint,
        notify: MachPort,
    ) -> KernReturn;
    pub fn mach_msg_destroy(msg: *mut MachMsgHeader);
    pub fn bootstrap_look_up(
        bp: MachPort,
        service_name: *const c_char,
        sp: *mut MachPort,
    ) -> KernReturn;
    pub fn bootstrap_register(
        bp: MachPort,
        service_name: *const c_char,
        sp: MachPort,
    ) -> KernReturn;
}

#[link(name = "System", kind = "dylib")]
unsafe extern "C" {
    pub fn mig_get_special_reply_port() -> MachPort;
    pub fn mig_dealloc_special_reply_port(port: MachPort) -> MachPort;
}
