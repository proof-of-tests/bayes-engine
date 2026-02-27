use dioxus::prelude::*;
use dioxus_router::{Link, Routable, Router};
use gloo_net::http::{Request, Response};
use hyperloglog::{HyperLogLog, DEFAULT_HLL_BITS};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use wasm_bindgen::prelude::*;

#[derive(Clone, Routable, Debug, PartialEq)]
enum Route {
    #[route("/")]
    Home {},
    #[route("/repo/:owner/:repo")]
    Repo { owner: String, repo: String },
}

#[derive(Clone, Deserialize)]
struct RepositoryListResponse {
    total_estimated_tests: f64,
    repository_count: usize,
    version_count: usize,
    file_count: usize,
    function_count: usize,
    repositories: Vec<RepositorySummary>,
}

#[derive(Clone, Deserialize)]
struct RepositorySummary {
    github_repo: String,
    latest_version: Option<String>,
    latest_estimated_tests: f64,
    total_estimated_tests: f64,
    version_count: usize,
    file_count: usize,
    function_count: usize,
    submitted_updates: i64,
}

#[derive(Clone, Deserialize)]
struct RepositoryDetailResponse {
    repository: String,
    latest_version: Option<String>,
    total_estimated_tests: f64,
    latest_estimated_tests: f64,
    submitted_updates: i64,
    versions: Vec<VersionSummary>,
}

#[derive(Clone, Deserialize)]
struct VersionSummary {
    version: String,
    is_latest: bool,
    estimated_tests: f64,
    submitted_updates: i64,
    file_count: usize,
    function_count: usize,
    files: Vec<WasmFileSummary>,
}

#[derive(Clone, Deserialize)]
struct WasmFileSummary {
    id: i64,
    sha256: String,
    uploaded_at: String,
    functions: Vec<FunctionSummary>,
}

#[derive(Clone, Deserialize)]
struct FunctionSummary {
    id: i64,
    wasm_file_id: i64,
    name: String,
    estimated_tests: f64,
    submitted_updates: i64,
}

#[derive(Clone, Deserialize)]
struct UploadCatalogResponse {
    files: Vec<WasmFileSummary>,
}

#[derive(Deserialize)]
struct ApiErrorPayload {
    error: Option<String>,
}

#[derive(Serialize)]
struct SubmitHashRequest {
    function_id: i64,
    wasm_file_id: Option<i64>,
    function_name: Option<String>,
    seed: String,
    hash: String,
}

struct ExecutableFunction {
    function_id: i64,
    wasm_file_id: i64,
    function_name: String,
    wasm_bytes: Vec<u8>,
}

#[wasm_bindgen]
pub fn hydrate() {
    console_error_panic_hook::set_once();
    dioxus_web::launch::launch(App, vec![], Default::default());
}

#[component]
fn App() -> Element {
    rsx! { Router::<Route> {} }
}

#[component]
fn Home() -> Element {
    let mut data = use_signal(|| None::<RepositoryListResponse>);
    let mut load_error = use_signal(|| None::<String>);

    use_resource(move || async move {
        match Request::get("/api/repositories").send().await {
            Ok(response) => {
                if !response.ok() {
                    load_error.set(Some(
                        api_error_message(response, "Failed loading repositories").await,
                    ));
                    return;
                }
                match response.json::<RepositoryListResponse>().await {
                    Ok(payload) => {
                        load_error.set(None);
                        data.set(Some(payload));
                    }
                    Err(err) => {
                        load_error.set(Some(format!("Failed decoding repositories: {err}")))
                    }
                }
            }
            Err(err) => load_error.set(Some(format!("Failed loading repositories: {err}"))),
        }
    });

    let repositories = data().map(|d| d.repositories).unwrap_or_default();
    let total_tests = data()
        .as_ref()
        .map(|d| format_estimate(d.total_estimated_tests))
        .unwrap_or_else(|| "0".to_string());

    let repo_count = data().as_ref().map(|d| d.repository_count).unwrap_or(0);
    let version_count = data().as_ref().map(|d| d.version_count).unwrap_or(0);
    let file_count = data().as_ref().map(|d| d.file_count).unwrap_or(0);
    let function_count = data().as_ref().map(|d| d.function_count).unwrap_or(0);

    rsx! {
        div { class: "landing-page",
            section { class: "hero",
                div { class: "hero-badge", "Proof-of-Work For Tests" }
                h1 { class: "hero-title", "{total_tests}" }
                p { class: "hero-subtitle", "Estimated total tests executed across uploaded WASM test functions" }
                div { class: "hero-stats",
                    div { class: "hero-stat", span { class: "label", "Repositories" } span { class: "value", "{repo_count}" } }
                    div { class: "hero-stat", span { class: "label", "Versions" } span { class: "value", "{version_count}" } }
                    div { class: "hero-stat", span { class: "label", "WASM Files" } span { class: "value", "{file_count}" } }
                    div { class: "hero-stat", span { class: "label", "Functions" } span { class: "value", "{function_count}" } }
                }
            }

            if let Some(err) = load_error() {
                div { class: "error-banner", "{err}" }
            }

            section { class: "repo-grid",
                for repo in repositories {
                    {render_repo_card(repo)}
                }
            }
        }
    }
}

