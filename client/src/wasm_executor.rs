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

                match execute_wasm_module(&wasm_file.data).await {
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
            p { "Upload and execute WebAssembly modules using wasmi" }

            div { class: "upload-section",
                h3 { "Upload WASM Module" }
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
                    h3 { "Stored Modules" }
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
                        {if is_executing() { "Executing..." } else { "Execute Module" }}
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

async fn execute_wasm_module(wasm_bytes: &[u8]) -> Result<String, String> {
    use wasmi::{Engine, Linker, Module, Store};

    // Create an engine and store
    let engine = Engine::default();
    let mut store = Store::new(&engine, ());

    // Parse the WASM module
    let module =
        Module::new(&engine, wasm_bytes).map_err(|e| format!("Failed to parse module: {}", e))?;

    // Create a linker (for imports)
    let linker = Linker::new(&engine);

    // Instantiate the module (without running start function)
    // Note: We use the deprecated `instantiate` method because we want to avoid
    // running the start function. The recommended `instantiate_and_start` would
    // automatically run the start function.
    #[allow(deprecated)]
    let pre_instance = linker
        .instantiate(&mut store, &module)
        .map_err(|e| format!("Failed to instantiate module: {}", e))?;

    let instance = pre_instance
        .ensure_no_start(&mut store)
        .map_err(|e| format!("Module has start function, which is not supported: {}", e))?;

    // Try to call the add function as a test
    if let Ok(add_func) = instance.get_typed_func::<(i32, i32), i32>(&store, "add") {
        match add_func.call(&mut store, (10, 32)) {
            Ok(result) => {
                return Ok(format!("Successfully executed! add(10, 32) = {}", result));
            }
            Err(e) => {
                return Err(format!("Failed to call add function: {}", e));
            }
        }
    }

    // Try to call get_greeting function
    if let Ok(greeting_func) = instance.get_typed_func::<(), i32>(&store, "get_greeting") {
        match greeting_func.call(&mut store, ()) {
            Ok(result) => {
                return Ok(format!("get_greeting() returned: {}", result));
            }
            Err(e) => {
                return Err(format!("Failed to call get_greeting: {}", e));
            }
        }
    }

    Ok("Module loaded successfully (no recognized exports)".to_string())
}
