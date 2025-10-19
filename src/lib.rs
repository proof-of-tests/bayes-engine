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
        "/style.css" => {
            // Serve the static CSS file
            let css_content = include_str!("../static/style.css");
            Response::from_html(css_content).map(|mut resp| {
                resp.headers_mut().set("Content-Type", "text/css").unwrap();
                resp
            })
        }
        "/" => {
            // Serve HTML shell that loads the WASM bundle
            let html = r#"<!DOCTYPE html>
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
        import init, { hydrate } from './index.js';
        init().then(() => {
            hydrate();
        });
    </script>
</body>
</html>"#;

            Response::from_html(html)
        }
        _ => Response::error("Not Found", 404),
    }
}

// WASM entry point for the Dioxus web app
// Note: Not using #[wasm_bindgen(start)] to avoid auto-initialization on server
#[wasm_bindgen]
pub fn hydrate() {
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
