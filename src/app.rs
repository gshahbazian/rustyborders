use std::collections::HashMap;
use std::ffi::CString;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Mutex, OnceLock};

use libc::{RTLD_LAZY, RTLD_LOCAL, dlopen, dlsym};
use thiserror::Error;

use crate::border::Border;
use crate::events;
use crate::ipc::{self, MachServer};
use crate::parser::{ParseError, parse_settings};
use crate::settings::{Settings, UpdateMask};
use crate::sys::cf::{
    CFArrayGetCount, CFArrayGetValueAtIndex, CFNumberGetValue, CFRunLoopRun, CFTypeRef,
    K_CF_NUMBER_SINT32_TYPE,
};
use crate::sys::geometry::{SpaceId, WindowId};
use crate::sys::mach::{MachPort, mach_task_self, pid_for_task};
use crate::sys::skylight::{CornerRadiiFn, SLSMainConnectionID};
use crate::windows;

const VERSION: &str = "rustyborders-v0.1.0";
const SKYLIGHT_PATH: &str = "/System/Library/PrivateFrameworks/SkyLight.framework/SkyLight";
const CORNER_RADII_SYMBOL: &str = "SLSWindowIteratorGetCornerRadii";

static APP: OnceLock<Mutex<App>> = OnceLock::new();
static CORNER_RADII_FN: OnceLock<Option<CornerRadiiFn>> = OnceLock::new();

#[derive(Debug, Error)]
pub enum AppError {
    #[error("{0}")]
    Parse(#[from] ParseError),
    #[error("a rustyborders instance is already running; provide valid arguments to update it")]
    AlreadyRunning,
    #[error("failed to initialize Mach server")]
    MachServer,
    #[error("application state is already initialized")]
    AlreadyInitialized,
}

pub struct App {
    pid: libc::pid_t,
    settings: Settings,
    windows: HashMap<WindowId, Border>,
    mach_server: MachServer,
    server_port: MachPort,
}

impl App {
    fn new(pid: libc::pid_t, settings: Settings, server_port: MachPort) -> Self {
        Self {
            pid,
            settings,
            windows: HashMap::new(),
            mach_server: MachServer::default(),
            server_port,
        }
    }

    fn handle_message(&mut self, data: *const u8, len: usize) {
        let arguments = ipc::decode_message(data, len);
        if arguments.is_empty() {
            return;
        }

        let Ok((mut settings, update_mask)) =
            ipc::parse_message_settings(&self.settings, &arguments)
        else {
            return;
        };
        if settings.ax_focus && !self.settings.ax_focus {
            settings.ax_focus = windows::ax_check_trust(true);
        }

        if let Some(wid) = settings.apply_to {
            if let Some(border) = self.windows.get_mut(&wid) {
                border.set_override(settings);
                border.update(&self.settings, self.server_port);
            }
            return;
        }

        self.settings = settings;
        for border in self.windows.values_mut() {
            if let Some(override_settings) = border.setting_override.as_mut() {
                let _ = parse_settings(override_settings, &arguments);
                if !update_mask.intersects(UpdateMask::ALL | UpdateMask::RECREATE_ALL) {
                    border.needs_redraw = true;
                    border.update(&self.settings, self.server_port);
                }
            }
        }

        self.apply_update_mask(update_mask);
    }

    fn apply_update_mask(&mut self, update_mask: UpdateMask) {
        if update_mask.contains(UpdateMask::RECREATE_ALL) {
            self.recreate_all_borders();
        } else if update_mask.intersects(UpdateMask::ALL) {
            self.update_all();
        } else if update_mask.contains(UpdateMask::ACTIVE) {
            self.update_active();
        } else if update_mask.contains(UpdateMask::INACTIVE) {
            self.update_inactive();
        }
    }

    fn recreate_all_borders(&mut self) {
        let overrides = self
            .windows
            .iter()
            .filter_map(|(wid, border)| {
                border
                    .setting_override
                    .clone()
                    .map(|settings| (*wid, settings))
            })
            .collect::<HashMap<_, _>>();
        for border in self.windows.values_mut() {
            border.destroy();
        }
        self.windows.clear();
        windows::add_existing_windows(
            &mut self.windows,
            self.pid,
            &self.settings,
            self.server_port,
        );
        for (wid, settings) in overrides {
            if let Some(border) = self.windows.get_mut(&wid) {
                border.set_override(settings);
                border.update(&self.settings, self.server_port);
            }
        }
        windows::update_notifications(&self.windows);
    }

