use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Request, Response, ResponseInit};

#[derive(Deserialize)]
struct UppercaseRequest {
    text: String,
}

#[derive(Serialize)]
struct UppercaseResponse {
    result: String,
}

// Export the fetch function for Cloudflare Workers
// This function handles incoming HTTP requests
#[wasm_bindgen]
pub async fn fetch(req: Request, _env: JsValue, _ctx: JsValue) -> Result<Response, JsValue> {
    console_error_panic_hook::set_once();

    // Get the request URL and method
    let url = req.url();
    let method = req.method();

    // Handle POST /api/uppercase
    if method == "POST" && url.contains("/api/uppercase") {
        handle_uppercase(req).await
    } else {
        // Return 404 for other routes
        let init = ResponseInit::new();
        init.set_status(404);
        Response::new_with_opt_str_and_init(Some("Not Found"), &init)
    }
}

async fn handle_uppercase(req: Request) -> Result<Response, JsValue> {
    // Read the request body as text
    let body_promise = req.text()?;
    let body_text = JsFuture::from(body_promise).await?;
    let body_str = body_text.as_string().unwrap_or_default();

    // Parse the JSON request
    let request: UppercaseRequest = serde_json::from_str(&body_str)
        .map_err(|e| JsValue::from_str(&format!("Invalid JSON: {}", e)))?;

    // Convert text to uppercase
    let result = request.text.to_uppercase();

    // Create response
    let response = UppercaseResponse { result };
    let response_json = serde_json::to_string(&response)
        .map_err(|e| JsValue::from_str(&format!("Failed to serialize response: {}", e)))?;

    // Create HTTP response with JSON content type
    let init = ResponseInit::new();
    init.set_status(200);
    let headers = web_sys::Headers::new()?;
    headers.set("Content-Type", "application/json")?;
    init.set_headers(&headers);

    Response::new_with_opt_str_and_init(Some(&response_json), &init)
}
