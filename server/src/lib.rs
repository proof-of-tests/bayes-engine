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

/// Establishes a connection to the Postgres database via Hyperdrive
async fn connect_to_db(env: &Env) -> Result<tokio_postgres::Client> {
    // Get Hyperdrive configuration
    let hyperdrive = env.hyperdrive("HYPERDRIVE")?;

    // Establish socket connection with TLS
    let socket = Socket::builder()
        .secure_transport(SecureTransport::StartTls)
        .connect(hyperdrive.host(), hyperdrive.port())?;

    // Parse connection configuration
    let config = hyperdrive
        .connection_string()
        .parse::<tokio_postgres::Config>()
        .map_err(|e| Error::RustError(format!("Failed to parse connection string: {}", e)))?;

    // Connect using raw socket with NoTls (TLS is handled by the socket layer)
    let (client, connection) = config
        .connect_raw(socket, tokio_postgres::NoTls)
        .await
        .map_err(|e| Error::RustError(format!("Failed to connect to database: {}", e)))?;

    // Handle connection lifecycle asynchronously
    wasm_bindgen_futures::spawn_local(async move {
        if let Err(error) = connection.await {
            console_log!("Database connection error: {:?}", error);
        }
    });

    Ok(client)
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

            // Connect to Postgres via Hyperdrive
            let client = match connect_to_db(&env).await {
                Ok(client) => client,
                Err(e) => {
                    return Response::error(format!("Failed to connect to database: {}", e), 500);
                }
            };

            // Query all messages from the database
            match client
                .query("SELECT message FROM messages ORDER BY message", &[])
                .await
            {
                Ok(rows) => {
                    let messages: Vec<Message> = rows
                        .iter()
                        .map(|row| {
                            let message: String = row.get(0);
                            Message { message }
                        })
                        .collect();

                    let response = MessagesResponse { messages };
                    Response::from_json(&response)
                }
                Err(e) => Response::error(format!("Failed to query messages: {}", e), 500),
            }
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

            // Connect to Postgres via Hyperdrive
            let client = match connect_to_db(&env).await {
                Ok(client) => client,
                Err(e) => {
                    return Response::error(format!("Failed to connect to database: {}", e), 500);
                }
            };

            // Insert message into the database
            match client
                .execute(
                    "INSERT INTO messages (message) VALUES ($1)",
                    &[&body.message],
                )
                .await
            {
                Ok(_) => {
                    let response = Message {
                        message: body.message,
                    };
                    Response::from_json(&response)
                }
                Err(e) => Response::error(format!("Failed to insert message: {}", e), 500),
            }
        })
        .get_async("/*catchall", |_req, _ctx| async move {
            // For SPA routing: serve index.html for all non-API routes
            // This allows the Dioxus router to handle routing on the client side
            // Static assets (/pkg/*, /style.css) are served by CloudFlare Workers Assets
            const INDEX_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Hello World - Dioxus on CloudFlare Workers</title>
    <link rel="stylesheet" href="/style.css">
</head>
<body>
    <div id="main"></div>
    <script type="module">
        import init, { hydrate } from '/pkg/client.js';
        init().then(() => {
            hydrate();
        });
    </script>
</body>
</html>"#;

            Response::from_html(INDEX_HTML)
        })
        .run(req, env)
        .await
}
