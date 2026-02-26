use anyhow::{anyhow, Context, Result};
use clap::Parser;
use rand::prelude::SliceRandom;
use reqwest::blocking::Client as BlockingClient;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tokio::runtime::Builder as TokioRuntimeBuilder;
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;
use wasmtime::{Config, Engine, Linker, Module, OptLevel, Store, TypedFunc};

#[derive(Parser, Debug)]
#[command(name = "bayes-cli")]
#[command(about = "High-performance WASM test runner for bayes-engine")]
struct Args {
    #[arg(long, default_value = "https://bayes.lemmih.com")]
    base_url: String,

    #[arg(long, default_value_t = num_cpus::get())]
    cores: usize,

    #[arg(long, default_value_t = 12)]
    hll_bits: u8,
}

#[derive(Debug, Deserialize)]
struct RepositoryListResponse {
    total_estimated_tests: f64,
    repositories: Vec<RepositorySummary>,
}

#[derive(Debug, Deserialize, Clone)]
struct RepositorySummary {
    github_repo: String,
}

#[derive(Debug, Deserialize)]
struct UploadCatalogResponse {
    files: Vec<WasmFileSummary>,
}

#[derive(Debug, Deserialize)]
struct WasmFileHllStateResponse {
    functions: Vec<FunctionHllStateEntry>,
}

#[derive(Debug, Deserialize)]
struct FunctionHllStateEntry {
    function_id: i64,
    hll_bits: u8,
    hashes: Vec<String>,
}

#[derive(Debug, Deserialize, Clone)]
struct WasmFileSummary {
    id: i64,
    functions: Vec<FunctionSummary>,
}

#[derive(Debug, Deserialize, Clone)]
struct FunctionSummary {
    id: i64,
    wasm_file_id: i64,
    name: String,
    estimated_tests: f64,
}

#[derive(Debug, Serialize)]
struct SubmitHashRequest {
    function_id: i64,
    wasm_file_id: Option<i64>,
    function_name: Option<String>,
    seed: String,
    hash: String,
}

#[derive(Debug, Deserialize)]
struct SubmitHashResponse {
    improved: bool,
    estimated_tests: f64,
}

#[derive(Debug)]
struct Target {
    repository: String,
    wasm_bytes: Vec<u8>,
    functions: Vec<FunctionSummary>,
    baseline_total_estimate: f64,
    hll_bits: u8,
    server_hashes_by_function: HashMap<i64, Vec<u64>>,
}

#[derive(Debug)]
struct Submission {
    slot_index: usize,
    function_id: i64,
    wasm_file_id: i64,
    function_name: String,
    register: usize,
    seed: u64,
    hash: u64,
}

#[derive(Default)]
struct Metrics {
    local_tests: AtomicU64,
    submitted_hashes: AtomicU64,
    failed_submissions: AtomicU64,
    estimate_gain_bits: AtomicU64,
    last_error: Mutex<String>,
}

impl Metrics {
    fn add_estimate_gain(&self, delta: f64) {
        if !(delta.is_finite() && delta > 0.0) {
            return;
        }
        let mut old = self.estimate_gain_bits.load(Ordering::Relaxed);
        loop {
            let current = f64::from_bits(old);
            let next = (current + delta).to_bits();
            match self.estimate_gain_bits.compare_exchange_weak(
                old,
                next,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => return,
                Err(actual) => old = actual,
            }
        }
    }

    fn estimate_gain(&self) -> f64 {
        f64::from_bits(self.estimate_gain_bits.load(Ordering::Relaxed))
    }
}

struct Shutdown {
    requested: AtomicBool,
    wait_mutex: Mutex<()>,
    wait_cvar: Condvar,
    token: CancellationToken,
}

impl Shutdown {
    fn new() -> Self {
        Self {
            requested: AtomicBool::new(false),
            wait_mutex: Mutex::new(()),
            wait_cvar: Condvar::new(),
            token: CancellationToken::new(),
        }
    }

    fn request(&self) {
        if !self.requested.swap(true, Ordering::SeqCst) {
            self.token.cancel();
            self.wait_cvar.notify_all();
        }
    }

