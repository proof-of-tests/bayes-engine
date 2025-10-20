use crate::hyperloglog::HyperLogLog;
use dioxus::prelude::*;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = Math)]
    fn random() -> f64;
}

fn generate_random_hash() -> u64 {
    // Generate a pseudo-random 64-bit hash using JavaScript's Math.random()
    let high = (random() * (u32::MAX as f64)) as u64;
    let low = (random() * (u32::MAX as f64)) as u64;
    (high << 32) | low
}

#[component]
pub fn HyperLogLogDemo() -> Element {
    let mut bits = use_signal(|| 8u8);
    let mut hll = use_signal(|| HyperLogLog::new(8));
    let mut real_count = use_signal(|| 0u64);
    let mut is_running = use_signal(|| false);

    // Use resource to handle periodic hash generation
    let _ticker = use_resource(move || async move {
        loop {
            gloo_timers::future::TimeoutFuture::new(50).await;

            if is_running() {
                // Add a random hash
                let hash = generate_random_hash();
                let seed = real_count();

                hll.write().add(seed, hash);
                real_count += 1;
            }
        }
    });

    let estimated_count = hll.read().count();
    let real = real_count();
    let error_percentage = if real > 0 {
        ((estimated_count - real as f64).abs() / real as f64) * 100.0
    } else {
        0.0
    };

    let error_class = if error_percentage < 5.0 {
        "stat-value good"
    } else if error_percentage < 10.0 {
        "stat-value warning"
    } else {
        "stat-value error"
    };

    rsx! {
        div { class: "hyperloglog-section",
            h2 { "HyperLogLog Cardinality Estimator" }
            p {
                "HyperLogLog is a probabilistic data structure that estimates the number of unique elements in a set. "
                "This demo shows how the estimated count stays close to the actual count, especially with higher bit values."
            }

            div { class: "controls",
                div { class: "control-group",
                    label { "Number of bits: " }
                    select {
                        value: "{bits}",
                        onchange: move |evt| {
                            let new_bits: u8 = evt.value().parse().unwrap_or(8);
                            bits.set(new_bits);
                            hll.set(HyperLogLog::new(new_bits));
                            real_count.set(0);
                            is_running.set(false);
                        },
                        option { value: "4", "4 bits (16 registers)" }
                        option { value: "5", "5 bits (32 registers)" }
                        option { value: "6", "6 bits (64 registers)" }
                        option { value: "7", "7 bits (128 registers)" }
                        option { value: "8", "8 bits (256 registers)" }
                        option { value: "9", "9 bits (512 registers)" }
                        option { value: "10", "10 bits (1024 registers)" }
                        option { value: "11", "11 bits (2048 registers)" }
                        option { value: "12", "12 bits (4096 registers)" }
                    }
                }

                button {
                    class: "control-button",
                    onclick: move |_| {
                        is_running.set(!is_running());
                    },
                    if is_running() { "Stop" } else { "Start" }
                }

                button {
                    class: "control-button reset-button",
                    onclick: move |_| {
                        hll.set(HyperLogLog::new(bits()));
                        real_count.set(0);
                        is_running.set(false);
                    },
                    "Reset"
                }
            }

            div { class: "stats",
                div { class: "stat-item",
                    div { class: "stat-label", "Real Count:" }
                    div { class: "stat-value", "{real}" }
                }

                div { class: "stat-item",
                    div { class: "stat-label", "Estimated Count:" }
                    div { class: "stat-value", "{estimated_count:.2}" }
                }

                div { class: "stat-item",
                    div { class: "stat-label", "Error:" }
                    div {
                        class: "{error_class}",
                        "{error_percentage:.2}%"
                    }
                }
            }

            div { class: "info",
                p {
                    "The HyperLogLog algorithm provides excellent accuracy with minimal memory usage. "
                    "With {bits} bits, we use {1 << bits()} registers to estimate cardinality. "
                    "Higher bit values provide more accurate estimates but use more memory."
                }
            }
        }
    }
}
