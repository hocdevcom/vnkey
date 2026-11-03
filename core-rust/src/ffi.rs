//! Exactly five C entry points. Returned strings are borrowed from the handle.
use crate::engine::{Action, ActionType, VietEngine};
use std::{
    ffi::{c_char, CStr, CString},
    panic::{catch_unwind, AssertUnwindSafe},
};

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CActionResult {
    pub action_type: u32,
    pub backspace_count: u32,
    pub preedit_str: *const c_char,
    pub commit_str: *const c_char,
}
impl Default for CActionResult {
    fn default() -> Self {
        Self {
            action_type: ActionType::Ignore as u32,
            backspace_count: 0,
            preedit_str: std::ptr::null(),
            commit_str: std::ptr::null(),
        }
    }
}
impl VietEngine {
    fn store_result(&mut self, action: Action) -> CActionResult {
        self.preedit_buf = CString::new(action.preedit).unwrap_or_default();
        self.commit_buf = CString::new(action.commit).unwrap_or_default();
        CActionResult {
            action_type: action.action_type as u32,
            backspace_count: action.backspace_count,
            preedit_str: self.preedit_buf.as_ptr(),
            commit_str: self.commit_buf.as_ptr(),
        }
    }
}

#[no_mangle]
pub extern "C" fn viet_engine_new() -> *mut VietEngine {
    catch_unwind(|| {
        VietEngine::new()
            .map(|e| Box::into_raw(Box::new(e)))
            .unwrap_or(std::ptr::null_mut())
    })
    .unwrap_or(std::ptr::null_mut())
}
/// # Safety
/// Null or a live handle returned by viet_engine_new; release exactly once.
#[no_mangle]
pub unsafe extern "C" fn viet_engine_free(engine: *mut VietEngine) {
    if !engine.is_null() {
        let _ = catch_unwind(AssertUnwindSafe(|| drop(unsafe { Box::from_raw(engine) })));
    }
}
/// # Safety
/// Handle must be exclusively borrowed. app_id is null (unknown) or valid UTF-8 C text.
/// Finalize the previous context with KEY_FLUSH before changing focus.
#[no_mangle]
pub unsafe extern "C" fn viet_engine_set_app_id(engine: *mut VietEngine, app_id: *const c_char) {
    let Some(engine) = (unsafe { engine.as_mut() }) else {
        return;
    };
    let app = if app_id.is_null() {
        ""
    } else {
        (unsafe { CStr::from_ptr(app_id) }).to_str().unwrap_or("")
    };
    let _ = catch_unwind(AssertUnwindSafe(|| engine.set_app_id(app)));
}
/// # Safety
/// Handle must be live and exclusively borrowed. Pointers in result are borrowed
/// until the next mutating call on this handle or until it is freed. Never free them.
#[no_mangle]
pub unsafe extern "C" fn viet_engine_process_key(
    engine: *mut VietEngine,
    keyval: u32,
    keycode: u32,
    modifiers: u32,
) -> CActionResult {
    let Some(engine) = (unsafe { engine.as_mut() }) else {
        return CActionResult::default();
    };
    catch_unwind(AssertUnwindSafe(|| {
        let result = engine.process_key(keyval, keycode, modifiers);
        engine.store_result(result)
    }))
    .unwrap_or_else(|_| {
        engine.reset();
        CActionResult::default()
    })
}
/// # Safety
/// Handle must be live and exclusively borrowed. Reset discards pending preedit.
#[no_mangle]
pub unsafe extern "C" fn viet_engine_reset(engine: *mut VietEngine) {
    if let Some(engine) = unsafe { engine.as_mut() } {
        let _ = catch_unwind(AssertUnwindSafe(|| engine.reset()));
    }
}