    fn is_requested(&self) -> bool {
        self.requested.load(Ordering::SeqCst)
    }

    fn wait_timeout(&self, duration: Duration) {
        if self.is_requested() {
            return;
        }
        if let Ok(guard) = self.wait_mutex.lock() {
            let _ = self.wait_cvar.wait_timeout(guard, duration);
        }
    }
}

#[derive(Clone, Copy)]
struct HashSample {
    hash: u64,
    seed: u64,
}

struct FunctionSlotState {
    local: Vec<HashSample>,
    server: Vec<u64>,
}

struct FunctionSlot {
    meta: FunctionSummary,
    state: Mutex<FunctionSlotState>,
}

struct SubmissionState {
    bits: u8,
    slots: Vec<Arc<FunctionSlot>>,
    slot_by_function_id: HashMap<i64, usize>,
    notify: Notify,
}

impl SubmissionState {
    fn new(bits: u8, functions: Vec<FunctionSummary>) -> Self {
        let registers = 1usize << bits;
        let empty = HashSample {
            hash: u64::MAX,
            seed: 0,
        };
        let slots = functions
            .into_iter()
            .map(|meta| {
                Arc::new(FunctionSlot {
                    meta,
                    state: Mutex::new(FunctionSlotState {
                        local: vec![empty; registers],
                        server: vec![u64::MAX; registers],
                    }),
                })
            })
            .collect::<Vec<_>>();
        let slot_by_function_id = slots
            .iter()
            .enumerate()
            .map(|(index, slot)| (slot.meta.id, index))
            .collect();
        Self {
            bits,
            slots,
            slot_by_function_id,
            notify: Notify::new(),
        }
    }

    fn register_for_hash(&self, hash: u64) -> usize {
        let mask = (1usize << self.bits) - 1;
        (hash as usize) & mask
    }

    fn update_local(&self, slot: &Arc<FunctionSlot>, hash: u64, seed: u64) {
        let register = self.register_for_hash(hash);
        if let Ok(mut guard) = slot.state.lock() {
            if hash < guard.local[register].hash {
                guard.local[register] = HashSample { hash, seed };
                if hash < guard.server[register] {
                    self.notify.notify_one();
                }
            }
        }
    }

    fn next_submission(&self) -> Option<Submission> {
        let mut best: Option<Submission> = None;
        for (slot_index, slot) in self.slots.iter().enumerate() {
            let Ok(guard) = slot.state.lock() else {
                continue;
            };
            for (register, local_sample) in guard.local.iter().enumerate() {
                if local_sample.hash >= guard.server[register] {
                    continue;
                }
                let candidate = Submission {
                    slot_index,
                    function_id: slot.meta.id,
                    wasm_file_id: slot.meta.wasm_file_id,
                    function_name: slot.meta.name.clone(),
                    register,
                    seed: local_sample.seed,
                    hash: local_sample.hash,
                };
                match &best {
                    Some(current) if candidate.hash >= current.hash => {}
                    _ => best = Some(candidate),
                }
            }
        }
        best
    }

    fn mark_server_seen(&self, slot_index: usize, register: usize, hash: u64) {
        if let Some(slot) = self.slots.get(slot_index) {
            if let Ok(mut guard) = slot.state.lock() {
                if hash < guard.server[register] {
                    guard.server[register] = hash;
                }
            }
        }
    }

    fn initialize_server_hashes(&self, function_id: i64, hashes: &[u64]) {
        let Some(&slot_index) = self.slot_by_function_id.get(&function_id) else {
            return;
        };
        let Some(slot) = self.slots.get(slot_index) else {
            return;
        };
        if let Ok(mut guard) = slot.state.lock() {
            for (idx, hash) in hashes.iter().enumerate().take(guard.server.len()) {
                if *hash < guard.server[idx] {
                    guard.server[idx] = *hash;
                }
            }
        }
    }
}

struct LocalHyperLogLog {
    bits: u8,
    hashes: Vec<u64>,
}

impl LocalHyperLogLog {
    fn new(bits: u8) -> Self {
        let m = 1usize << bits;
        Self {
            bits,
            hashes: vec![u64::MAX; m],
        }
    }

