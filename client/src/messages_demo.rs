use dioxus::prelude::*;
use gloo_net::http::Request;

use crate::{Message, MessageRequest, MessagesResponse};

#[component]
pub fn MessagesDemo() -> Element {
    let mut message_input = use_signal(String::new);
    let mut messages = use_signal(Vec::<Message>::new);
    let mut is_loading = use_signal(|| false);
    let mut is_submitting = use_signal(|| false);
    let mut error = use_signal(|| Option::<String>::None);

    // Load messages on component mount
    use_effect(move || {
        spawn(async move {
            is_loading.set(true);
            match Request::get("/api/messages").send().await {
                Ok(response) => match response.json::<MessagesResponse>().await {
                    Ok(data) => {
                        messages.set(data.messages);
                        error.set(None);
                    }
                    Err(e) => {
                        error.set(Some(format!("Error parsing messages: {}", e)));
                    }
                },
                Err(e) => {
                    error.set(Some(format!("Error loading messages: {}", e)));
                }
            }
            is_loading.set(false);
        });
    });

    let submit_message = move || {
        let text = message_input().clone();
        if text.trim().is_empty() {
            return;
        }

        spawn(async move {
            is_submitting.set(true);
            error.set(None);

            let request_body = MessageRequest { message: text };

            match Request::post("/api/messages").json(&request_body) {
                Ok(req) => match req.send().await {
                    Ok(response) => {
                        if response.ok() {
                            match response.json::<Message>().await {
                                Ok(new_message) => {
                                    // Add the new message to the list
                                    let mut current_messages = messages();
                                    current_messages.push(new_message);
                                    messages.set(current_messages);
                                    message_input.set(String::new());
                                }
                                Err(e) => {
                                    error.set(Some(format!("Error parsing response: {}", e)));
                                }
                            }
                        } else {
                            error.set(Some(format!("Server error: {}", response.status())));
                        }
                    }
                    Err(e) => {
                        error.set(Some(format!("Error sending message: {}", e)));
                    }
                },
                Err(e) => {
                    error.set(Some(format!("Error creating request: {}", e)));
                }
            }

            is_submitting.set(false);
        });
    };

    rsx! {
        div { class: "messages-section",
            h2 { "Messages Demo" }
            p { "Add messages to the database and see them listed below" }

            if let Some(err) = error() {
                div { class: "error-message",
                    "Error: {err}"
                }
            }

            div { class: "message-input-section",
                input {
                    class: "message-input",
                    r#type: "text",
                    placeholder: "Enter your message...",
                    value: "{message_input}",
                    disabled: is_submitting(),
                    oninput: move |evt| message_input.set(evt.value().clone()),
                    onkeydown: move |evt| {
                        if evt.key() == Key::Enter {
                            submit_message();
                        }
                    },
                }

                button {
                    class: "message-submit-button",
                    disabled: is_submitting() || message_input().trim().is_empty(),
                    onclick: move |_| submit_message(),
                    if is_submitting() { "Sending..." } else { "Send Message" }
                }
            }

            div { class: "messages-list",
                h3 { "Messages:" }

                if is_loading() {
                    p { "Loading messages..." }
                } else if messages().is_empty() {
                    p { class: "empty-message", "No messages yet. Be the first to add one!" }
                } else {
                    ul { class: "message-items",
                        for message in messages() {
                            li { class: "message-item",
                                "{message.message}"
                            }
                        }
                    }
                }
            }
        }
    }
}
