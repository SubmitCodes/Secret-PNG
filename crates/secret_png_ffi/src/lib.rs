use libc::c_char;
use secret_png_core::{
    embed_files, extract_payload, has_carrier_payload, inspect_carrier,
    strip_payload_to_file, EmbedOptions, ProgressUpdate,
};
use std::cell::RefCell;
use std::ffi::{CStr, CString};
use std::path::Path;

thread_local! {
    static LAST_ERROR: RefCell<Option<String>> = const { RefCell::new(None) };
}

fn set_last_error(err: String) {
    LAST_ERROR.with(|cell| {
        *cell.borrow_mut() = Some(err);
    });
}

/// Progress callback signature for C-ABI
pub type FfiProgressCallback = unsafe extern "C" fn(
    phase: *const c_char,
    bytes_processed: u64,
    total_bytes: u64,
    speed_bytes_sec: f64,
    percentage: f32,
    user_data: *mut libc::c_void,
);

fn c_str_to_str<'a>(ptr: *const c_char) -> std::result::Result<&'a str, i32> {
    if ptr.is_null() {
        return Err(-1);
    }
    unsafe {
        CStr::from_ptr(ptr)
            .to_str()
            .map_err(|_| -2)
    }
}

/// Retrieve the last error message on current thread as a heap-allocated C string
#[no_mangle]
pub extern "C" fn secret_png_last_error() -> *mut c_char {
    LAST_ERROR.with(|cell| {
        if let Some(ref err) = *cell.borrow() {
            CString::new(err.clone()).unwrap_or_default().into_raw()
        } else {
            std::ptr::null_mut()
        }
    })
}

/// Free a string returned by any FFI function
#[no_mangle]
pub extern "C" fn secret_png_free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe {
            let _ = CString::from_raw(ptr);
        }
    }
}

/// Check if file contains carrier metadata (1 = true, 0 = false, negative = error)
#[no_mangle]
pub extern "C" fn secret_png_has_carrier(carrier_path: *const c_char) -> i32 {
    let path_str = match c_str_to_str(carrier_path) {
        Ok(s) => s,
        Err(code) => return code,
    };
    if has_carrier_payload(path_str) {
        1
    } else {
        0
    }
}

/// Inspect carrier image and return serialized JSON metadata
#[no_mangle]
pub extern "C" fn secret_png_inspect_json(
    carrier_path: *const c_char,
    out_json: *mut *mut c_char,
) -> i32 {
    if out_json.is_null() {
        return -1;
    }
    let path_str = match c_str_to_str(carrier_path) {
        Ok(s) => s,
        Err(code) => return code,
    };

    match inspect_carrier(path_str) {
        Ok((trailer, meta)) => {
            let info = serde_json::json!({
                "protocol_version": meta.protocol_version,
                "original_filename": meta.original_filename,
                "file_extension": meta.file_extension,
                "mime_type": meta.mime_type,
                "original_file_size": meta.original_file_size,
                "payload_size": meta.payload_size,
                "host_image_size": trailer.host_image_size,
                "is_encrypted": meta.is_encrypted,
                "blake3_hex": meta.blake3_hex,
                "crc32": meta.crc32,
                "host_image_format": meta.host_image_format,
                "host_image_width": meta.host_image_width,
                "host_image_height": meta.host_image_height,
            });
            let json_str = serde_json::to_string(&info).unwrap_or_default();
            unsafe {
                *out_json = CString::new(json_str).unwrap_or_default().into_raw();
            }
            0
        }
        Err(e) => {
            set_last_error(e.to_string());
            -3
        }
    }
}

