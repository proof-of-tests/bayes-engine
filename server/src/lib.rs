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

#[derive(Deserialize)]
struct MessageRequest {
    message: String,
}

#[derive(Serialize)]
struct Message {
    message: String,
}

#[derive(Serialize)]
struct MessagesResponse {
    messages: Vec<Message>,
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
        .get_async("/api/messages", |_req, ctx| async move {
            let env = ctx.env;

            // TODO: Replace with actual Hyperdrive/Postgres connection
            // For now, using KV storage as a workaround for WASM compatibility
            // The proper solution requires either:
            // 1. Using CloudFlare D1 (native SQLite)
            // 2. HTTP bridge to Postgres
            // 3. JavaScript interop with Hyperdrive
            let messages = match env.kv("MESSAGES") {
                Ok(kv) => {
                    // Try to get messages from KV
                    match kv.list().execute().await {
                        Ok(keys) => {
                            let mut msgs = Vec::new();
                            for key in keys.keys {
                                if let Ok(Some(value)) = kv.get(&key.name).text().await {
                                    msgs.push(Message { message: value });
                                }
                            }
                            msgs
                        }
                        Err(_) => Vec::new(),
                    }
                }
                Err(_) => {
                    // KV not available, return empty list
                    Vec::new()
                }
            };

            let response = MessagesResponse { messages };
            Response::from_json(&response)
        })
        .post_async("/api/messages", |mut req, ctx| async move {
            let env = ctx.env;

            // Parse the JSON request body
            let body: MessageRequest = match req.json().await {
                Ok(b) => b,
                Err(e) => {
                    return Response::error(format!("Invalid request: {}", e), 400);
                }
            };

            // TODO: Replace with actual Hyperdrive/Postgres INSERT
            // For now, using KV storage as a workaround
            match env.kv("MESSAGES") {
                Ok(kv) => {
                    // Use timestamp as key for uniqueness
                    let key = format!("msg_{}", Date::now().as_millis());
                    match kv.put(&key, &body.message) {
                        Ok(builder) => match builder.execute().await {
                            Ok(_) => {
                                let response = Message {
                                    message: body.message,
                                };
                                Response::from_json(&response)
                            }
                            Err(e) => {
                                Response::error(format!("Failed to store message: {}", e), 500)
                            }
                        },
                        Err(e) => Response::error(format!("Failed to create put: {}", e), 500),
                    }
                }
                Err(_) => Response::error("KV storage not configured", 500),
            }
        })
        .run(req, env)
        .await
}
