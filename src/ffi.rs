//! Opaque, caller-owned handles. Calls on one handle must be serialized.
use crate::Engine;
use std::ffi::{c_char, CStr, CString};
#[no_mangle]
pub extern "C" fn viet_engine_new(flags: u32) -> *mut Engine {
    Box::into_raw(Box::new(Engine::new(
        crate::rules::InputMethod::telex(),
        flags,
    )))
}
/// # Safety
/// `engine` must be null or a live handle allocated by viet_engine_new, freed once.
#[no_mangle]
pub unsafe extern "C" fn viet_engine_free(engine: *mut Engine) {
    if !engine.is_null() {
        drop(unsafe { Box::from_raw(engine) });
    }
}
/// # Safety
/// Handle must be live and exclusively borrowed for the duration of this call.
#[no_mangle]
pub unsafe extern "C" fn viet_engine_key(engine: *mut Engine, scalar: u32) -> bool {
    let Some(c) = char::from_u32(scalar).filter(|c| *c != '\0') else {
        return false;
    };
    let Some(engine) = (unsafe { engine.as_mut() }) else {
        return false;
    };
    engine.process_key(c);
    true
}
/// # Safety
/// Handle must be live and exclusively borrowed. Text must be a valid NUL-terminated string.
#[no_mangle]
pub unsafe extern "C" fn viet_engine_process(engine: *mut Engine, text: *const c_char) -> bool {
    if text.is_null() {
        return false;
    }
    let Ok(text) = (unsafe { CStr::from_ptr(text) }).to_str() else {
        return false;
    };
    let Some(engine) = (unsafe { engine.as_mut() }) else {
        return false;
    };
    engine.process(text);
    true
}
/// Returns an owned UTF-8 string; release with viet_string_free.
/// # Safety
/// Handle must be live and readable throughout this call.
#[no_mangle]
pub unsafe extern "C" fn viet_engine_text(engine: *const Engine) -> *mut c_char {
    let Some(engine) = (unsafe { engine.as_ref() }) else {
        return std::ptr::null_mut();
    };
    CString::new(engine.text()).map_or(std::ptr::null_mut(), CString::into_raw)
}
/// # Safety
/// Text must be null or a string returned by viet_engine_text, freed once.
#[no_mangle]
pub unsafe extern "C" fn viet_string_free(text: *mut c_char) {
    if !text.is_null() {
        drop(unsafe { CString::from_raw(text) });
    }
}
/// # Safety
/// Handle must be live and exclusively borrowed.
#[no_mangle]
pub unsafe extern "C" fn viet_engine_reset(engine: *mut Engine) {
    if let Some(e) = unsafe { engine.as_mut() } {
        e.reset();
    }
}
/// # Safety
/// Handle must be live and exclusively borrowed.
#[no_mangle]
pub unsafe extern "C" fn viet_engine_backspace(engine: *mut Engine) {
    if let Some(e) = unsafe { engine.as_mut() } {
        e.backspace(true);
    }
}