/// Embed a video file into a host image with progress callback
#[no_mangle]
pub extern "C" fn secret_png_embed(
    host_path: *const c_char,
    payload_path: *const c_char,
    output_path: *const c_char,
    password: *const c_char,
    progress_cb: Option<FfiProgressCallback>,
    user_data: *mut libc::c_void,
    out_report_json: *mut *mut c_char,
) -> i32 {
    let host_str = match c_str_to_str(host_path) {
        Ok(s) => s,
        Err(c) => return c,
    };
    let payload_str = match c_str_to_str(payload_path) {
        Ok(s) => s,
        Err(c) => return c,
    };
    let out_str = match c_str_to_str(output_path) {
        Ok(s) => s,
        Err(c) => return c,
    };

    let pass_opt = if !password.is_null() {
        c_str_to_str(password).ok().map(|s| s.to_string())
    } else {
        None
    };

    let cb_wrapper = progress_cb.map(|cb| {
        let user_ptr = user_data as usize;
        let closure: Box<dyn Fn(ProgressUpdate) + Send + Sync> = Box::new(move |up: ProgressUpdate| {
            let phase_c = CString::new(up.phase).unwrap_or_default();
            unsafe {
                cb(
                    phase_c.as_ptr(),
                    up.bytes_processed,
                    up.total_bytes,
                    up.speed_bytes_sec,
                    up.percentage,
                    user_ptr as *mut libc::c_void,
                );
            }
        });
        closure
    });

    match embed_files(
        host_str,
        payload_str,
        out_str,
        EmbedOptions { password: pass_opt },
        cb_wrapper,
    ) {
        Ok(report) => {
            if !out_report_json.is_null() {
                let json = serde_json::json!({
                    "original_file_name": report.original_file_name,
                    "host_image_size": report.host_image_size,
                    "payload_size": report.payload_size,
                    "total_carrier_size": report.total_carrier_size,
                    "blake3_hex": report.blake3_hex,
                    "crc32": report.crc32,
                    "is_encrypted": report.is_encrypted,
                    "elapsed_millis": report.elapsed_millis,
                });
                let json_str = serde_json::to_string(&json).unwrap_or_default();
                unsafe {
                    *out_report_json = CString::new(json_str).unwrap_or_default().into_raw();
                }
            }
            0
        }
        Err(e) => {
            set_last_error(e.to_string());
            -4
        }
    }
}

/// Extract embedded video from carrier image
#[no_mangle]
pub extern "C" fn secret_png_extract(
    carrier_path: *const c_char,
    output_path: *const c_char,
    password: *const c_char,
    progress_cb: Option<FfiProgressCallback>,
    user_data: *mut libc::c_void,
    out_report_json: *mut *mut c_char,
) -> i32 {
    let carrier_str = match c_str_to_str(carrier_path) {
        Ok(s) => s,
        Err(c) => return c,
    };

    let out_opt = if !output_path.is_null() {
        c_str_to_str(output_path).ok().map(Path::new)
    } else {
        None
    };

    let pass_opt = if !password.is_null() {
        c_str_to_str(password).ok()
    } else {
        None
    };

    let cb_wrapper = progress_cb.map(|cb| {
        let user_ptr = user_data as usize;
        let closure: Box<dyn Fn(ProgressUpdate) + Send + Sync> = Box::new(move |up: ProgressUpdate| {
            let phase_c = CString::new(up.phase).unwrap_or_default();
            unsafe {
                cb(
                    phase_c.as_ptr(),
                    up.bytes_processed,
                    up.total_bytes,
                    up.speed_bytes_sec,
                    up.percentage,
                    user_ptr as *mut libc::c_void,
                );
            }
        });
        closure
    });

    match extract_payload(carrier_str, out_opt, pass_opt, cb_wrapper) {
        Ok(report) => {
            if !out_report_json.is_null() {
                let json = serde_json::json!({
                    "output_path": report.output_path.to_string_lossy(),
                    "original_filename": report.original_filename,
                    "file_size": report.file_size,
                    "blake3_hex": report.blake3_hex,
                    "crc32": report.crc32,
                    "is_encrypted": report.is_encrypted,
                    "elapsed_millis": report.elapsed_millis,
                });
                let json_str = serde_json::to_string(&json).unwrap_or_default();
                unsafe {
                    *out_report_json = CString::new(json_str).unwrap_or_default().into_raw();
                }
            }
            0
        }
        Err(e) => {
            set_last_error(e.to_string());
            -5
        }
    }
}

/// Strip embedded payload to restore original image
#[no_mangle]
pub extern "C" fn secret_png_strip(
    carrier_path: *const c_char,
    output_path: *const c_char,
    out_report_json: *mut *mut c_char,
) -> i32 {
    let carrier_str = match c_str_to_str(carrier_path) {
        Ok(s) => s,
        Err(c) => return c,
    };
    let out_str = match c_str_to_str(output_path) {
        Ok(s) => s,
        Err(c) => return c,
    };

    match strip_payload_to_file(carrier_str, out_str) {
        Ok(report) => {
            if !out_report_json.is_null() {
                let json = serde_json::json!({
                    "original_host_image_size": report.original_host_image_size,
                    "payload_bytes_removed": report.payload_bytes_removed,
                    "host_image_format": report.host_image_format,
                });
                let json_str = serde_json::to_string(&json).unwrap_or_default();
                unsafe {
                    *out_report_json = CString::new(json_str).unwrap_or_default().into_raw();
                }
            }
            0
        }
        Err(e) => {
            set_last_error(e.to_string());
            -6
        }
    }
}
