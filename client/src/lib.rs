use dioxus::prelude::*;
use dioxus_router::prelude::*;
use gloo_net::http::Request;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

mod hyperloglog;
mod hyperloglog_demo;
mod messages_demo;
use hyperloglog_demo::HyperLogLogDemo;
use messages_demo::MessagesDemo;

#[derive(Serialize)]
struct UppercaseRequest {
    text: String,
}

#[derive(Deserialize)]
struct UppercaseResponse {
    result: String,
}

#[derive(Serialize)]
pub struct MessageRequest {
    pub message: String,
}

#[derive(Deserialize, Clone, PartialEq)]
pub struct Message {
    pub message: String,
}

#[derive(Deserialize)]
pub struct MessagesResponse {
    pub messages: Vec<Message>,
}

#[derive(Clone, Routable, Debug, PartialEq)]
enum Route {
    #[route("/")]
    Home {},
    #[route("/tests")]
    Tests {},
}

// WASM entry point for the Dioxus web app (client-side)
// This function is called from JavaScript in the browser
#[wasm_bindgen]
pub fn hydrate() {
    console_error_panic_hook::set_once();
    dioxus_web::launch::launch(App, vec![], Default::default());
}

#[component]
fn App() -> Element {
    rsx! {
        Router::<Route> {}
    }
}

#[component]
fn Home() -> Element {
    rsx! {
        div { class: "home-container",
            h1 { class: "home-title", "bayes engine" }
            nav { class: "home-nav",
                Link { to: Route::Tests {}, "View Tests" }
            }
        }
    }
}

#[component]
fn Tests() -> Element {
    // Use Rust signal for reactive state management
    let mut count = use_signal(|| 0);
    let mut input_text = use_signal(String::new);
    let mut uppercase_result = use_signal(String::new);
    let mut is_loading = use_signal(|| false);

    rsx! {
        div { class: "tests-container",
            nav { class: "tests-nav",
                Link { to: Route::Home {}, "← Back to Home" }
            }

            h1 { "Tests & Demos" }

            div { class: "test-grid",
                div { class: "test-box",
                    h2 { "Click Counter" }
                    p { class: "counter-label", "Click Counter:" }
                    p { class: "counter-display", "{count}" }
                    button {
                        class: "counter-button",
                        onclick: move |_| count += 1,
                        "Click Me!"
                    }
                }

                div { class: "test-box",
                    HyperLogLogDemo {}
                }

                div { class: "test-box",
                    h2 { "Uppercase REST API Demo" }
                    p { "Enter text and click the button to convert it to uppercase using the server API" }

                    input {
                        class: "text-input",
                        r#type: "text",
                        placeholder: "Enter text here...",
                        value: "{input_text}",
                        oninput: move |evt| input_text.set(evt.value().clone()),
                    }

                    button {
                        class: "uppercase-button",
                        disabled: is_loading(),
                        onclick: move |_| {
                            let text = input_text().clone();
                            spawn(async move {
                                is_loading.set(true);

                                let request_body = UppercaseRequest { text };

                                match Request::post("/api/uppercase")
                                    .json(&request_body)
                                {
                                    Ok(req) => {
                                        match req.send().await {
                                            Ok(response) => {
                                                match response.json::<UppercaseResponse>().await {
                                                    Ok(data) => {
                                                        uppercase_result.set(data.result);
                                                    }
                                                    Err(_) => {
                                                        uppercase_result.set("Error parsing response".to_string());
                                                    }
                                                }
                                            }
                                            Err(_) => {
                                                uppercase_result.set("Error sending request".to_string());
                                            }
                                        }
                                    }
                                    Err(_) => {
                                        uppercase_result.set("Error creating request".to_string());
                                    }
                                }

                                is_loading.set(false);
                            });
                        },
                        if is_loading() { "Converting..." } else { "Convert to Uppercase" }
                    }

                    if !uppercase_result().is_empty() {
                        div { class: "result-display",
                            p { class: "result-label", "Result:" }
                            p { class: "result-text", "{uppercase_result}" }
                        }
                    }
                }

                div { class: "test-box",
                    MessagesDemo {}
                }
            }

            footer { class: "tests-footer",
                p {
                    "Built with "
                    a { href: "https://dioxuslabs.com/", target: "_blank", "Dioxus" }
                    " and "
                    a { href: "https://github.com/cloudflare/workers-rs", target: "_blank", "workers-rs" }
                }
            }
        }
    }
}
