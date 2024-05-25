
mod mediator;
mod util;
use async_std::task;
use std::ffi::CStr;
use std::os::raw::c_char;
use std::path::PathBuf;
use encoding_rs::GBK;

#[no_mangle]
pub extern "C" fn send_file(file_paths: *const *const c_char,length:usize,file_name:*const c_char) -> *const c_char {
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

    let offer = task::block_on(mediator::make_send_offer(paths_vec, file_name_str));
    return std::ptr::null();
}


// ***********************************************
// cpp call rust test begin
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
// cpp call rust test end
// ***********************************************