    fn add_hash(&mut self, hash: u64) -> bool {
        let mask = (1usize << self.bits) - 1;
        let register = (hash as usize) & mask;
        if hash < self.hashes[register] {
            self.hashes[register] = hash;
            return true;
        }
        false
    }
}

fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9e3779b97f4a7c15);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
    x ^ (x >> 31)
}

fn next_seed(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e3779b97f4a7c15);
    splitmix64(*state)
}

fn split_repo_name(repo: &str) -> (String, String) {
    let mut parts = repo.splitn(2, '/');
    let owner = parts.next().unwrap_or("unknown").to_string();
    let name = parts.next().unwrap_or("unknown").to_string();
    (owner, name)
}

fn endpoint(base: &str, path: &str) -> String {
    format!(
        "{}/{}",
        base.trim_end_matches('/'),
        path.trim_start_matches('/')
    )
}

fn fetch_target(client: &BlockingClient, base_url: &str) -> Result<Target> {
    let repos_url = endpoint(base_url, "/api/repositories");
    let repo_list = client
        .get(repos_url)
        .send()
        .context("requesting /api/repositories")?
        .error_for_status()
        .context("loading /api/repositories")?
        .json::<RepositoryListResponse>()
        .context("decoding /api/repositories")?;

    if repo_list.repositories.is_empty() {
        return Err(anyhow!("No repositories available"));
    }

    let mut repos = repo_list.repositories;
    let mut rng = rand::rng();
    repos.shuffle(&mut rng);

    for repo in repos {
        let (owner, name) = split_repo_name(&repo.github_repo);
        let catalog_url = endpoint(
            base_url,
            &format!("/api/repositories/{owner}/{name}/latest-catalog"),
        );

        let catalog_resp = match client.get(catalog_url).send() {
            Ok(resp) => resp,
            Err(_) => continue,
        };
        let catalog = match catalog_resp.error_for_status() {
            Ok(resp) => match resp.json::<UploadCatalogResponse>() {
                Ok(c) => c,
                Err(_) => continue,
            },
            Err(_) => continue,
        };

        let mut files: Vec<WasmFileSummary> = catalog
            .files
            .into_iter()
            .filter(|f| !f.functions.is_empty())
            .collect();
        if files.is_empty() {
            continue;
        }
        files.shuffle(&mut rng);
        let file = files.remove(0);

        let wasm_url = endpoint(base_url, &format!("/api/wasm-files/{}", file.id));
        let wasm_bytes = client
            .get(wasm_url)
            .send()
            .context("requesting wasm file")?
            .error_for_status()
            .context("loading wasm file")?
            .bytes()
            .context("reading wasm file bytes")?
            .to_vec();

        let hll_url = endpoint(base_url, &format!("/api/wasm-files/{}/hll-state", file.id));
        let hll_state = client
            .get(hll_url)
            .send()
            .context("requesting wasm hll state")?
            .error_for_status()
            .context("loading wasm hll state")?
            .json::<WasmFileHllStateResponse>()
            .context("decoding wasm hll state")?;

        let hll_bits = hll_state
            .functions
            .first()
            .map(|entry| entry.hll_bits)
            .unwrap_or(12);
        let mut server_hashes_by_function = HashMap::new();
        for entry in hll_state.functions {
            let hashes = entry
                .hashes
                .iter()
                .filter_map(|value| value.parse::<u64>().ok())
                .collect::<Vec<_>>();
            server_hashes_by_function.insert(entry.function_id, hashes);
        }

        return Ok(Target {
            repository: repo.github_repo,
            wasm_bytes,
            functions: file.functions,
            baseline_total_estimate: repo_list.total_estimated_tests,
            hll_bits,
            server_hashes_by_function,
        });
    }

    Err(anyhow!(
        "No repository had a latest catalog with callable WASM functions"
    ))
}

fn create_engine() -> Result<Engine> {
    let mut config = Config::new();
    config.cranelift_opt_level(OptLevel::Speed);
    config.wasm_simd(true);
    config.strategy(wasmtime::Strategy::Cranelift);
    config.epoch_interruption(true);
    Engine::new(&config).context("creating wasmtime engine")
}

