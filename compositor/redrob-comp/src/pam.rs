//! Password check for the lock screen, through PAM.
//!
//! libpam is opened at run time (dlopen) so the build needs no PAM headers.
//! The service is `redrob-lock` (/etc/pam.d/redrob-lock, shipped in the
//! image), falling back to `login`. Runs on its own thread: PAM may sleep
//! after a wrong password.

use std::ffi::{CStr, CString, c_char, c_int, c_void};

#[repr(C)]
struct PamMessage {
    msg_style: c_int,
    msg: *const c_char,
}

#[repr(C)]
struct PamResponse {
    resp: *mut c_char,
    resp_retcode: c_int,
}

type ConvFn = extern "C" fn(c_int, *mut *const PamMessage, *mut *mut PamResponse, *mut c_void) -> c_int;

#[repr(C)]
struct PamConv {
    conv: ConvFn,
    appdata_ptr: *mut c_void,
}

const PAM_SUCCESS: c_int = 0;
const PAM_PROMPT_ECHO_OFF: c_int = 1;
const PAM_PROMPT_ECHO_ON: c_int = 2;

extern "C" fn conv(n: c_int, msgs: *mut *const PamMessage, resp: *mut *mut PamResponse, data: *mut c_void) -> c_int {
    unsafe {
        let pw = &*(data as *const CString);
        let out = libc::calloc(n as usize, std::mem::size_of::<PamResponse>()) as *mut PamResponse;
        if out.is_null() {
            return 5; // PAM_BUF_ERR
        }
        for i in 0..n as isize {
            let m = &**msgs.offset(i);
            if m.msg_style == PAM_PROMPT_ECHO_OFF || m.msg_style == PAM_PROMPT_ECHO_ON {
                (*out.offset(i)).resp = libc::strdup(pw.as_ptr());
            }
        }
        *resp = out;
    }
    PAM_SUCCESS
}

type StartFn = unsafe extern "C" fn(*const c_char, *const c_char, *const PamConv, *mut *mut c_void) -> c_int;
type AuthFn = unsafe extern "C" fn(*mut c_void, c_int) -> c_int;
type EndFn = unsafe extern "C" fn(*mut c_void, c_int) -> c_int;

/// True when `password` is the current user's.
pub fn check(password: &str) -> bool {
    let user = std::env::var("USER").or_else(|_| std::env::var("LOGNAME")).unwrap_or_default();
    if user.is_empty() {
        return false;
    }
    unsafe {
        let lib = libc::dlopen(c"libpam.so.0".as_ptr(), libc::RTLD_NOW);
        if lib.is_null() {
            tracing::warn!("lock: libpam.so.0 not found");
            return false;
        }
        let sym = |n: &CStr| libc::dlsym(lib, n.as_ptr());
        let (s, a, e) = (sym(c"pam_start"), sym(c"pam_authenticate"), sym(c"pam_end"));
        if s.is_null() || a.is_null() || e.is_null() {
            return false;
        }
        let (start, auth, end): (StartFn, AuthFn, EndFn) =
            (std::mem::transmute(s), std::mem::transmute(a), std::mem::transmute(e));
        let pw = CString::new(password).unwrap_or_default();
        let user = CString::new(user).unwrap_or_default();
        let service = if std::path::Path::new("/etc/pam.d/redrob-lock").exists() { c"redrob-lock" } else { c"login" };
        let conv = PamConv { conv, appdata_ptr: &pw as *const CString as *mut c_void };
        let mut h: *mut c_void = std::ptr::null_mut();
        if start(service.as_ptr(), user.as_ptr(), &conv, &mut h) != PAM_SUCCESS {
            return false;
        }
        let r = auth(h, 0);
        end(h, r);
        tracing::info!(ok = r == PAM_SUCCESS, "lock: password check");
        r == PAM_SUCCESS
    }
}
