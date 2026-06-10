use std::ffi::CStr;
use std::os::raw::{c_char, c_int};

const LC_SEGMENT_64: u32 = 0x19;
const LC_SYMTAB: u32 = 0x2;
const SEG_LINKEDIT: &[u8] = b"__LINKEDIT";

#[repr(C)]
struct MachHeader64 {
    magic: u32,
    cputype: i32,
    cpusubtype: i32,
    filetype: u32,
    ncmds: u32,
    sizeofcmds: u32,
    flags: u32,
    reserved: u32,
}

#[repr(C)]
struct LoadCommand {
    cmd: u32,
    cmdsize: u32,
}

#[repr(C)]
struct SegmentCommand64 {
    cmd: u32,
    cmdsize: u32,
    segname: [c_char; 16],
    vmaddr: u64,
    vmsize: u64,
    fileoff: u64,
    filesize: u64,
    maxprot: i32,
    initprot: i32,
    nsects: u32,
    flags: u32,
}

#[repr(C)]
struct SymtabCommand {
    cmd: u32,
    cmdsize: u32,
    symoff: u32,
    nsyms: u32,
    stroff: u32,
    strsize: u32,
}

#[repr(C)]
struct Nlist64 {
    n_strx: u32,
    n_type: u8,
    n_sect: u8,
    n_desc: u16,
    n_value: u64,
}

#[link(name = "System", kind = "dylib")]
unsafe extern "C" {
    fn _dyld_image_count() -> u32;
    fn _dyld_get_image_name(image_index: u32) -> *const c_char;
    fn _dyld_get_image_header(image_index: u32) -> *const MachHeader64;
    fn _dyld_get_image_vmaddr_slide(image_index: u32) -> isize;
}

pub unsafe fn find_symbol(
    target_image: &str,
    target_symbol: &str,
) -> Option<*mut std::ffi::c_void> {
    let image_count = unsafe { _dyld_image_count() };

    for image_index in 0..image_count {
        let image_name = unsafe { _dyld_get_image_name(image_index) };
        if image_name.is_null() {
            continue;
        }

        let image_name = unsafe { CStr::from_ptr(image_name) }.to_string_lossy();
        if image_name != target_image {
            continue;
        }

        let slide = unsafe { _dyld_get_image_vmaddr_slide(image_index) } as u64;
        let header = unsafe { _dyld_get_image_header(image_index) };
        if header.is_null() {
            return None;
        }

        return unsafe { find_symbol_in_image(header, slide, target_symbol) };
    }

    None
}

unsafe fn find_symbol_in_image(
    header: *const MachHeader64,
    slide: u64,
    target_symbol: &str,
) -> Option<*mut std::ffi::c_void> {
    let mut offset = std::mem::size_of::<MachHeader64>();
    let mut linkedit: Option<&SegmentCommand64> = None;
    let mut symtab: Option<&SymtabCommand> = None;

    for _ in 0..unsafe { (*header).ncmds } {
        let command = unsafe { (header.cast::<u8>().add(offset)).cast::<LoadCommand>() };
        match unsafe { (*command).cmd } {
            LC_SEGMENT_64 => {
                let segment = unsafe { &*command.cast::<SegmentCommand64>() };
                let nul = segment
                    .segname
                    .iter()
                    .position(|byte| *byte == 0)
                    .unwrap_or(segment.segname.len());
                let name = segment.segname[..nul]
                    .iter()
                    .map(|byte| *byte as u8)
                    .collect::<Vec<_>>();
                if name == SEG_LINKEDIT {
                    linkedit = Some(segment);
                }
            }
            LC_SYMTAB => {
                symtab = Some(unsafe { &*command.cast::<SymtabCommand>() });
            }
            _ => {}
        }
        offset += unsafe { (*command).cmdsize } as usize;
    }

    let linkedit = linkedit?;
    let symtab = symtab?;
    let linkedit_base = linkedit.vmaddr - linkedit.fileoff + slide;
    let string_table = (linkedit_base + u64::from(symtab.stroff)) as *const c_char;
    let symbol_table = (linkedit_base + u64::from(symtab.symoff)) as *const Nlist64;

    for index in 0..symtab.nsyms {
        let list = unsafe { symbol_table.add(index as usize) };
        let symbol_name = unsafe { string_table.add((*list).n_strx as usize) };
        if symbol_name.is_null() {
            continue;
        }
        let symbol_name = unsafe { CStr::from_ptr(symbol_name) }.to_string_lossy();
        if symbol_name == target_symbol {
            return Some((unsafe { (*list).n_value } + slide) as *mut std::ffi::c_void);
        }
    }

    None
}

#[allow(dead_code)]
fn _assert_c_int(_: c_int) {}