async fn with_shutdown<T>(
    shutdown: &Shutdown,
    fut: impl std::future::Future<Output = T>,
) -> Option<T> {
    tokio::select! {
        _ = shutdown.token.cancelled() => None,
        result = fut => Some(result),
    }
}

async fn backoff_or_stop(shutdown: &Shutdown, duration: Duration) -> bool {
    with_shutdown(shutdown, tokio::time::sleep(duration))
        .await
        .is_some()
}

async fn submission_loop_async(
    shutdown: Arc<Shutdown>,
    base_url: String,
    submission_state: Arc<SubmissionState>,
    metrics: Arc<Metrics>,
    initial_estimates: HashMap<i64, f64>,
) {
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(2))
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());
    let submit_url = endpoint(&base_url, "/api/test-results");
    let mut last_estimate_by_function = initial_estimates;

    loop {
        let submission = match submission_state.next_submission() {
            Some(next) => next,
            None => {
                let Some(_) = with_shutdown(&shutdown, submission_state.notify.notified()).await
                else {
                    break;
                };
                continue;
            }
        };

        let payload = SubmitHashRequest {
            function_id: submission.function_id,
            wasm_file_id: Some(submission.wasm_file_id),
            function_name: Some(submission.function_name.clone()),
            seed: submission.seed.to_string(),
            hash: submission.hash.to_string(),
        };

        let mut submitted = false;
        for attempt in 1..=3 {
            let Some(send_result) =
                with_shutdown(&shutdown, client.post(&submit_url).json(&payload).send()).await
            else {
                break;
            };
            let Ok(response) = send_result else {
                if attempt == 3 {
                    metrics.failed_submissions.fetch_add(1, Ordering::Relaxed);
                    if let Ok(mut guard) = metrics.last_error.lock() {
                        *guard = "network error while submitting hash".to_string();
                    }
                } else {
                    if !backoff_or_stop(&shutdown, Duration::from_millis(100)).await {
                        break;
                    }
                }
                continue;
            };

            let status = response.status();
            if !status.is_success() {
                let Some(body_result) = with_shutdown(&shutdown, response.text()).await else {
                    break;
                };
                let body = body_result.unwrap_or_else(|_| "<no body>".to_string());
                if attempt == 3 || !(status.is_server_error() || status.as_u16() == 429) {
                    metrics.failed_submissions.fetch_add(1, Ordering::Relaxed);
                    if let Ok(mut guard) = metrics.last_error.lock() {
                        *guard = format!("submit failed: HTTP {} {}", status.as_u16(), body.trim());
                    }
                    break;
                }
                if !backoff_or_stop(&shutdown, Duration::from_millis(100)).await {
                    break;
                }
                continue;
            }

            let Some(parsed_result) =
                with_shutdown(&shutdown, response.json::<SubmitHashResponse>()).await
            else {
                break;
            };

            match parsed_result {
                Ok(parsed) => {
                    metrics.submitted_hashes.fetch_add(1, Ordering::Relaxed);
                    if parsed.improved {
                        let prev = last_estimate_by_function
                            .get(&submission.function_id)
                            .copied()
                            .unwrap_or(0.0);
                        let delta = (parsed.estimated_tests - prev).max(0.0);
                        metrics.add_estimate_gain(delta);
                        last_estimate_by_function
                            .insert(submission.function_id, parsed.estimated_tests);
                    }
                    submission_state.mark_server_seen(
                        submission.slot_index,
                        submission.register,
                        submission.hash,
                    );
                    submitted = true;
                    break;
                }
                Err(_) => {
                    if attempt == 3 {
                        metrics.failed_submissions.fetch_add(1, Ordering::Relaxed);
                        if let Ok(mut guard) = metrics.last_error.lock() {
                            *guard = "submit succeeded but response JSON parse failed".to_string();
                        }
                    } else {
                        if !backoff_or_stop(&shutdown, Duration::from_millis(100)).await {
                            break;
                        }
                    }
                }
            }
        }

        if !submitted || shutdown.is_requested() {
            continue;
        }
    }
}