    fn update_all(&mut self) {
        for border in self.windows.values_mut() {
            border.needs_redraw = true;
            border.update(&self.settings, self.server_port);
        }
    }

    fn update_active(&mut self) {
        for border in self.windows.values_mut().filter(|border| border.focused) {
            border.needs_redraw = true;
            border.update(&self.settings, self.server_port);
        }
    }

    fn update_inactive(&mut self) {
        for border in self.windows.values_mut().filter(|border| !border.focused) {
            border.needs_redraw = true;
            border.update(&self.settings, self.server_port);
        }
    }

    fn focus_window(&mut self, wid: WindowId) -> bool {
        let mut found = false;
        for border in self.windows.values_mut() {
            if border.focused && border.target_wid != wid {
                border.focused = false;
                border.needs_redraw = true;
                border.update(&self.settings, self.server_port);
            }

            if !border.focused && border.target_wid == wid {
                border.focused = true;
                border.needs_redraw = true;
                border.update(&self.settings, self.server_port);
            }

            if border.target_wid == wid {
                found = true;
            }
        }
        found
    }

    fn determine_and_focus_active_window(&mut self) {
        let cid = unsafe { SLSMainConnectionID() };
        let front_wid = if self.settings.ax_focus {
            windows::ax_get_front_window(cid)
        } else {
            windows::get_front_window(cid)
        };

        if !self.focus_window(front_wid) && front_wid.0 != 0 {
            let sid = windows::window_space_id(cid, front_wid);
            if windows::create_window_border(
                &mut self.windows,
                self.pid,
                &self.settings,
                self.server_port,
                front_wid,
                sid,
            ) {
                windows::update_notifications(&self.windows);
                self.focus_window(front_wid);
            }
        }
    }

