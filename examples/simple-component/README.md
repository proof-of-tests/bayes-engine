# Simple WebAssembly Module Example

This example demonstrates how to create a simple WebAssembly module with exported functions.

## Building the Module

Build the module with:

```bash
cargo build --target wasm32-unknown-unknown --release
```

The compiled module will be in `target/wasm32-unknown-unknown/release/simple_wasm_module.wasm`.

## Using the Module

1. Build the module using the instructions above
2. Open the bayes-engine client in your browser
3. Navigate to the Tests page
4. Find the "WebAssembly Executor" section
5. Upload the `simple_wasm_module.wasm` file
6. Click "Execute" to run it

The module exports:

- `add(a, b)` - Adds two numbers
- `get_greeting()` - Returns a simple value

## Creating Your Own Module

To create your own WASM module:

1. Create a new Rust library with `cargo new --lib my-module`
2. Set `crate-type = ["cdylib"]` in `Cargo.toml`
3. Export functions with `#[no_mangle] pub extern "C"`
4. Build with `cargo build --target wasm32-unknown-unknown --release`
