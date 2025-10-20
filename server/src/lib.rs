use serde::{Deserialize, Serialize};
use worker::*;

#[derive(Deserialize)]
struct UppercaseRequest {
    text: String,
}

#[derive(Serialize)]
struct UppercaseResponse {
    result: String,
}

#[event(fetch)]
async fn fetch(req: Request, env: Env, _ctx: worker::Context) -> Result<Response> {
    console_error_panic_hook::set_once();

    // Create a router to handle different routes
    let router = Router::new();

    router
        .post_async("/api/uppercase", |mut req, _ctx| async move {
            // Parse the JSON request body
            let body: UppercaseRequest = req.json().await?;

            // Convert text to uppercase
            let result = body.text.to_uppercase();

            // Create response
            let response = UppercaseResponse { result };

            // Return JSON response
            Response::from_json(&response)
        })
        .run(req, env)
        .await
}