    fn draw_borders_on_current_spaces(&mut self) {
        let cid = unsafe { SLSMainConnectionID() };
        let displays = unsafe { crate::sys::skylight::SLSCopyManagedDisplays(cid) };
        let Some(displays) = (unsafe { crate::sys::cf::OwnedCf::from_create_rule(displays) })
        else {
            return;
        };

        let count = unsafe { CFArrayGetCount(displays.as_raw()) };
        let mut spaces = Vec::with_capacity(count as usize);
        for index in 0..count {
            let display = unsafe { CFArrayGetValueAtIndex(displays.as_raw(), index) };
            let sid = unsafe {
                crate::sys::skylight::SLSManagedDisplayGetCurrentSpace(cid, display.cast())
            };
            if sid != 0 {
                spaces.push(sid);
            }
        }

        let Some(space_list) = crate::sys::cf::cfarray_of_u64(&spaces) else {
            return;
        };
        let mut set_tags = 1_u64;
        let mut clear_tags = 0_u64;
        let window_list = unsafe {
            crate::sys::skylight::SLSCopyWindowsWithOptionsAndTags(
                cid,
                0,
                space_list.as_raw(),
                0x2,
                &mut set_tags,
                &mut clear_tags,
            )
        };
        let Some(window_list) = (unsafe { crate::sys::cf::OwnedCf::from_create_rule(window_list) })
        else {
            return;
        };

        let query =
            unsafe { crate::sys::skylight::SLSWindowQueryWindows(cid, window_list.as_raw(), 0) };
        let Some(query) = (unsafe { crate::sys::cf::OwnedCf::from_create_rule(query) }) else {
            return;
        };
        let iterator =
            unsafe { crate::sys::skylight::SLSWindowQueryResultCopyWindows(query.as_raw()) };
        let Some(iterator) = (unsafe { crate::sys::cf::OwnedCf::from_create_rule(iterator) })
        else {
            return;
        };

        let mut created = false;
        while unsafe { crate::sys::skylight::SLSWindowIteratorAdvance(iterator.as_raw()) } {
            if !windows::window_suitable(iterator.as_raw()) {
                continue;
            }
            let wid = WindowId(unsafe {
                crate::sys::skylight::SLSWindowIteratorGetWindowID(iterator.as_raw())
            });
            if let Some(border) = self.windows.get_mut(&wid) {
                border.update(&self.settings, self.server_port);
            } else {
                created |= windows::create_window_border(
                    &mut self.windows,
                    self.pid,
                    &self.settings,
                    self.server_port,
                    wid,
                    windows::window_space_id(cid, wid),
                );
            }
        }
        if created {
            windows::update_notifications(&self.windows);
        }
    }
}

pub fn run(arguments: Vec<String>) -> Result<(), AppError> {
    crate::rb_log!("starting with args: {:?}", &arguments[1..]);
    if arguments
        .get(1)
        .is_some_and(|arg| arg == "--version" || arg == "-v")
    {
        println!("{VERSION}");
        return Ok(());
    }

    if arguments
        .get(1)
        .is_some_and(|arg| arg == "--help" || arg == "-h")
    {
        println!("Refer to the man page for help: man borders");
        return Ok(());
    }

    let mut settings = Settings {
        ax_focus: windows::ax_check_trust(false),
        ..Settings::default()
    };
    let cli_arguments = arguments.into_iter().skip(1).collect::<Vec<_>>();
    let update_mask = parse_settings(&mut settings, &cli_arguments)?;
    if settings.ax_focus {
        settings.ax_focus = windows::ax_check_trust(true);
    }
    crate::rb_log!(
        "parsed settings: width={} active={} inactive={} hidpi={} order={} style={} ax_focus={} update_mask={:?}",
        settings.border_width,
        settings.active_window,
        settings.inactive_window,
        settings.hidpi,
        settings.border_order,
        settings.border_style,
        settings.ax_focus,
        update_mask
    );

    let existing_server_port = ipc::lookup_server_port();
    crate::rb_log!("existing mach server port: {existing_server_port}");
    if existing_server_port != 0 && !update_mask.is_empty() {
        crate::rb_log!("sending args to existing instance");
        ipc::send_args_to_server(existing_server_port, &cli_arguments);
        return Ok(());
    }
    if existing_server_port != 0 {
        return Err(AppError::AlreadyRunning);
    }

    load_symbols();

    let mut pid = 0;
    unsafe {
        pid_for_task(mach_task_self(), &mut pid);
    }

    let server_port = windows::create_connection_server_port();
    crate::rb_log!("connection server port: {server_port}");
    let mut app = App::new(pid, settings, server_port);
    if !ipc::begin_server(&mut app.mach_server) {
        return Err(AppError::MachServer);
    }

    let cid = unsafe { SLSMainConnectionID() };
    crate::rb_log!("main connection id: {cid}; pid: {pid}");
    events::register(cid);
    events::register_event_port(cid);
    windows::add_existing_windows(&mut app.windows, app.pid, &app.settings, app.server_port);
    app.determine_and_focus_active_window();
    crate::rb_log!("initial border count: {}", app.windows.len());

    APP.set(Mutex::new(app))
        .map_err(|_| AppError::AlreadyInitialized)?;

    if update_mask.is_empty() {
        execute_config_file("borders", "bordersrc");
    }

    unsafe {
        CFRunLoopRun();
    }
    Ok(())
}

pub fn handle_ipc_message(data: *const u8, len: usize) {
    with_app(|app| app.handle_message(data, len));
}

pub fn handle_window_create(wid: WindowId, sid: SpaceId, cid: i32) {
    with_app(|app| {
        if windows::is_own_window(app.pid, cid, wid) {
            return;
        }
        if windows::create_window_border(
            &mut app.windows,
            app.pid,
            &app.settings,
            app.server_port,
            wid,
            sid,
        ) {
            windows::update_notifications(&app.windows);
            app.determine_and_focus_active_window();
        }
    });
}

pub fn handle_window_destroy(wid: WindowId, sid: SpaceId) {
    with_app(|app| {
        if let Some(border) = app.windows.get(&wid)
            && (border.sid == sid || border.sticky || sid.0 == 0)
        {
            let mut border = app.windows.remove(&wid).expect("window existed");
            border.destroy();
            windows::update_notifications(&app.windows);
        }
    });
}

pub fn handle_window_update(wid: WindowId, cid: i32) {
    with_app(|app| {
        if windows::is_own_window(app.pid, cid, wid) {
            return;
        }
        if let Some(border) = app.windows.get_mut(&wid) {
            border.update(&app.settings, app.server_port);
        }
    });
}

pub fn handle_window_move(wid: WindowId, cid: i32) {
    with_app(|app| {
        if windows::is_own_window(app.pid, cid, wid) {
            return;
        }
        if let Some(border) = app.windows.get_mut(&wid) {
            border.move_border(&app.settings, app.server_port);
        }
    });
}

pub fn handle_window_hide(wid: WindowId, cid: i32) {
    with_app(|app| {
        if windows::is_own_window(app.pid, cid, wid) {
            return;
        }
        if let Some(border) = app.windows.get_mut(&wid) {
            border.hide();
        }
    });
}

pub fn handle_window_unhide(wid: WindowId, cid: i32) {
    with_app(|app| {
        if windows::is_own_window(app.pid, cid, wid) {
            return;
        }
        if let Some(border) = app.windows.get_mut(&wid) {
            border.unhide(&app.settings);
        }
    });
}

pub fn handle_window_close(wid: WindowId) {
    with_app(|app| {
        if let Some(mut border) = app.windows.remove(&wid) {
            border.destroy();
            windows::update_notifications(&app.windows);
        }
    });
}

pub fn determine_and_focus_active_window() {
    with_app(App::determine_and_focus_active_window);
}

pub fn draw_borders_on_current_spaces() {
    with_app(App::draw_borders_on_current_spaces);
}

pub fn corner_radius_for_iterator(iterator: CFTypeRef) -> Option<f64> {
    let function = CORNER_RADII_FN.get().copied().flatten()?;
    let radii = unsafe { function(iterator) };
    let radii = unsafe { crate::sys::cf::OwnedCf::from_create_rule(radii) }?;
    if unsafe { CFArrayGetCount(radii.as_raw()) } <= 0 {
        return None;
    }
    let value = unsafe { CFArrayGetValueAtIndex(radii.as_raw(), 0) };
    let mut radius = 0_i32;
    unsafe {
        CFNumberGetValue(
            value,
            K_CF_NUMBER_SINT32_TYPE,
            (&mut radius as *mut i32).cast(),
        );
    }
    (radius > 0).then_some(f64::from(radius))
}

fn with_app(function: impl FnOnce(&mut App)) {
    let Some(app) = APP.get() else {
        return;
    };
    match app.lock() {
        Ok(mut app) => function(&mut app),
        Err(poisoned) => {
            crate::rb_log!("application state lock was poisoned; recovering");
            let mut app = poisoned.into_inner();
            function(&mut app);
        }
    };
}

fn load_symbols() {
    let path = CString::new(SKYLIGHT_PATH).expect("static path has no nul");
    let symbol = CString::new(CORNER_RADII_SYMBOL).expect("static symbol has no nul");
    let function = unsafe {
        let lib = dlopen(path.as_ptr(), RTLD_LAZY | RTLD_LOCAL);
        if lib.is_null() {
            None
        } else {
            let symbol = dlsym(lib, symbol.as_ptr());
            if symbol.is_null() {
                None
            } else {
                Some(std::mem::transmute::<*mut std::ffi::c_void, CornerRadiiFn>(
                    symbol,
                ))
            }
        }
    };
    let _ = CORNER_RADII_FN.set(function);
}

fn execute_config_file(name: &str, filename: &str) {
    let Some(home) = std::env::var_os("HOME") else {
        return;
    };

    let primary = PathBuf::from(&home)
        .join(".config")
        .join(name)
        .join(filename);
    let legacy = PathBuf::from(&home).join(format!(".{filename}"));
    let path = if primary.is_file() {
        primary
    } else if legacy.is_file() {
        legacy
    } else {
        return;
    };

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(metadata) = path.metadata() {
            let mut permissions = metadata.permissions();
            permissions.set_mode(permissions.mode() | 0o100);
            let _ = std::fs::set_permissions(&path, permissions);
        }
    }

    let _ = Command::new("/usr/bin/env")
        .arg("sh")
        .arg("-c")
        .arg(path)
        .spawn();
}
