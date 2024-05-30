mod mediator;

use std::ptr;
use std::ffi::CStr;
use std::ffi::CString;
use std::path::PathBuf;
use std::os::raw::c_char;

use async_std::task;
use encoding_rs::UTF_8;

// ***********************************************
// lib:
// C/C++ Can Call Them
// ***********************************************

/// # 发送文件
#[no_mangle]
pub extern "C" fn send_files(
    file_paths: *const *const c_char,
    length: usize,
    new_name: *const c_char,
    code_length: usize,
) -> *const c_char {
    // # 发送文件
    let paths_slice = unsafe { std::slice::from_raw_parts(file_paths, length) };
    let mut paths_vec = Vec::new();

    for &path in paths_slice {
        let c_str = unsafe { CStr::from_ptr(path) };
        let bytes = c_str.to_bytes();
        let (decoded, _, _) = UTF_8.decode(bytes);
        let path_buf = PathBuf::from(decoded.to_string());
        paths_vec.push(path_buf);
    }

    // 转换文件名
    let file_name_str = if !new_name.is_null() {
        let c_str = unsafe { CStr::from_ptr(new_name) };
        let bytes = c_str.to_bytes();
        let (decoded, _, _) = UTF_8.decode(bytes);
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

/// # 接受文件
#[no_mangle]
pub extern "C" fn receive_files(wormhole_code:*const c_char,save_path: *const c_char)->bool{
    let c_wormhole_code = unsafe { CStr::from_ptr(wormhole_code) };
    let c_wormhole_code_bytes = c_wormhole_code.to_bytes();
    let (decoded_wormhole_code, _, _) = UTF_8.decode(c_wormhole_code_bytes);

    let s_wormhole_code = decoded_wormhole_code.to_string();

    let c_save_path = unsafe { CStr::from_ptr(save_path) };
    let c_save_path_bytes = c_save_path.to_bytes();
    let (save_path_decoded, _, _) = UTF_8.decode(c_save_path_bytes);

    let buf_save_path = PathBuf::from(save_path_decoded.to_string());

    let res =  task::block_on(mediator::try_recieve(s_wormhole_code,buf_save_path));
    match res{
        Ok(res) => {
            return res;
        },
        Err(error_report) => {
            println!("{:?}", error_report);
            return false;
        },
    }

}


// ***********************************************
// lib:
// C/C++ Can Call Them
// ***********************************************




// ***********************************************
// Cpp Call Rust Test
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
// Cpp Call Rust Test
// End
// ***********************************************



// ***********************************************
// Test Case
// ***********************************************

#[test]
fn test_send()
{
    // 创建一些 Rust 字符串
    let file_paths = vec!["./README.md"];

    // 将这些字符串转换为 CString
    let c_strings: Vec<CString> = file_paths
        .into_iter()
        .map(|s| CString::new(s).expect("CString::new failed"))
        .collect();

    // 获取每个 CString 的指针
    let c_ptrs: Vec<*const c_char> = c_strings.iter()
        .map(|s| s.as_ptr())
        .collect();

    // 获取指向这些指针的指针
    let c_ptrs_ptr: *const *const c_char = c_ptrs.as_ptr();

    
    let new_name: *const c_char = CString::new("new_name").expect("CString::new failed").as_ptr();


    let c_wormhole_code: *const c_char = send_files(c_ptrs_ptr,1, new_name,2);

    println!("Pointer to pointers: {:?}", c_wormhole_code);
}