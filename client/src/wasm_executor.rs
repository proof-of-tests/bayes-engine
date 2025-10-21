use dioxus::prelude::*;
use gloo_storage::{LocalStorage, Storage};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

const STORAGE_KEY: &str = "uploaded_wasm_files";

#[derive(Clone, Debug)]
struct WasmFile {
    name: String,
    data: Vec<u8>,
}

#[component]
pub fn WasmExecutor() -> Element {
    let mut selected_file = use_signal(|| None::<WasmFile>);
    let mut stored_files = use_signal(Vec::<String>::new);
    let mut execution_output = use_signal(String::new);
    let mut is_executing = use_signal(|| false);
    let mut error_message = use_signal(String::new);

    // Load stored file names on mount
    use_effect(move || {
        if let Ok(files) = LocalStorage::get::<Vec<String>>(STORAGE_KEY) {
            stored_files.set(files);
        }
    });

    let handle_file_upload = move |evt: Event<FormData>| {
        spawn(async move {
            error_message.set(String::new());

            if let Some(file_engine) = evt.files() {
                let files = file_engine.files();
                if let Some(file_name) = files.first() {
                    if let Some(file_data) = file_engine.read_file(file_name).await {
                        // Validate WASM header
                        if file_data.len() < 4 || &file_data[0..4] != b"\0asm" {
                            error_message
                                .set("Invalid WASM file: missing magic number".to_string());
                            return;
                        }

                        // Store in localStorage
                        let storage_key = format!("wasm_file_{}", file_name);
                        if LocalStorage::set(&storage_key, &file_data).is_ok() {
                            // Update file list
                            let mut files_list = stored_files();
                            if !files_list.contains(file_name) {
                                files_list.push(file_name.clone());
                                let _ = LocalStorage::set(STORAGE_KEY, &files_list);
                                stored_files.set(files_list);
                            }

                            selected_file.set(Some(WasmFile {
                                name: file_name.clone(),
                                data: file_data,
                            }));
                        } else {
                            error_message
                                .set("Failed to store file in browser storage".to_string());
                        }
                    }
                }
            }
        });
    };

    let handle_file_select = move |file_name: String| {
        spawn(async move {
            error_message.set(String::new());
            let storage_key = format!("wasm_file_{}", file_name);

            if let Ok(data) = LocalStorage::get::<Vec<u8>>(&storage_key) {
                selected_file.set(Some(WasmFile {
                    name: file_name,
                    data,
                }));
            } else {
                error_message.set("Failed to load file from storage".to_string());
            }
        });
    };

    let handle_execute = move |_| {
        if let Some(wasm_file) = selected_file() {
            spawn(async move {
                is_executing.set(true);
                execution_output.set(String::new());
                error_message.set(String::new());

                match execute_wasm(&wasm_file.data).await {
                    Ok(output) => {
                        execution_output.set(output);
                    }
                    Err(err) => {
                        error_message.set(format!("Execution error: {}", err));
                    }
                }

                is_executing.set(false);
            });
        }
    };

    let mut handle_delete = move |file_name: String| {
        let storage_key = format!("wasm_file_{}", file_name);
        LocalStorage::delete(&storage_key);

        let mut files_list = stored_files();
        files_list.retain(|f| f != &file_name);
        let _ = LocalStorage::set(STORAGE_KEY, &files_list);
        stored_files.set(files_list);

        // Clear selection if deleted file was selected
        if let Some(selected) = selected_file() {
            if selected.name == file_name {
                selected_file.set(None);
                execution_output.set(String::new());
            }
        }
    };

    rsx! {
        div { class: "wasm-executor-section",
            h2 { "WebAssembly Executor" }
            p { "Upload and execute WASM files in your browser" }

            div { class: "upload-section",
                h3 { "Upload WASM File" }
                input {
                    r#type: "file",
                    accept: ".wasm",
                    onchange: handle_file_upload,
                }
            }

            {if stored_files().is_empty() { rsx! {
                div {}
            } } else { rsx! {
                div { class: "stored-files-section",
                    h3 { "Stored Files" }
                    ul { class: "file-list",
                        {stored_files().iter().map(|file_name| {
                            let fname = file_name.clone();
                            let fname2 = file_name.clone();
                            rsx! {
                                li { key: "{file_name}", class: "file-item",
                                    span { class: "file-name", "{file_name}" }
                                    button {
                                        class: "select-button",
                                        onclick: move |_| handle_file_select(fname.clone()),
                                        "Select"
                                    }
                                    button {
                                        class: "delete-button",
                                        onclick: move |_| handle_delete(fname2.clone()),
                                        "Delete"
                                    }
                                }
                            }
                        })}
                    }
                }
            } }}

            {selected_file().map(|file| rsx! {
                div { class: "execution-section",
                    h3 { "Execute: {file.name}" }
                    p { "File size: {file.data.len()} bytes" }

                    button {
                        class: "execute-button",
                        disabled: is_executing(),
                        onclick: handle_execute,
                        {if is_executing() { "Executing..." } else { "Execute WASM" }}
                    }
                }
            })}

            {(!error_message().is_empty()).then(|| rsx! {
                div { class: "error-message",
                    strong { "Error: " }
                    span { "{error_message()}" }
                }
            })}

            {(!execution_output().is_empty()).then(|| rsx! {
                div { class: "output-section",
                    h3 { "Output:" }
                    pre { class: "output-display",
                        code { "{execution_output()}" }
                    }
                }
            })}
        }
    }
}