fn worker_loop(
    shutdown: Arc<Shutdown>,
    submission_state: Arc<SubmissionState>,
    metrics: Arc<Metrics>,
    engine: Arc<Engine>,
    module: Arc<Module>,
    slots: Vec<Arc<FunctionSlot>>,
    hll_bits: u8,
    worker_id: usize,
) -> Result<()> {
    let mut store = Store::new(&engine, ());
    store.set_epoch_deadline(1);
    let linker = Linker::new(&engine);
    let instance = linker
        .instantiate(&mut store, &module)
        .context("instantiating module")?;

    let mut funcs: Vec<(Arc<FunctionSlot>, TypedFunc<u64, u64>, LocalHyperLogLog)> = Vec::new();
    for slot in slots {
        let typed = instance
            .get_typed_func::<u64, u64>(&mut store, &slot.meta.name)
            .with_context(|| format!("resolving function {}", slot.meta.name))?;
        funcs.push((slot, typed, LocalHyperLogLog::new(hll_bits)));
    }

    let mut state = splitmix64(0x1234_5678_abcd_ef01u64 ^ worker_id as u64);

    while !shutdown.is_requested() {
        for (slot, typed, hll) in &mut funcs {
            if shutdown.is_requested() {
                break;
            }
            let seed = next_seed(&mut state);
            match typed.call(&mut store, seed) {
                Ok(hash) => {
                    metrics.local_tests.fetch_add(1, Ordering::Relaxed);
                    if hll.add_hash(hash) {
                        submission_state.update_local(slot, hash, seed);
                    }
                }
                Err(_) => {
                    if shutdown.is_requested() {
                        return Ok(());
                    }
                }
            }
        }
    }

    Ok(())
}

fn stats_loop(
    shutdown: Arc<Shutdown>,
    metrics: Arc<Metrics>,
    repo: String,
    cores: usize,
    base_total: f64,
) {
    let mut last_tests = 0u64;
    let mut last_time = Instant::now();

    while !shutdown.is_requested() {
        shutdown.wait_timeout(Duration::from_secs(1));
        if shutdown.is_requested() {
            break;
        }
        let now = Instant::now();

        let local_tests = metrics.local_tests.load(Ordering::Relaxed);
        let submitted = metrics.submitted_hashes.load(Ordering::Relaxed);
        let failed = metrics.failed_submissions.load(Ordering::Relaxed);
        let estimate_gain = metrics.estimate_gain();
        let last_error = metrics
            .last_error
            .lock()
            .ok()
            .map(|s| s.clone())
            .unwrap_or_default();

        let elapsed = now.duration_since(last_time).as_secs_f64().max(0.001);
        let delta_tests = local_tests.saturating_sub(last_tests) as f64;
        let rate = delta_tests / elapsed;

        let total_estimate = base_total + estimate_gain;

        print!("\x1B[2J\x1B[H");
        println!("bayes-cli running");
        println!("cores in use: {cores}");
        println!("repository: {repo}");
        println!("estimated tests total (global): {:.0}", total_estimate);
        println!("tests run locally: {local_tests}");
        println!("current tests/sec: {:.0}", rate);
        println!(
            "hashes submitted: {submitted} (estimate gain +{:.0})",
            estimate_gain
        );
        println!("failed submissions: {failed}");
        if !last_error.is_empty() {
            println!("last submit error: {last_error}");
        }
        println!("press Ctrl-C to stop");
        let _ = io::stdout().flush();

        last_tests = local_tests;
        last_time = now;
    }
}

fn stdin_shutdown_loop(shutdown: Arc<Shutdown>) {
    let mut stdin = io::stdin().lock();
    let mut buf = [0u8; 1];
    while !shutdown.is_requested() {
        match stdin.read(&mut buf) {
            Ok(0) => break,
            Ok(_) => {
                if buf[0] == 3 || buf[0] == b'q' || buf[0] == b'Q' {
                    shutdown.request();
                    break;
                }
            }
            Err(_) => break,
        }
    }
}

