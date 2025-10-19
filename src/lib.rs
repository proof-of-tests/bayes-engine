use dioxus::prelude::*;
use worker::*;

#[event(fetch)]
async fn fetch(req: Request, _env: Env, _ctx: worker::Context) -> Result<Response> {
    // Get the path from the request
    let path = req.path();

    // Route handling
    match path.as_str() {
        "/" => {
            // Render the Dioxus app to HTML
            let mut vdom = VirtualDom::new(App);
            vdom.rebuild_in_place();
            let html = dioxus_ssr::render(&vdom);

            // Create HTML response with proper DOCTYPE and structure
            let full_html = format!(
                r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Hello World - Dioxus on CloudFlare Workers</title>
    <style>
        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', Arial, sans-serif;
            display: flex;
            justify-content: center;
            align-items: center;
            min-height: 100vh;
            margin: 0;
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
        }}
        .container {{
            background: white;
            padding: 3rem;
            border-radius: 1rem;
            box-shadow: 0 20px 60px rgba(0, 0, 0, 0.3);
            text-align: center;
        }}
    </style>
</head>
<body>
    {html}
</body>
</html>"#
            );

            Response::from_html(full_html)
        }
        _ => Response::error("Not Found", 404),
    }
}

#[component]
fn App() -> Element {
    rsx! {
        div { class: "container",
            h1 { "Hello World!" }
            p { "This is a Dioxus app running on CloudFlare Workers" }
            p {
                "Built with "
                a { href: "https://dioxuslabs.com/", target: "_blank", "Dioxus" }
                " and "
                a { href: "https://github.com/cloudflare/workers-rs", target: "_blank", "workers-rs" }
            }
        }
    }
}
