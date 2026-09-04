use std::{
    ffi::{CStr, CString},
    path::PathBuf,
};

#[cfg(target_os = "macos")]
extern "C" {
    fn oe_bookmark_create(path: *const std::os::raw::c_char) -> *mut std::os::raw::c_char;
    fn oe_bookmark_resolve(encoded: *const std::os::raw::c_char) -> *mut std::os::raw::c_char;
    fn oe_string_free(value: *mut std::os::raw::c_char);
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