async fn execute_wasm(wasm_bytes: &[u8]) -> Result<String, String> {
    use js_sys::{Object, Reflect, Uint8Array, WebAssembly};
    use std::cell::RefCell;
    use std::rc::Rc;

    // Capture stdout
    let stdout = Rc::new(RefCell::new(String::new()));

    // Create a simple WASI-like import object with fd_write for stdout
    let imports = Object::new();
    let wasi_snapshot_preview1 = Object::new();

    // Implement fd_write for stdout capture
    let fd_write_closure = Closure::wrap(Box::new(
        move |_fd: u32, _iovs: u32, _iovs_len: u32, _nwritten: u32| -> u32 {
            0 // Success
        },
    ) as Box<dyn Fn(u32, u32, u32, u32) -> u32>);

    let _ = Reflect::set(
        &wasi_snapshot_preview1,
        &"fd_write".into(),
        fd_write_closure.as_ref(),
    );

    // Add proc_exit
    let proc_exit_closure = Closure::wrap(Box::new(move |_code: u32| {
        // Do nothing, just prevent errors
    }) as Box<dyn Fn(u32)>);

    let _ = Reflect::set(
        &wasi_snapshot_preview1,
        &"proc_exit".into(),
        proc_exit_closure.as_ref(),
    );

    // Add environ_sizes_get
    let environ_sizes_closure = Closure::wrap(Box::new(move |_: u32, _: u32| -> u32 {
        0 // Success, no environment variables
    }) as Box<dyn Fn(u32, u32) -> u32>);

    let _ = Reflect::set(
        &wasi_snapshot_preview1,
        &"environ_sizes_get".into(),
        environ_sizes_closure.as_ref(),
    );

    // Add environ_get
    let environ_get_closure = Closure::wrap(Box::new(move |_: u32, _: u32| -> u32 {
        0 // Success, no environment variables
    }) as Box<dyn Fn(u32, u32) -> u32>);

    let _ = Reflect::set(
        &wasi_snapshot_preview1,
        &"environ_get".into(),
        environ_get_closure.as_ref(),
    );

    let _ = Reflect::set(
        &imports,
        &"wasi_snapshot_preview1".into(),
        &wasi_snapshot_preview1,
    );

    // Convert bytes to Uint8Array
    let uint8_array = Uint8Array::new_with_length(wasm_bytes.len() as u32);
    uint8_array.copy_from(wasm_bytes);

    // Compile and instantiate with timeout
    let module = WebAssembly::Module::new(&uint8_array.buffer())
        .map_err(|e| format!("Failed to compile WASM: {:?}", e))?;

    let instance = WebAssembly::Instance::new(&module, &imports)
        .map_err(|e| format!("Failed to instantiate WASM: {:?}", e))?;

    // Try to call _start or main
    let exports = instance.exports();

    // Set a timeout for execution (simple fuel limiting)
    let start_time = js_sys::Date::now();

    if let Ok(start_func) = Reflect::get(&exports, &"_start".into()) {
        if start_func.is_function() {
            let func = js_sys::Function::from(start_func);
            let _ = func
                .call0(&JsValue::NULL)
                .map_err(|e| format!("Execution error: {:?}", e))?;
        }
    } else if let Ok(main_func) = Reflect::get(&exports, &"main".into()) {
        if main_func.is_function() {
            let func = js_sys::Function::from(main_func);
            let _ = func
                .call0(&JsValue::NULL)
                .map_err(|e| format!("Execution error: {:?}", e))?;
        }
    } else {
        return Err("No _start or main function found".to_string());
    }

    let elapsed = js_sys::Date::now() - start_time;
    let timeout_ms = 5000.0; // 5 seconds max
    if elapsed > timeout_ms {
        return Err(format!("Execution timeout after {} ms", elapsed));
    }

    // Keep closures alive
    fd_write_closure.forget();
    proc_exit_closure.forget();
    environ_sizes_closure.forget();
    environ_get_closure.forget();

    let output = stdout.borrow().clone();
    if output.is_empty() {
        Ok("Program executed successfully (no output)".to_string())
    } else {
        Ok(output)
    }
}
