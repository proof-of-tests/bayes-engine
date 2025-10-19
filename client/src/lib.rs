use dioxus::prelude::*;
use wasm_bindgen::prelude::*;

// WASM entry point for the Dioxus web app (client-side)
// This function is called from JavaScript in the browser
#[wasm_bindgen]
pub fn hydrate() {
    console_error_panic_hook::set_once();
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
