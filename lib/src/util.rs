

pub fn cstr_to_option_string(cstr: &std::ffi::CStr) -> Option<String> {
    cstr.to_str().ok().map(|s| s.to_string())
}