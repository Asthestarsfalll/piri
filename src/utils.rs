use std::ffi::CString;
use std::os::raw::c_char;

/// Set the process name (for Linux)
/// This sets the thread name, which is what shows up in `ps -o comm=`
#[cfg(target_os = "linux")]
pub fn set_process_name(name: &str) {
    // PR_SET_NAME is 15 on Linux
    const PR_SET_NAME: libc::c_int = 15;

    // Truncate to 15 bytes (16 bytes including null terminator) as per prctl limitation
    let truncated = if name.len() > 15 { &name[..15] } else { name };

    let name_cstr = match CString::new(truncated) {
        Ok(s) => s,
        Err(_) => return, // Invalid name, skip
    };

    unsafe {
        libc::prctl(PR_SET_NAME, name_cstr.as_ptr() as *const c_char, 0, 0, 0);
    }
}

#[cfg(not(target_os = "linux"))]
pub fn set_process_name(_name: &str) {
    // No-op on non-Linux systems
}

/// Send a desktop notification using notify-send
pub fn send_notification(summary: &str, body: &str) {
    let _ = std::process::Command::new("notify-send")
        .arg("-a")
        .arg("piri")
        .arg("-i")
        .arg("dialog-error")
        .arg(summary)
        .arg(body)
        .spawn();
}
