/// # Safe Luau API Wrappers
///
/// These helpers wrap the Luau C API in ergonomic Rust functions.
/// They are designed to be used by the Sandbox module.

use std::ffi::CString;

/// The maximum allowed depth for table nesting (anti-recursion).
pub const MAX_TABLE_DEPTH: u32 = 50;

/// The maximum length of a string that can be passed to/from Luau.
pub const MAX_STRING_LENGTH: usize = 1_000_000;

/// Convert a Rust string slice to a C string suitable for Luau.
/// Returns an error if the string contains interior null bytes.
pub fn to_luau_string(s: &str) -> Result<CString, String> {
    if s.len() > MAX_STRING_LENGTH {
        return Err(format!(
            "String too long: {} bytes (max {})",
            s.len(),
            MAX_STRING_LENGTH
        ));
    }
    CString::new(s).map_err(|_| "String contains interior null bytes".to_string())
}

/// Strip a Luau library by name from the global table.
/// In C API terms: lua_pushnil(L); lua_setglobal(L, name);
pub fn strip_library(_state: *mut std::ffi::c_void, _lib_name: &str) {
    // Production implementation uses the C API.
}

/// Inject a safe C function as a global.
pub fn inject_safe_function(
    _state: *mut std::ffi::c_void,
    _name: &str,
    _callback: unsafe extern "C" fn(*mut std::ffi::c_void) -> i32,
) {
    // Production: lua_pushcfunction(state, callback); lua_setglobal(state, name);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_string_conversion() {
        let cstr = to_luau_string("hello world").unwrap();
        assert_eq!(cstr.to_str().unwrap(),"hello world");
    }

    #[test]
    fn test_string_with_nulls_fails() {
        let result = to_luau_string("hello\0world");
        assert!(result.is_err());
    }

    #[test]
    fn test_string_too_long() {
        let long = "a".repeat(MAX_STRING_LENGTH + 1);
        let result = to_luau_string(&long);
        assert!(result.is_err());
    }
}
