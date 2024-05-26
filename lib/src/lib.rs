mod mediator;

use std::ffi::CStr;
use std::ffi::CString;
use std::os::raw::c_char;
use std::path::PathBuf;
use std::ptr;

use async_std::task;
use encoding_rs::GBK;

// ***********************************************
// lib:
// C/C++ can call them
// ***********************************************

/// # 发送文件
/// file_paths
#[no_mangle]
pub extern "C" fn send_file(
    file_paths: *const *const c_char,
    length: usize,
    file_name: *const c_char,
    code_length: usize,
) -> *const c_char {
    // # 发送文件
    let paths_slice = unsafe { std::slice::from_raw_parts(file_paths, length) };
    let mut paths_vec = Vec::new();

    for &path in paths_slice {
        let c_str = unsafe { CStr::from_ptr(path) };
        let bytes = c_str.to_bytes();
        let (decoded, _, _) = GBK.decode(bytes);
        let path_buf = PathBuf::from(decoded.to_string());
        paths_vec.push(path_buf);
    }

    // 转换文件名
    let file_name_str = if !file_name.is_null() {
        let c_str = unsafe { CStr::from_ptr(file_name) };
        let bytes = c_str.to_bytes();
        let (decoded, _, _) = GBK.decode(bytes);
        Some(decoded.to_string())
    } else {
        None
    };

    let wormhole_code = task::block_on(mediator::try_send(paths_vec, file_name_str, code_length));
    match wormhole_code {
        Ok(code) => {
            let c_code = CString::new(code.clone()).unwrap();
            return c_code.into_raw();
        },
        Err(error_report) => {
            println!("{:?}", error_report);
            return ptr::null();
        },
    }
}
// ***********************************************
// lib:
// C/C++ can call them
// ***********************************************

// ***********************************************
// Cpp call rust test
// Begin
// ***********************************************
#[no_mangle]
pub extern "C" fn add(a: i32, b: i32) -> i32 {
    a + b
}

#[no_mangle]
pub extern "C" fn hello() {
    println!("Hello from Rust!");
}

// ***********************************************
// Cpp call rust test
// End
// ***********************************************
