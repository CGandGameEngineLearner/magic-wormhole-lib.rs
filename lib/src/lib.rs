
mod mediator;
mod util;

#[no_mangle]
pub extern "C" fn send(c_file_path: *const c_char) -> *const c_char {
    let file_path = unsafe {
        if c_file_path.is_null() {
            return std::ptr::null();
        }
        let cstr_file_path = CStr::from_ptr(c_file_path);
        util::cstr_to_option_string(cstr_file_path)
    };
    
    //let offer = mediator::make_send_offer(, file_path).await?;

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


use std::ffi::{CStr, CString};
use std::os::raw::c_char;

#[no_mangle]
pub extern "C" fn process_gb18030_string(input: *const c_char) -> *const c_char {
    // Cpp的字符串传过来时 不可避免的unsafe
    let c_str = unsafe {
        if input.is_null() {
            return std::ptr::null();
        }
        CStr::from_ptr(input)
    };

    // 将 C 字符串转换为 Rust 字符串
    let r_str = match c_str.to_str() {
        Ok(s) => s,
        Err(_) => return CString::new("Invalid UTF-8").unwrap().into_raw(),
    };

    // 对字符串进行处理（示例：转换为大写）
    let result_str = r_str.to_uppercase();

    // 将 Rust 字符串转换回 C 字符串
    let c_result_str = CString::new(result_str).unwrap();
    c_result_str.into_raw()
}



// ***********************************************
// cpp call rust test end
// ***********************************************