fn render_repo_card(repo: RepositorySummary) -> Element {
    let (owner, name) = split_repo_name(&repo.github_repo);

    rsx! {
        article { class: "repo-card",
            h2 { class: "repo-name", "{repo.github_repo}" }
            p { class: "repo-main-metric", "Latest estimate: {format_estimate(repo.latest_estimated_tests)} tests" }
            p { class: "repo-metric", "All versions: {format_estimate(repo.total_estimated_tests)}" }
            p { class: "repo-metric", "Versions: {repo.version_count} | Files: {repo.file_count} | Functions: {repo.function_count}" }
            p { class: "repo-metric", "Submitted improvements: {repo.submitted_updates}" }
            if let Some(version) = repo.latest_version.clone() {
                p { class: "repo-version", "Latest version: {version}" }
            }

            div { class: "repo-actions",
                Link {
                    class: "repo-link",
                    to: Route::Repo { owner, repo: name },
                    "View details"
                }
            }

            RepoRunner {
                repository: repo.github_repo,
                latest_version: repo.latest_version,
            }
        }
    }
}

#[component]
fn Repo(owner: String, repo: String) -> Element {
    let repository = format!("{owner}/{repo}");
    let mut data = use_signal(|| None::<RepositoryDetailResponse>);
    let mut load_error = use_signal(|| None::<String>);

    use_resource({
        let owner = owner.clone();
        let repo = repo.clone();
        move || {
            let owner = owner.clone();
            let repo = repo.clone();
            async move {
                let url = format!("/api/repositories/{owner}/{repo}");
                match Request::get(&url).send().await {
                    Ok(response) => {
                        if !response.ok() {
                            load_error.set(Some(
                                api_error_message(response, "Failed loading repository detail")
                                    .await,
                            ));
                            return;
                        }
                        match response.json::<RepositoryDetailResponse>().await {
                            Ok(payload) => {
                                load_error.set(None);
                                data.set(Some(payload));
                            }
                            Err(err) => {
                                load_error.set(Some(format!("Failed decoding detail: {err}")))
                            }
                        }
                    }
                    Err(err) => {
                        load_error.set(Some(format!("Failed loading repository detail: {err}")))
                    }
                }
            }
        }
    });

    rsx! {
        div { class: "repo-detail-page",
            nav { class: "top-nav", Link { to: Route::Home {}, "← Back" } }
            h1 { class: "detail-title", "{repository}" }

            if let Some(err) = load_error() {
                div { class: "error-banner", "{err}" }
            }

            if let Some(detail) = data() {
                p { class: "detail-summary", "Total estimated tests: {format_estimate(detail.total_estimated_tests)}" }
                p { class: "detail-summary", "Latest version estimate: {format_estimate(detail.latest_estimated_tests)}" }
                p { class: "detail-summary", "Submitted improvements: {detail.submitted_updates}" }

                RepoRunner {
                    repository: detail.repository.clone(),
                    latest_version: detail.latest_version.clone(),
                }

                div { class: "versions-grid",
                    for version in detail.versions {
                        article { class: "version-card",
                            h2 {
                                "{version.version}"
                                if version.is_latest {
                                    span { class: "latest-chip", "latest" }
                                }
                            }
                            p { "Estimated tests: {format_estimate(version.estimated_tests)}" }
                            p { "Submitted improvements: {version.submitted_updates}" }
                            p { "Files: {version.file_count} | Functions: {version.function_count}" }

                            for file in version.files {
                                div { class: "file-card",
                                    p { class: "file-sha", "SHA-256: {file.sha256}" }
                                    p { class: "file-meta", "Uploaded: {file.uploaded_at}" }
                                    ul {
                                        for function in file.functions {
                                            li {
                                                strong { "{function.name}" }
                                                " · est {format_estimate(function.estimated_tests)}"
                                                " · updates {function.submitted_updates}"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn RepoRunner(repository: String, latest_version: Option<String>) -> Element {
    let mut is_running = use_signal(|| false);
    let mut attempts = use_signal(|| 0u64);
    let mut improvements = use_signal(|| 0u64);
    let mut runner_error = use_signal(|| None::<String>);

    let on_start = {
        let repository = repository.clone();
        move |_| {
            if is_running() {
                return;
            }
            is_running.set(true);
            attempts.set(0);
            improvements.set(0);
            runner_error.set(None);

            let repository = repository.clone();
            spawn(async move {
                let mut executables = match load_latest_functions(&repository).await {
                    Ok(value) => value,
                    Err(err) => {
                        runner_error.set(Some(err));
                        is_running.set(false);
                        return;
                    }
                };

                if executables.is_empty() {
                    runner_error.set(Some("No callable test functions found".to_string()));
                    is_running.set(false);
                    return;
                }

                let mut local_hll = HashMap::<i64, HyperLogLog>::new();
                let mut state = 0x1234_5678_abcd_ef01u64;

                while is_running() {
                    gloo_timers::future::TimeoutFuture::new(30).await;
                    for executable in &mut executables {
                        let seed = next_seed(&mut state);
                        attempts += 1;

                        let hash = match execute_function_once(
                            &executable.wasm_bytes,
                            &executable.function_name,
                            seed,
                        ) {
                            Ok(hash) => hash,
                            Err(err) => {
                                runner_error.set(Some(format!(
                                    "{} failed: {}",
                                    executable.function_name, err
                                )));
                                continue;
                            }
                        };

                        let hll = local_hll
                            .entry(executable.function_id)
                            .or_insert_with(|| HyperLogLog::new(DEFAULT_HLL_BITS));
                        if hll.add_hash(hash) {
                            improvements += 1;
                            let function_id = executable.function_id;
                            let wasm_file_id = executable.wasm_file_id;
                            let function_name = executable.function_name.clone();
                            spawn(async move {
                                let _ = submit_improvement(
                                    function_id,
                                    wasm_file_id,
                                    function_name,
                                    seed,
                                    hash,
                                )
                                .await;
                            });
                        }
                    }
                }
            });
        }
    };

    let on_stop = move |_| {
        is_running.set(false);
    };

    rsx! {
        div { class: "runner-panel",
            p { class: "runner-title", "Run tests on latest version" }
            if let Some(version) = latest_version {
                p { class: "runner-version", "Target version: {version}" }
            }
            div { class: "runner-metrics",
                span { "Attempts (local): {attempts}" }
                span { "New lows sent: {improvements}" }
            }
            div { class: "runner-actions",
                if is_running() {
                    button { class: "btn-stop", onclick: on_stop, "Stop" }
                } else {
                    button { class: "btn-run", onclick: on_start, "Run tests" }
                }
            }
            if let Some(err) = runner_error() {
                p { class: "runner-error", "{err}" }
            }
            p { class: "runner-note", "The browser executes u64→u64 functions with random seeds. New local low hashes are submitted to update server-side HyperLogLog estimates." }
        }
    }
}

async fn load_latest_functions(repository: &str) -> Result<Vec<ExecutableFunction>, String> {
    let (owner, repo) = split_repo_name(repository);
    let url = format!("/api/repositories/{owner}/{repo}/latest-catalog");
    let response = Request::get(&url)
        .send()
        .await
        .map_err(|err| format!("Failed requesting latest catalog: {err}"))?;
    if !response.ok() {
        return Err(api_error_message(response, "Failed loading latest catalog").await);
    }
    let catalog = response
        .json::<UploadCatalogResponse>()
        .await
        .map_err(|err| format!("Failed decoding latest catalog: {err}"))?;

    let mut executables = Vec::new();
    for file in catalog.files {
        let wasm_url = format!("/api/wasm-files/{}", file.id);
        let response = Request::get(&wasm_url)
            .send()
            .await
            .map_err(|err| format!("Failed downloading wasm {}: {err}", file.id))?;
        if !response.ok() {
            return Err(api_error_message(response, "Failed downloading wasm bytes").await);
        }
        let wasm_bytes = response
            .binary()
            .await
            .map_err(|err| format!("Failed reading wasm bytes {}: {err}", file.id))?;

        for function in file.functions {
            // Validate function at load time.
            execute_function_once(&wasm_bytes, &function.name, 0).map_err(|err| {
                format!(
                    "Function {} in wasm file {} is not callable as u64->u64: {}",
                    function.name, file.id, err
                )
            })?;
            executables.push(ExecutableFunction {
                function_id: function.id,
                wasm_file_id: function.wasm_file_id,
                function_name: function.name,
                wasm_bytes: wasm_bytes.clone(),
            });
        }
    }

    Ok(executables)
}

fn execute_function_once(wasm_bytes: &[u8], function_name: &str, seed: u64) -> Result<u64, String> {
    let engine = wasmi::Engine::default();
    let mut store = wasmi::Store::new(&engine, ());
    let module =
        wasmi::Module::new(&engine, wasm_bytes).map_err(|err| format!("parse failed: {err}"))?;
    let linker = wasmi::Linker::new(&engine);
    let instance = linker
        .instantiate_and_start(&mut store, &module)
        .map_err(|err| format!("instantiation failed: {err}"))?;
    let func = instance
        .get_typed_func::<u64, u64>(&store, function_name)
        .map_err(|err| format!("typed function lookup failed: {err}"))?;
    func.call(&mut store, seed)
        .map_err(|err| format!("function call failed: {err}"))
}

async fn submit_improvement(
    function_id: i64,
    wasm_file_id: i64,
    function_name: String,
    seed: u64,
    hash: u64,
) -> Result<(), String> {
    let payload = SubmitHashRequest {
        function_id,
        wasm_file_id: Some(wasm_file_id),
        function_name: Some(function_name),
        seed: seed.to_string(),
        hash: hash.to_string(),
    };

    let response = Request::post("/api/test-results")
        .json(&payload)
        .map_err(|err| format!("Failed encoding payload: {err}"))?
        .send()
        .await
        .map_err(|err| format!("Failed submitting update: {err}"))?;
    if !response.ok() {
        return Err(api_error_message(response, "Failed submitting update").await);
    }

    Ok(())
}

async fn api_error_message(response: Response, fallback: &str) -> String {
    let status = response.status();
    if let Ok(payload) = response.json::<ApiErrorPayload>().await {
        if let Some(error) = payload.error {
            if !error.is_empty() {
                return format!("{fallback}: {error}");
            }
        }
    }
    format!("{fallback} (HTTP {status})")
}

fn split_repo_name(repo: &str) -> (String, String) {
    let mut parts = repo.splitn(2, '/');
    let owner = parts.next().unwrap_or("unknown").to_string();
    let name = parts.next().unwrap_or("unknown").to_string();
    (owner, name)
}

fn next_seed(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e3779b97f4a7c15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    z ^ (z >> 31)
}

fn format_estimate(value: f64) -> String {
    if value.is_nan() || value <= 0.0 {
        "0".to_string()
    } else {
        format!("{:.0}", value)
    }
}