fn main() -> Result<()> {
    let args = Args::parse();
    let cores = args.cores.max(1);

    let client = BlockingClient::builder()
        .pool_max_idle_per_host(16)
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(15))
        .build()
        .context("creating HTTP client")?;

    let target = fetch_target(&client, &args.base_url)?;
    let engine = Arc::new(create_engine()?);
    let module = Arc::new(Module::new(&engine, &target.wasm_bytes).context("compiling module")?);
    let hll_bits = if target.hll_bits > 0 {
        target.hll_bits
    } else {
        args.hll_bits
    };

    let metrics = Arc::new(Metrics::default());
    let shutdown = Arc::new(Shutdown::new());
    let submission_state = Arc::new(SubmissionState::new(hll_bits, target.functions.clone()));
    for (function_id, hashes) in &target.server_hashes_by_function {
        submission_state.initialize_server_hashes(*function_id, hashes);
    }

    let mut initial_estimates = HashMap::new();
    for f in &target.functions {
        initial_estimates.insert(f.id, f.estimated_tests);
    }

    {
        let shutdown = shutdown.clone();
        let sigints = Arc::new(AtomicU64::new(0));
        let sigints_inner = sigints.clone();
        ctrlc::set_handler(move || {
            let count = sigints_inner.fetch_add(1, Ordering::Relaxed) + 1;
            shutdown.request();
            if count >= 2 {
                eprintln!("forcing exit on second Ctrl-C");
                std::process::exit(130);
            }
        })
        .context("installing ctrl-c handler")?;
    }

    {
        let shutdown = shutdown.clone();
        thread::spawn(move || stdin_shutdown_loop(shutdown));
    }

    let submit_thread = {
        let shutdown = shutdown.clone();
        let submission_state = submission_state.clone();
        let metrics = metrics.clone();
        let base_url = args.base_url.clone();
        thread::spawn(move || {
            let runtime = TokioRuntimeBuilder::new_current_thread()
                .enable_all()
                .build()
                .expect("failed to build tokio runtime for submit thread");
            runtime.block_on(submission_loop_async(
                shutdown,
                base_url,
                submission_state,
                metrics,
                initial_estimates,
            ));
        })
    };

    let epoch_thread = {
        let shutdown = shutdown.clone();
        let engine = engine.clone();
        thread::spawn(move || {
            while !shutdown.is_requested() {
                shutdown.wait_timeout(Duration::from_millis(100));
                if shutdown.is_requested() {
                    break;
                }
                engine.increment_epoch();
            }
            // Nudge one final time so any blocked call observes shutdown promptly.
            engine.increment_epoch();
        })
    };

    let stats_thread = {
        let shutdown = shutdown.clone();
        let metrics = metrics.clone();
        let repo = target.repository.clone();
        let base_total = target.baseline_total_estimate;
        thread::spawn(move || stats_loop(shutdown, metrics, repo, cores, base_total))
    };

    let errors = Arc::new(Mutex::new(Vec::<String>::new()));
    let mut workers = Vec::new();
    let slots = submission_state.slots.clone();
    for worker_id in 0..cores {
        let shutdown = shutdown.clone();
        let submission_state = submission_state.clone();
        let metrics = metrics.clone();
        let engine = engine.clone();
        let module = module.clone();
        let worker_slots = slots.clone();
        let errors = errors.clone();
        let hll_bits = hll_bits;
        workers.push(thread::spawn(move || {
            if let Err(err) = worker_loop(
                shutdown,
                submission_state,
                metrics,
                engine,
                module,
                worker_slots,
                hll_bits,
                worker_id,
            ) {
                if let Ok(mut guard) = errors.lock() {
                    guard.push(format!("worker {worker_id}: {err}"));
                }
            }
        }));
    }

    for worker in workers {
        let _ = worker.join();
    }
    let _ = epoch_thread.join();
    let _ = submit_thread.join();
    let _ = stats_thread.join();

    if let Ok(errors) = errors.lock() {
        if !errors.is_empty() {
            eprintln!("worker errors:");
            for err in errors.iter() {
                eprintln!("  - {err}");
            }
        }
    }

    Ok(())
}
