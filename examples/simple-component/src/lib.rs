// Simple WASM module that returns a greeting
#[no_mangle]
pub extern "C" fn get_greeting() -> i32 {
    // Return a simple number for now
    // In a real implementation, we'd return a pointer to a string in linear memory
    42
}

#[no_mangle]
pub extern "C" fn add(a: i32, b: i32) -> i32 {
    a + b
}
