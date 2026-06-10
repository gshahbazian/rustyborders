#![allow(dead_code, non_upper_case_globals)]

use std::ffi::CString;
use std::marker::PhantomData;
use std::os::raw::{c_char, c_void};
use std::ptr;

use crate::sys::mach::MachPort;

pub type Boolean = u8;
pub type CFIndex = isize;
pub type CFTypeRef = *const c_void;
pub type CFAllocatorRef = *const c_void;
pub type CFArrayRef = *const c_void;
pub type CFDictionaryRef = *const c_void;
pub type CFNumberRef = *const c_void;
pub type CFStringRef = *const c_void;
pub type CFRunLoopRef = *const c_void;
pub type CFRunLoopSourceRef = *const c_void;
pub type CFMachPortRef = *const c_void;
pub type CFUUIDRef = *const c_void;

pub const K_CF_NUMBER_SINT32_TYPE: i32 = 3;
pub const K_CF_NUMBER_SINT64_TYPE: i32 = 4;
pub const K_CF_NUMBER_CFINDEX_TYPE: i32 = 14;
pub const K_CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;

#[repr(C)]
pub struct CFArrayCallBacks {
    pub version: CFIndex,
    pub retain: *const c_void,
    pub release: *const c_void,
    pub copy_description: *const c_void,
    pub equal: *const c_void,
}

#[repr(C)]
pub struct CFDictionaryKeyCallBacks {
    pub version: CFIndex,
    pub retain: *const c_void,
    pub release: *const c_void,
    pub copy_description: *const c_void,
    pub equal: *const c_void,
    pub hash: *const c_void,
}

#[repr(C)]
pub struct CFDictionaryValueCallBacks {
    pub version: CFIndex,
    pub retain: *const c_void,
    pub release: *const c_void,
    pub copy_description: *const c_void,
    pub equal: *const c_void,
}

#[repr(C)]
pub struct CFMachPortContext {
    pub version: CFIndex,
    pub info: *mut c_void,
    pub retain: *const c_void,
    pub release: *const c_void,
    pub copy_description: *const c_void,
}

pub type CFMachPortCallBack =
    unsafe extern "C" fn(CFMachPortRef, *mut c_void, CFIndex, *mut c_void);

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    pub static kCFTypeArrayCallBacks: CFArrayCallBacks;
    pub static kCFTypeDictionaryKeyCallBacks: CFDictionaryKeyCallBacks;
    pub static kCFTypeDictionaryValueCallBacks: CFDictionaryValueCallBacks;
    pub static kCFBooleanTrue: CFTypeRef;
    pub static kCFRunLoopDefaultMode: CFStringRef;

    pub fn CFRelease(cf: CFTypeRef);
    pub fn CFRetain(cf: CFTypeRef) -> CFTypeRef;
    pub fn CFArrayCreate(
        allocator: CFAllocatorRef,
        values: *const *const c_void,
        num_values: CFIndex,
        callbacks: *const CFArrayCallBacks,
    ) -> CFArrayRef;
    pub fn CFArrayGetCount(array: CFArrayRef) -> CFIndex;
    pub fn CFArrayGetValueAtIndex(array: CFArrayRef, index: CFIndex) -> *const c_void;
    pub fn CFNumberCreate(
        allocator: CFAllocatorRef,
        the_type: i32,
        value_ptr: *const c_void,
    ) -> CFNumberRef;
    pub fn CFNumberGetType(number: CFNumberRef) -> i32;
    pub fn CFNumberGetValue(number: CFNumberRef, the_type: i32, value_ptr: *mut c_void) -> Boolean;
    pub fn CFStringCreateWithCString(
        allocator: CFAllocatorRef,
        c_str: *const c_char,
        encoding: u32,
    ) -> CFStringRef;
    pub fn CFUUIDCreateString(allocator: CFAllocatorRef, uuid: CFUUIDRef) -> CFStringRef;
    pub fn CFDictionaryCreate(
        allocator: CFAllocatorRef,
        keys: *const *const c_void,
        values: *const *const c_void,
        num_values: CFIndex,
        key_callbacks: *const CFDictionaryKeyCallBacks,
        value_callbacks: *const CFDictionaryValueCallBacks,
    ) -> CFDictionaryRef;
    pub fn CFDictionaryGetValue(dictionary: CFDictionaryRef, key: *const c_void) -> *const c_void;
    pub fn CFMachPortCreateWithPort(
        allocator: CFAllocatorRef,
        port_num: MachPort,
        callout: CFMachPortCallBack,
        context: *mut CFMachPortContext,
        should_free_info: *mut Boolean,
    ) -> CFMachPortRef;
    pub fn CFMachPortCreateRunLoopSource(
        allocator: CFAllocatorRef,
        port: CFMachPortRef,
        order: CFIndex,
    ) -> CFRunLoopSourceRef;
    pub fn CFRunLoopGetCurrent() -> CFRunLoopRef;
    pub fn CFRunLoopGetMain() -> CFRunLoopRef;
    pub fn CFRunLoopAddSource(rl: CFRunLoopRef, source: CFRunLoopSourceRef, mode: CFStringRef);
    pub fn CFRunLoopRun();
}

pub struct OwnedCf<T: Copy + Into<CFTypeRef>> {
    raw: T,
    _marker: PhantomData<T>,
}

impl<T> OwnedCf<T>
where
    T: Copy + Into<CFTypeRef>,
{
    pub unsafe fn from_create_rule(raw: T) -> Option<Self> {
        if raw.into().is_null() {
            None
        } else {
            Some(Self {
                raw,
                _marker: PhantomData,
            })
        }
    }

    pub fn as_raw(&self) -> T {
        self.raw
    }

    pub fn into_raw(self) -> T {
        let raw = self.raw;
        std::mem::forget(self);
        raw
    }
}

impl<T> Drop for OwnedCf<T>
where
    T: Copy + Into<CFTypeRef>,
{
    fn drop(&mut self) {
        unsafe {
            CFRelease(self.raw.into());
        }
    }
}

pub fn cf_string(value: &str) -> Option<OwnedCf<CFStringRef>> {
    let c_string = CString::new(value).ok()?;
    unsafe {
        OwnedCf::from_create_rule(CFStringCreateWithCString(
            ptr::null(),
            c_string.as_ptr(),
            K_CF_STRING_ENCODING_UTF8,
        ))
    }
}

pub fn cfarray_of_u32(values: &[u32]) -> Option<OwnedCf<CFArrayRef>> {
    cfarray_of_numbers(values, K_CF_NUMBER_SINT32_TYPE)
}

pub fn cfarray_of_u64(values: &[u64]) -> Option<OwnedCf<CFArrayRef>> {
    cfarray_of_numbers(values, K_CF_NUMBER_SINT64_TYPE)
}

fn cfarray_of_numbers<T>(values: &[T], number_type: i32) -> Option<OwnedCf<CFArrayRef>> {
    let numbers = values
        .iter()
        .map(|value| unsafe {
            OwnedCf::from_create_rule(CFNumberCreate(
                ptr::null(),
                number_type,
                std::ptr::from_ref(value).cast(),
            ))
        })
        .collect::<Option<Vec<_>>>()?;

    let raw_values = numbers
        .iter()
        .map(|number| number.as_raw().cast::<c_void>())
        .collect::<Vec<_>>();

    unsafe {
        OwnedCf::from_create_rule(CFArrayCreate(
            ptr::null(),
            raw_values.as_ptr(),
            raw_values.len() as CFIndex,
            &raw const kCFTypeArrayCallBacks,
        ))
    }
}
