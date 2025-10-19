use dioxus::prelude::*;
use wasm_bindgen::prelude::*;
use worker::*;

// CloudFlare Worker entry point
#[event(fetch)]
async fn fetch(req: Request, _env: Env, _ctx: worker::Context) -> Result<Response> {
    // Get the path from the request
    let path = req.path();

    // Route handling
    match path.as_str() {
        "/" => {
            // Serve HTML shell that loads the WASM bundle
            let html = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Hello World - Dioxus on CloudFlare Workers</title>
    <style>
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', Arial, sans-serif;
            display: flex;
            justify-content: center;
            align-items: center;
            min-height: 100vh;
            margin: 0;
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
        }
        .container {
            background: white;
            padding: 3rem;
            border-radius: 1rem;
            box-shadow: 0 20px 60px rgba(0, 0, 0, 0.3);
            text-align: center;
        }
        .counter-section {
            margin: 2rem 0;
            padding: 1.5rem;
            background: #f8f9fa;
            border-radius: 0.5rem;
        }
        .counter-label {
            font-size: 1.2rem;
            font-weight: 600;
            color: #495057;
            margin: 0 0 1rem 0;
        }
        .counter-display {
            font-size: 3rem;
            font-weight: bold;
            color: #667eea;
            margin: 1rem 0;
        }
        .counter-button {
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: white;
            border: none;
            padding: 1rem 2rem;
            font-size: 1.1rem;
            font-weight: 600;
            border-radius: 0.5rem;
            cursor: pointer;
            transition: transform 0.2s, box-shadow 0.2s;
            box-shadow: 0 4px 12px rgba(102, 126, 234, 0.4);
        }
        .counter-button:hover {
            transform: translateY(-2px);
            box-shadow: 0 6px 16px rgba(102, 126, 234, 0.6);
        }
        .counter-button:active {
            transform: translateY(0);
        }
    </style>
</head>
<body>
    <div id="main"></div>
    <script type="module">
        import init from './index.js';
        init();
    </script>
</body>
</html>"#;

            Response::from_html(html)
        }
        _ => Response::error("Not Found", 404),
    }
}

// WASM entry point for the Dioxus web app
#[wasm_bindgen(start)]
pub fn wasm_main() {
    dioxus_web::launch::launch(App, vec![], Default::default());
}

#[component]
fn App() -> Element {
    // Use Rust signal for reactive state management
    let mut count = use_signal(|| 0);

    rsx! {
        div { class: "container",
            h1 { "Hello World!" }
            p { "This is a Dioxus WASM app running on CloudFlare Workers" }

            div { class: "counter-section",
                p { class: "counter-label", "Click Counter:" }
                p { class: "counter-display", "{count}" }
                button {
                    class: "counter-button",
                    onclick: move |_| count += 1,
                    "Click Me!"
                }
            }

            p {
                "Built with "
                a { href: "https://dioxuslabs.com/", target: "_blank", "Dioxus" }
                " and "
                a { href: "https://github.com/cloudflare/workers-rs", target: "_blank", "workers-rs" }
            }
        }
    }
}
