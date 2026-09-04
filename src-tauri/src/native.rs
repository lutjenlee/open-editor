use std::{
    ffi::{CStr, CString},
    path::PathBuf,
};

#[cfg(target_os = "macos")]
extern "C" {
    fn oe_player_create() -> *mut std::ffi::c_void;
    fn oe_player_release(handle: *mut std::ffi::c_void);
    fn oe_player_attach(
        handle: *mut std::ffi::c_void,
        view: *mut std::ffi::c_void,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    ) -> bool;
    fn oe_player_set_frame(
        handle: *mut std::ffi::c_void,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    ) -> bool;
    fn oe_player_load_composition(
        handle: *mut std::ffi::c_void,
        json: *const std::os::raw::c_char,
    ) -> bool;
    fn oe_player_detach(handle: *mut std::ffi::c_void);
    fn oe_player_play(handle: *mut std::ffi::c_void);
    fn oe_player_pause(handle: *mut std::ffi::c_void);
    fn oe_player_seek(handle: *mut std::ffi::c_void, value: i64, timescale: i32);
    fn oe_player_current_time(handle: *mut std::ffi::c_void, timescale: i32) -> i64;
    fn oe_player_rate(handle: *mut std::ffi::c_void) -> f64;
    fn oe_export_start(
        json: *const std::os::raw::c_char,
        output_path: *const std::os::raw::c_char,
        callback: extern "C" fn(bool, *const std::os::raw::c_char, *mut std::ffi::c_void),
        context: *mut std::ffi::c_void,
    ) -> *mut std::ffi::c_void;
    fn oe_export_cancel(handle: *mut std::ffi::c_void);
}

#[cfg(target_os = "macos")]
extern "C" {
    fn oe_bookmark_create(path: *const std::os::raw::c_char) -> *mut std::os::raw::c_char;
    fn oe_bookmark_resolve(encoded: *const std::os::raw::c_char) -> *mut std::os::raw::c_char;
    fn oe_string_free(value: *mut std::os::raw::c_char);
}

pub struct NativePlayer(usize);

impl NativePlayer {
    #[cfg(target_os = "macos")]
    pub fn new() -> Self {
        Self(unsafe { oe_player_create() } as usize)
    }

    #[cfg(not(target_os = "macos"))]
    pub fn new() -> Self {
        Self(0)
    }

    #[cfg(target_os = "macos")]
    fn pointer(&self) -> *mut std::ffi::c_void {
        self.0 as *mut std::ffi::c_void
    }

    pub fn handle(&self) -> usize {
        self.0
    }
}

#[cfg(target_os = "macos")]
pub unsafe fn player_attach(handle: usize, view: usize, frame: [f64; 4]) -> bool {
    oe_player_attach(
        handle as *mut std::ffi::c_void,
        view as *mut std::ffi::c_void,
        frame[0],
        frame[1],
        frame[2],
        frame[3],
    )
}

#[cfg(target_os = "macos")]
pub unsafe fn player_set_frame(handle: usize, frame: [f64; 4]) -> bool {
    oe_player_set_frame(
        handle as *mut std::ffi::c_void,
        frame[0],
        frame[1],
        frame[2],
        frame[3],
    )
}

#[cfg(target_os = "macos")]
pub unsafe fn player_load(handle: usize, json: &str) -> bool {
    CString::new(json)
        .map(|value| oe_player_load_composition(handle as *mut _, value.as_ptr()))
        .unwrap_or(false)
}

#[cfg(target_os = "macos")]
pub unsafe fn player_detach(handle: usize) {
    oe_player_detach(handle as *mut _)
}

#[cfg(target_os = "macos")]
pub unsafe fn player_play(handle: usize) {
    oe_player_play(handle as *mut _)
}

#[cfg(target_os = "macos")]
pub unsafe fn player_pause(handle: usize) {
    oe_player_pause(handle as *mut _)
}

#[cfg(target_os = "macos")]
pub unsafe fn player_seek(handle: usize, value: i64, timescale: i32) {
    oe_player_seek(handle as *mut _, value, timescale)
}

#[cfg(target_os = "macos")]
pub unsafe fn player_time(handle: usize, timescale: i32) -> (i64, f64) {
    (
        oe_player_current_time(handle as *mut _, timescale),
        oe_player_rate(handle as *mut _),
    )
}

#[cfg(target_os = "macos")]
pub unsafe fn start_export(
    json: &str,
    output_path: &str,
    callback: extern "C" fn(bool, *const std::os::raw::c_char, *mut std::ffi::c_void),
    context: usize,
) -> Option<usize> {
    let json = CString::new(json).ok()?;
    let output_path = CString::new(output_path).ok()?;
    let handle = oe_export_start(
        json.as_ptr(),
        output_path.as_ptr(),
        callback,
        context as *mut std::ffi::c_void,
    );
    (!handle.is_null()).then_some(handle as usize)
}

#[cfg(target_os = "macos")]
pub unsafe fn cancel_export(handle: usize) {
    oe_export_cancel(handle as *mut _)
}

impl Default for NativePlayer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(target_os = "macos")]
impl Drop for NativePlayer {
    fn drop(&mut self) {
        if self.0 != 0 {
            unsafe { oe_player_release(self.pointer()) };
        }
    }
}

#[cfg(target_os = "macos")]
fn take_string(pointer: *mut std::os::raw::c_char) -> Option<String> {
    if pointer.is_null() {
        return None;
    }
    let value = unsafe { CStr::from_ptr(pointer) }
        .to_string_lossy()
        .into_owned();
    unsafe { oe_string_free(pointer) };
    Some(value)
}

#[cfg(target_os = "macos")]
pub fn create_security_bookmark(path: &std::path::Path) -> Option<String> {
    let path = CString::new(path.to_string_lossy().as_bytes()).ok()?;
    take_string(unsafe { oe_bookmark_create(path.as_ptr()) })
}

#[cfg(not(target_os = "macos"))]
pub fn create_security_bookmark(_path: &std::path::Path) -> Option<String> {
    None
}

#[cfg(target_os = "macos")]
pub fn resolve_security_bookmark(encoded: &str) -> Option<PathBuf> {
    let encoded = CString::new(encoded).ok()?;
    take_string(unsafe { oe_bookmark_resolve(encoded.as_ptr()) }).map(PathBuf::from)
}

#[cfg(not(target_os = "macos"))]
pub fn resolve_security_bookmark(_encoded: &str) -> Option<PathBuf> {
    None
}
