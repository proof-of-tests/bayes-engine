use dioxus::prelude::*;
use worker::*;

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
    <link rel="stylesheet" href="/style.css">
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
