use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use hyperloglog::{HyperLogLog, DEFAULT_HLL_BITS};
use js_sys::{Array, Function, Object, Reflect, Uint8Array};
use once_cell::sync::Lazy;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};
use std::sync::Mutex;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use worker::*;

const OIDC_CONFIG_URL: &str =
    "https://token.actions.githubusercontent.com/.well-known/openid-configuration";
const EXPECTED_OIDC_ISSUER: &str = "https://token.actions.githubusercontent.com";
const EXPECTED_OIDC_AUDIENCE: &str = "bayes-engine-ci-upload";
const GITHUB_API_BASE: &str = "https://api.github.com";
const REPLAY_TTL_SECS: u64 = 600;
const MAX_UPLOAD_BYTES: usize = 10 * 1024 * 1024;

static IN_MEMORY_REPLAY: Lazy<Mutex<HashMap<String, u64>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

#[derive(Deserialize)]
struct UppercaseRequest {
    text: String,
}

#[derive(Serialize)]
struct UppercaseResponse {
    result: String,
}

#[derive(Serialize)]
struct ApiErrorResponse {
    ok: bool,
    code: String,
    error: String,
}

#[derive(Serialize)]
struct CiUploadResponse {
    ok: bool,
    wasm_sha256: String,
    wasm_size_bytes: usize,
    wasm_valid: bool,
    dry_run: bool,
    persisted: bool,
    repository: String,
    repository_id: u64,
    run_id: Option<String>,
    run_attempt: Option<String>,
    event_name: String,
    r#ref: String,
    workflow_ref: Option<String>,
    received_at: String,
    repository_version: String,
    function_count: usize,
    function_names: Vec<String>,
    wasm_file_id: Option<i64>,
    r2_key: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
struct FunctionSummary {
    id: i64,
    wasm_file_id: i64,
    name: String,
    estimated_tests: f64,
    submitted_updates: i64,
    lowest_hash: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
struct WasmFileSummary {
    id: i64,
    sha256: String,
    r2_key: Option<String>,
    uploaded_at: String,
    functions: Vec<FunctionSummary>,
}

#[derive(Serialize, Deserialize, Clone)]
struct VersionSummary {
    version: String,
    is_latest: bool,
    estimated_tests: f64,
    submitted_updates: i64,
    file_count: usize,
    function_count: usize,
    files: Vec<WasmFileSummary>,
}

#[derive(Serialize, Deserialize, Clone)]
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

#[derive(Serialize)]
struct RepositoryListResponse {
    total_estimated_tests: f64,
    repository_count: usize,
    version_count: usize,
    file_count: usize,
    function_count: usize,
    repositories: Vec<RepositorySummary>,
}

#[derive(Serialize, Deserialize)]
struct RepositoryDetailResponse {
    repository: String,
    latest_version: Option<String>,
    total_estimated_tests: f64,
    latest_estimated_tests: f64,
    submitted_updates: i64,
    versions: Vec<VersionSummary>,
}

#[derive(Deserialize)]
struct SubmitHashRequest {
    function_id: i64,
    #[serde(default)]
    wasm_file_id: Option<i64>,
    #[serde(default)]
    function_name: Option<String>,
    seed: String,
    hash: String,
}

#[derive(Serialize)]
struct SubmitHashResponse {
    ok: bool,
    improved: bool,
    estimated_tests: f64,
    submitted_updates: i64,
}

#[derive(Serialize)]
struct UploadCatalogResponse {
    repository: String,
    version: String,
    files: Vec<WasmFileSummary>,
}

#[derive(Serialize)]
struct FunctionHllStateResponse {
    function_id: i64,
    function_name: String,
    hll_bits: u8,
    hashes: Vec<String>,
}

#[derive(Serialize)]
struct WasmFileHllStateResponse {
    wasm_file_id: i64,
    functions: Vec<FunctionHllStateResponse>,
}

#[derive(Debug)]
struct ApiError {
    status: u16,
    code: &'static str,
    message: String,
}

impl ApiError {
    fn new(status: u16, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct OpenIdConfiguration {
    issuer: String,
    jwks_uri: String,
}

#[derive(Debug, Deserialize)]
struct JwkSet {
    keys: Vec<Jwk>,
}

#[derive(Debug, Deserialize)]
struct Jwk {
    kid: Option<String>,
    kty: String,
    alg: Option<String>,
    n: Option<String>,
    e: Option<String>,
}

#[derive(Debug, Deserialize)]
struct JwtHeader {
    alg: Option<String>,
    kid: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum AudienceClaim {
    One(String),
    Many(Vec<String>),
}

impl AudienceClaim {
    fn contains(&self, expected: &str) -> bool {
        match self {
            Self::One(value) => value == expected,
            Self::Many(values) => values.iter().any(|value| value == expected),
        }
    }
}

#[derive(Debug, Deserialize)]
struct OidcClaims {
    iss: String,
    aud: AudienceClaim,
    exp: u64,
    nbf: Option<u64>,
    iat: Option<u64>,
    jti: String,
    repository: String,
    #[serde(deserialize_with = "deserialize_u64_from_string_or_number")]
    repository_id: u64,
    event_name: String,
    #[serde(rename = "ref")]
    ref_name: String,
    workflow_ref: Option<String>,
    run_id: Option<serde_json::Value>,
    run_attempt: Option<serde_json::Value>,
    repository_visibility: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GitHubRepoResponse {
    private: bool,
}

/// Establishes a connection to the Postgres database via Hyperdrive
async fn connect_to_db(env: &Env) -> Result<tokio_postgres::Client> {
    let hyperdrive = env.hyperdrive("HYPERDRIVE")?;

    let socket = Socket::builder()
        .secure_transport(SecureTransport::StartTls)
        .connect(hyperdrive.host(), hyperdrive.port())?;

    let config = hyperdrive
        .connection_string()
        .parse::<tokio_postgres::Config>()
        .map_err(|e| Error::RustError(format!("Failed to parse connection string: {}", e)))?;

    let (client, connection) = config
        .connect_raw(socket, tokio_postgres::NoTls)
        .await
        .map_err(|e| Error::RustError(format!("Failed to connect to database: {}", e)))?;

    wasm_bindgen_futures::spawn_local(async move {
        if let Err(error) = connection.await {
            console_log!("Database connection error: {:?}", error);
        }
    });

    Ok(client)
}

fn now_unix_secs() -> u64 {
    (js_sys::Date::now() / 1000.0) as u64
}

fn now_iso_timestamp() -> String {
    js_sys::Date::new_0()
        .to_iso_string()
        .as_string()
        .unwrap_or_else(|| "1970-01-01T00:00:00.000Z".to_string())
}

fn json_response<T: Serialize>(status: u16, value: &T) -> Result<Response> {
    ResponseBuilder::new().with_status(status).from_json(value)
}

fn error_response(status: u16, code: &'static str, message: impl Into<String>) -> Result<Response> {
    let payload = ApiErrorResponse {
        ok: false,
        code: code.to_string(),
        error: message.into(),
    };
    json_response(status, &payload)
}

fn to_worker_error(err: ApiError) -> Result<Response> {
    error_response(err.status, err.code, err.message)
}

fn decode_base64url(input: &str) -> std::result::Result<Vec<u8>, ApiError> {
    URL_SAFE_NO_PAD.decode(input).map_err(|_| {
        ApiError::new(
            401,
            "invalid_token",
            "Failed to decode JWT part as base64url",
        )
    })
}

fn parse_bool_field(value: Option<String>) -> bool {
    matches!(
        value.as_deref(),
        Some("1") | Some("true") | Some("TRUE") | Some("True") | Some("yes") | Some("on")
    )
}

fn js_value_to_string(value: JsValue) -> String {
    value
        .as_string()
        .unwrap_or_else(|| "JavaScript error".to_string())
}

fn set_js_property(
    target: &Object,
    name: &str,
    value: &JsValue,
) -> std::result::Result<(), ApiError> {
    Reflect::set(target.as_ref(), &JsValue::from_str(name), value)
        .map_err(|e| ApiError::new(500, "crypto_setup_failed", js_value_to_string(e)))?;
    Ok(())
}

fn get_function(target: &JsValue, name: &str) -> std::result::Result<Function, ApiError> {
    Reflect::get(target, &JsValue::from_str(name))
        .map_err(|e| ApiError::new(500, "crypto_setup_failed", js_value_to_string(e)))?
        .dyn_into::<Function>()
        .map_err(|_| ApiError::new(500, "crypto_setup_failed", format!("Missing {}", name)))
}

async fn verify_rs256_signature_with_webcrypto(
    signing_input: &[u8],
    signature: &[u8],
    modulus_b64url: &str,
    exponent_b64url: &str,
) -> std::result::Result<bool, ApiError> {
    let global = js_sys::global();
    let crypto = Reflect::get(&global, &JsValue::from_str("crypto"))
        .map_err(|e| ApiError::new(500, "crypto_setup_failed", js_value_to_string(e)))?;
    let subtle = Reflect::get(&crypto, &JsValue::from_str("subtle"))
        .map_err(|e| ApiError::new(500, "crypto_setup_failed", js_value_to_string(e)))?;

    let import_key = get_function(&subtle, "importKey")?;
    let verify = get_function(&subtle, "verify")?;

    let jwk = Object::new();
    set_js_property(&jwk, "kty", &JsValue::from_str("RSA"))?;
    set_js_property(&jwk, "n", &JsValue::from_str(modulus_b64url))?;
    set_js_property(&jwk, "e", &JsValue::from_str(exponent_b64url))?;
    set_js_property(&jwk, "alg", &JsValue::from_str("RS256"))?;
    set_js_property(&jwk, "ext", &JsValue::TRUE)?;

    let key_ops = Array::new();
    key_ops.push(&JsValue::from_str("verify"));
    set_js_property(&jwk, "key_ops", key_ops.as_ref())?;

    let import_algorithm = Object::new();
    set_js_property(
        &import_algorithm,
        "name",
        &JsValue::from_str("RSASSA-PKCS1-v1_5"),
    )?;
    let hash_algorithm = Object::new();
    set_js_property(&hash_algorithm, "name", &JsValue::from_str("SHA-256"))?;
    set_js_property(&import_algorithm, "hash", hash_algorithm.as_ref())?;

    let key_promise = import_key
        .call5(
            &subtle,
            &JsValue::from_str("jwk"),
            jwk.as_ref(),
            import_algorithm.as_ref(),
            &JsValue::FALSE,
            key_ops.as_ref(),
        )
        .map_err(|e| ApiError::new(401, "invalid_token", js_value_to_string(e)))?;
    let crypto_key = JsFuture::from(js_sys::Promise::from(key_promise))
        .await
        .map_err(|e| ApiError::new(401, "invalid_token", js_value_to_string(e)))?;

    let signature_array = Uint8Array::new_with_length(signature.len() as u32);
    signature_array.copy_from(signature);
    let data_array = Uint8Array::new_with_length(signing_input.len() as u32);
    data_array.copy_from(signing_input);

    let verify_algorithm = Object::new();
    set_js_property(
        &verify_algorithm,
        "name",
        &JsValue::from_str("RSASSA-PKCS1-v1_5"),
    )?;

    let verify_promise = verify
        .call4(
            &subtle,
            verify_algorithm.as_ref(),
            &crypto_key,
            signature_array.as_ref(),
            data_array.as_ref(),
        )
        .map_err(|e| ApiError::new(401, "invalid_token", js_value_to_string(e)))?;

    let verify_result = JsFuture::from(js_sys::Promise::from(verify_promise))
        .await
        .map_err(|e| ApiError::new(401, "invalid_token", js_value_to_string(e)))?;

    Ok(verify_result.as_bool().unwrap_or(false))
}

fn extract_bearer_token(req: &Request) -> std::result::Result<String, ApiError> {
    let auth_header = req
        .headers()
        .get("Authorization")
        .map_err(|_| ApiError::new(401, "missing_auth", "Missing Authorization header"))?
        .ok_or_else(|| ApiError::new(401, "missing_auth", "Missing Authorization header"))?;

    let mut parts = auth_header.splitn(2, ' ');
    let scheme = parts.next().unwrap_or_default();
    let token = parts.next().unwrap_or_default();

    if !scheme.eq_ignore_ascii_case("bearer") || token.is_empty() {
        return Err(ApiError::new(
            401,
            "invalid_auth",
            "Authorization header must be a bearer token",
        ));
    }

    Ok(token.to_string())
}

async fn fetch_json<T: DeserializeOwned>(
    url: &str,
    headers: &[(&str, &str)],
) -> std::result::Result<T, ApiError> {
    let mut request = Request::new(url, Method::Get)
        .map_err(|e| ApiError::new(500, "request_build_failed", format!("{}", e)))?;

    {
        let request_headers = request
            .headers_mut()
            .map_err(|e| ApiError::new(500, "request_header_failed", format!("{}", e)))?;
        for (name, value) in headers {
            request_headers
                .set(name, value)
                .map_err(|e| ApiError::new(500, "request_header_failed", format!("{}", e)))?;
        }
    }

    let mut response = Fetch::Request(request)
        .send()
        .await
        .map_err(|e| ApiError::new(502, "upstream_fetch_failed", format!("{}", e)))?;

    if response.status_code() < 200 || response.status_code() >= 300 {
        return Err(ApiError::new(
            502,
            "upstream_bad_status",
            format!("Upstream returned status {}", response.status_code()),
        ));
    }

    response.json::<T>().await.map_err(|_| {
        ApiError::new(
            502,
            "upstream_invalid_json",
            "Upstream returned invalid JSON",
        )
    })
}

async fn fetch_oidc_configuration() -> std::result::Result<OpenIdConfiguration, ApiError> {
    let config: OpenIdConfiguration =
        fetch_json(OIDC_CONFIG_URL, &[("User-Agent", "bayes-engine-ci-upload")]).await?;

    if config.issuer != EXPECTED_OIDC_ISSUER {
        return Err(ApiError::new(
            502,
            "upstream_invalid_issuer",
            "GitHub OIDC issuer mismatch",
        ));
    }

    Ok(config)
}

async fn verify_and_decode_oidc_token(token: &str) -> std::result::Result<OidcClaims, ApiError> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return Err(ApiError::new(
            401,
            "invalid_token",
            "JWT must contain 3 parts",
        ));
    }

    let header_bytes = decode_base64url(parts[0])?;
    let payload_bytes = decode_base64url(parts[1])?;
    let signature_bytes = decode_base64url(parts[2])?;

    let header: JwtHeader = serde_json::from_slice(&header_bytes)
        .map_err(|_| ApiError::new(401, "invalid_token", "Invalid JWT header JSON"))?;

    let alg = header
        .alg
        .ok_or_else(|| ApiError::new(401, "invalid_token", "JWT header missing alg"))?;
    if alg != "RS256" {
        return Err(ApiError::new(
            401,
            "invalid_token",
            format!("Unsupported JWT alg: {}", alg),
        ));
    }

    let kid = header
        .kid
        .ok_or_else(|| ApiError::new(401, "invalid_token", "JWT header missing kid"))?;

    let oidc_config = fetch_oidc_configuration().await?;
    let jwks: JwkSet = fetch_json(
        &oidc_config.jwks_uri,
        &[("User-Agent", "bayes-engine-ci-upload")],
    )
    .await?;

    let jwk = jwks
        .keys
        .iter()
        .find(|key| key.kid.as_deref() == Some(kid.as_str()))
        .ok_or_else(|| ApiError::new(401, "invalid_token", "No matching JWK for JWT kid"))?;

    if jwk.kty != "RSA" {
        return Err(ApiError::new(
            401,
            "invalid_token",
            "JWK key type is not RSA",
        ));
    }
    if let Some(jwk_alg) = &jwk.alg {
        if jwk_alg != "RS256" {
            return Err(ApiError::new(
                401,
                "invalid_token",
                "JWK algorithm is not RS256",
            ));
        }
    }

    let n = jwk
        .n
        .as_ref()
        .ok_or_else(|| ApiError::new(401, "invalid_token", "JWK missing modulus"))?;
    let e = jwk
        .e
        .as_ref()
        .ok_or_else(|| ApiError::new(401, "invalid_token", "JWK missing exponent"))?;
    let signing_input = format!("{}.{}", parts[0], parts[1]);
    let signature_valid =
        verify_rs256_signature_with_webcrypto(signing_input.as_bytes(), &signature_bytes, n, e)
            .await?;
    if !signature_valid {
        return Err(ApiError::new(
            401,
            "invalid_token",
            "JWT signature verification failed",
        ));
    }

    let claims: OidcClaims = serde_json::from_slice(&payload_bytes)
        .map_err(|_| ApiError::new(401, "invalid_token", "Invalid JWT payload JSON"))?;

    if claims.iss != EXPECTED_OIDC_ISSUER {
        return Err(ApiError::new(
            401,
            "invalid_token",
            "JWT issuer is not GitHub OIDC",
        ));
    }

    if !claims.aud.contains(EXPECTED_OIDC_AUDIENCE) {
        return Err(ApiError::new(
            401,
            "invalid_token",
            "JWT audience does not include expected audience",
        ));
    }

    let now = now_unix_secs();
    if now > claims.exp.saturating_add(60) {
        return Err(ApiError::new(401, "token_expired", "JWT has expired"));
    }
    if let Some(nbf) = claims.nbf {
        if now + 60 < nbf {
            return Err(ApiError::new(
                401,
                "invalid_token",
                "JWT not yet valid (nbf)",
            ));
        }
    }
    if let Some(iat) = claims.iat {
        if iat > now + 60 {
            return Err(ApiError::new(
                401,
                "invalid_token",
                "JWT issued-at time is in the future",
            ));
        }
    }

    Ok(claims)
}

async fn check_repository_public(repository_id: u64) -> std::result::Result<(), ApiError> {
    let endpoint = format!("{}/repositories/{}", GITHUB_API_BASE, repository_id);
    let repo: GitHubRepoResponse = fetch_json(
        &endpoint,
        &[
            ("User-Agent", "bayes-engine-ci-upload"),
            ("Accept", "application/vnd.github+json"),
        ],
    )
    .await?;

    if repo.private {
        return Err(ApiError::new(
            403,
            "private_repo_not_allowed",
            "CI uploads only accept public repositories",
        ));
    }

    Ok(())
}

fn normalize_optional_value(value: &Option<serde_json::Value>) -> Option<String> {
    match value {
        None => None,
        Some(serde_json::Value::String(s)) => Some(s.clone()),
        Some(serde_json::Value::Number(n)) => Some(n.to_string()),
        Some(serde_json::Value::Bool(b)) => Some(b.to_string()),
        _ => None,
    }
}

async fn check_and_mark_replay(env: &Env, jti: &str) -> std::result::Result<bool, ApiError> {
    let key_hash = Sha256::digest(jti.as_bytes());
    let replay_key = format!("ci-jti:{}", hex::encode(key_hash));

    if let Ok(kv) = env.kv("CI_REPLAY") {
        let existing = kv
            .get(&replay_key)
            .text()
            .await
            .map_err(|e| ApiError::new(500, "kv_read_failed", format!("{:?}", e)))?;

        if existing.is_some() {
            return Ok(true);
        }

        kv.put(&replay_key, "1")
            .map_err(|e| ApiError::new(500, "kv_write_failed", format!("{:?}", e)))?
            .expiration_ttl(REPLAY_TTL_SECS)
            .execute()
            .await
            .map_err(|e| ApiError::new(500, "kv_write_failed", format!("{:?}", e)))?;

        return Ok(false);
    }

    let now = now_unix_secs();
    let mut entries = IN_MEMORY_REPLAY
        .lock()
        .map_err(|_| ApiError::new(500, "replay_lock_failed", "Replay cache lock failed"))?;

    entries.retain(|_, expires_at| *expires_at > now);

    if let Some(expires_at) = entries.get(&replay_key) {
        if *expires_at > now {
            return Ok(true);
        }
    }

    entries.insert(replay_key, now + REPLAY_TTL_SECS);
    Ok(false)
}

fn validate_wasm(file_bytes: &[u8]) -> std::result::Result<(), ApiError> {
    if file_bytes.len() < 4 || &file_bytes[0..4] != b"\0asm" {
        return Err(ApiError::new(
            400,
            "invalid_wasm",
            "File is not a valid WASM binary (missing magic number)",
        ));
    }

    let engine = wasmi::Engine::default();
    wasmi::Module::new(&engine, file_bytes)
        .map_err(|e| ApiError::new(400, "invalid_wasm", format!("WASM parse failed: {}", e)))?;

    Ok(())
}

fn extract_u64_to_u64_function_names(
    file_bytes: &[u8],
) -> std::result::Result<Vec<String>, ApiError> {
    use wasmi::{ExternType, Module, ValType};

    let engine = wasmi::Engine::default();
    let module = Module::new(&engine, file_bytes).map_err(|e| {
        ApiError::new(
            400,
            "invalid_wasm",
            format!("Failed to parse module for exports: {}", e),
        )
    })?;

    let mut function_names = Vec::new();
    for export in module.exports() {
        let Some(func_ty) = export.ty().func() else {
            continue;
        };

        let params = func_ty.params();
        let results = func_ty.results();
        if params.len() == 1
            && results.len() == 1
            && params[0] == ValType::I64
            && results[0] == ValType::I64
        {
            function_names.push(export.name().to_string());
        } else if matches!(export.ty(), ExternType::Func(_)) {
            // keep scanning other exports
        }
    }

    if function_names.is_empty() {
        return Err(ApiError::new(
            400,
            "no_test_functions",
            "WASM module exports no u64->u64 functions",
        ));
    }

    function_names.sort();
    function_names.dedup();
    Ok(function_names)
}

async fn store_wasm_in_r2(
    env: &Env,
    r2_key: &str,
    file_bytes: &[u8],
) -> std::result::Result<(), ApiError> {
    let bucket = env.bucket("WASM_BUCKET").map_err(|_| {
        ApiError::new(
            500,
            "missing_bucket",
            "Missing R2 bucket binding WASM_BUCKET",
        )
    })?;

    bucket
        .put(r2_key, file_bytes.to_vec())
        .http_metadata(HttpMetadata {
            content_type: Some("application/wasm".to_string()),
            ..Default::default()
        })
        .execute()
        .await
        .map_err(|e| ApiError::new(500, "r2_put_failed", format!("{}", e)))?;
    Ok(())
}

async fn insert_wasm_catalog(
    client: &tokio_postgres::Client,
    repository: &str,
    version: &str,
    file_sha256: &str,
    r2_key: Option<&str>,
    function_names: &[String],
) -> std::result::Result<(i64, Vec<(i64, String)>), ApiError> {
    let repo_row = client
        .query_one(
            "
        INSERT INTO repositories (github_repo)
        VALUES ($1)
        ON CONFLICT (github_repo) DO UPDATE SET github_repo = EXCLUDED.github_repo
        RETURNING id
        ",
            &[&repository],
        )
        .await
        .map_err(|e| {
            ApiError::new(
                500,
                "db_error",
                format!("Failed upserting repository: {}", e),
            )
        })?;
    let repository_id: i64 = repo_row.get(0);

    let version_row = client
        .query_one(
            "
        INSERT INTO repository_versions (repository_id, version)
        VALUES ($1, $2)
        ON CONFLICT (repository_id, version) DO UPDATE SET version = EXCLUDED.version
        RETURNING id
        ",
            &[&repository_id, &version],
        )
        .await
        .map_err(|e| ApiError::new(500, "db_error", format!("Failed upserting version: {}", e)))?;
    let version_id: i64 = version_row.get(0);

    let file_row = client
        .query_one(
            "
        INSERT INTO wasm_files (repository_id, version_id, file_sha256, r2_key)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (repository_id, version_id, file_sha256)
        DO UPDATE SET r2_key = EXCLUDED.r2_key, uploaded_at = NOW()
        RETURNING id
        ",
            &[&repository_id, &version_id, &file_sha256, &r2_key],
        )
        .await
        .map_err(|e| {
            ApiError::new(
                500,
                "db_error",
                format!("Failed upserting wasm file: {}", e),
            )
        })?;
    let wasm_file_id: i64 = file_row.get(0);

    let mut function_rows = Vec::new();
    let default_hll = HyperLogLog::new(DEFAULT_HLL_BITS).to_json();

    for function_name in function_names {
        let row = client
            .query_one(
                "
            INSERT INTO wasm_functions (
                wasm_file_id, repository_id, version_id, function_name, hll_bits, hll_hashes_json
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (wasm_file_id, function_name)
            DO UPDATE SET function_name = EXCLUDED.function_name
            RETURNING id, function_name
            ",
                &[
                    &wasm_file_id,
                    &repository_id,
                    &version_id,
                    &function_name,
                    &(DEFAULT_HLL_BITS as i32),
                    &default_hll,
                ],
            )
            .await
            .map_err(|e| {
                ApiError::new(
                    500,
                    "db_error",
                    format!("Failed upserting function {}: {}", function_name, e),
                )
            })?;
        let function_id: i64 = row.get(0);
        let function_name: String = row.get(1);
        function_rows.push((function_id, function_name));
    }

    Ok((wasm_file_id, function_rows))
}

fn parse_u64_string(value: &str, field_name: &'static str) -> std::result::Result<u64, ApiError> {
    value.parse::<u64>().map_err(|_| {
        ApiError::new(
            400,
            "invalid_number",
            format!("{} must be an unsigned 64-bit integer", field_name),
        )
    })
}

fn build_repository_detail(
    rows: &[tokio_postgres::Row],
    repository: &str,
) -> RepositoryDetailResponse {
    let mut versions_map: BTreeMap<String, VersionSummary> = BTreeMap::new();
    let mut latest_version: Option<String> = None;
    let mut latest_uploaded_at = String::new();
    let mut total_estimated_tests = 0.0;
    let mut submitted_updates = 0i64;

    for row in rows {
        let version: String = row.get("version");
        let uploaded_at: String = row.get("uploaded_at");
        let file_id: i64 = row.get("file_id");
        let file_sha256: String = row.get("file_sha256");
        let r2_key: Option<String> = row.get("r2_key");
        let function_id: Option<i64> = row.get("function_id");
        let function_name: Option<String> = row.get("function_name");
        let hll_bits: Option<i32> = row.get("hll_bits");
        let hll_hashes_json: Option<String> = row.get("hll_hashes_json");
        let function_updates: Option<i64> = row.get("submitted_updates");
        let lowest_hash: Option<String> = row.get("lowest_hash");

        let version_entry = versions_map
            .entry(version.clone())
            .or_insert_with(|| VersionSummary {
                version: version.clone(),
                is_latest: false,
                estimated_tests: 0.0,
                submitted_updates: 0,
                file_count: 0,
                function_count: 0,
                files: Vec::new(),
            });

        if version_entry.files.iter().all(|file| file.id != file_id) {
            version_entry.file_count += 1;
            version_entry.files.push(WasmFileSummary {
                id: file_id,
                sha256: file_sha256,
                r2_key,
                uploaded_at: uploaded_at.clone(),
                functions: Vec::new(),
            });
        }

        if uploaded_at > latest_uploaded_at {
            latest_uploaded_at = uploaded_at.clone();
            latest_version = Some(version.clone());
        }

        if let (Some(fid), Some(name), Some(bits), Some(hll_json)) =
            (function_id, function_name, hll_bits, hll_hashes_json)
        {
            if let Some(file) = version_entry
                .files
                .iter_mut()
                .find(|file| file.id == file_id)
            {
                if file.functions.iter().all(|f| f.id != fid) {
                    let hll = HyperLogLog::from_json(bits as u8, &hll_json);
                    let estimate = hll.count();
                    let updates = function_updates.unwrap_or(0);
                    file.functions.push(FunctionSummary {
                        id: fid,
                        wasm_file_id: file_id,
                        name,
                        estimated_tests: estimate,
                        submitted_updates: updates,
                        lowest_hash,
                    });
                    version_entry.function_count += 1;
                    version_entry.estimated_tests += estimate;
                    version_entry.submitted_updates += updates;
                    total_estimated_tests += estimate;
                    submitted_updates += updates;
                }
            }
        }
    }

    let latest_version_value = latest_version.clone();
    let mut versions: Vec<VersionSummary> = versions_map.into_values().collect();
    for version in &mut versions {
        if Some(version.version.clone()) == latest_version_value {
            version.is_latest = true;
        }
    }
    versions.sort_by(|a, b| b.version.cmp(&a.version));

    let latest_estimated_tests = versions
        .iter()
        .find(|version| version.is_latest)
        .map(|version| version.estimated_tests)
        .unwrap_or(0.0);

    RepositoryDetailResponse {
        repository: repository.to_string(),
        latest_version,
        total_estimated_tests,
        latest_estimated_tests,
        submitted_updates,
        versions,
    }
}

async fn handle_list_repositories(env: Env) -> Result<Response> {
    let client = connect_to_db(&env).await?;

    let rows = client
        .query(
            "
        SELECT
            r.github_repo,
            rv.version,
            to_char(wf.uploaded_at, 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS uploaded_at,
            wf.id AS file_id,
            wf.file_sha256,
            to_jsonb(wf)->>'r2_key' AS r2_key,
            f.id AS function_id,
            f.function_name,
            f.hll_bits,
            f.hll_hashes_json,
            NULLIF(to_jsonb(f)->>'submitted_updates', '')::BIGINT AS submitted_updates,
            to_jsonb(f)->>'lowest_hash' AS lowest_hash
        FROM repositories r
        JOIN repository_versions rv ON rv.repository_id = r.id
        JOIN wasm_files wf ON wf.repository_id = r.id AND wf.version_id = rv.id
        LEFT JOIN wasm_functions f ON f.wasm_file_id = wf.id
        ORDER BY r.github_repo, wf.uploaded_at DESC
        ",
            &[],
        )
        .await
        .map_err(|e| Error::RustError(format!("Failed querying repositories: {}", e)))?;

    let mut grouped_rows: BTreeMap<String, Vec<tokio_postgres::Row>> = BTreeMap::new();
    for row in rows {
        let repo: String = row.get("github_repo");
        grouped_rows.entry(repo).or_default().push(row);
    }

    let mut repositories = Vec::new();
    let mut total_estimated_tests = 0.0;
    let mut version_count = 0usize;
    let mut file_count = 0usize;
    let mut function_count = 0usize;

    for (repo, repo_rows) in &grouped_rows {
        let detail = build_repository_detail(repo_rows, repo);
        let summary = RepositorySummary {
            github_repo: detail.repository.clone(),
            latest_version: detail.latest_version.clone(),
            latest_estimated_tests: detail.latest_estimated_tests,
            total_estimated_tests: detail.total_estimated_tests,
            version_count: detail.versions.len(),
            file_count: detail.versions.iter().map(|v| v.file_count).sum(),
            function_count: detail.versions.iter().map(|v| v.function_count).sum(),
            submitted_updates: detail.submitted_updates,
        };

        total_estimated_tests += summary.total_estimated_tests;
        version_count += summary.version_count;
        file_count += summary.file_count;
        function_count += summary.function_count;
        repositories.push(summary);
    }

    repositories.sort_by(|a, b| {
        b.latest_estimated_tests
            .partial_cmp(&a.latest_estimated_tests)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    json_response(
        200,
        &RepositoryListResponse {
            total_estimated_tests,
            repository_count: repositories.len(),
            version_count,
            file_count,
            function_count,
            repositories,
        },
    )
}

async fn handle_repository_detail(env: Env, repository: String) -> Result<Response> {
    let client = connect_to_db(&env).await?;

    let rows = client
        .query(
            "
        SELECT
            r.github_repo,
            rv.version,
            to_char(wf.uploaded_at, 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS uploaded_at,
            wf.id AS file_id,
            wf.file_sha256,
            to_jsonb(wf)->>'r2_key' AS r2_key,
            f.id AS function_id,
            f.function_name,
            f.hll_bits,
            f.hll_hashes_json,
            NULLIF(to_jsonb(f)->>'submitted_updates', '')::BIGINT AS submitted_updates,
            to_jsonb(f)->>'lowest_hash' AS lowest_hash
        FROM repositories r
        JOIN repository_versions rv ON rv.repository_id = r.id
        JOIN wasm_files wf ON wf.repository_id = r.id AND wf.version_id = rv.id
        LEFT JOIN wasm_functions f ON f.wasm_file_id = wf.id
        WHERE r.github_repo = $1
        ORDER BY wf.uploaded_at DESC
        ",
            &[&repository],
        )
        .await
        .map_err(|e| Error::RustError(format!("Failed querying repository details: {}", e)))?;

    if rows.is_empty() {
        return error_response(404, "not_found", "Repository not found");
    }

    let detail = build_repository_detail(&rows, &repository);
    json_response(200, &detail)
}

async fn handle_latest_catalog(env: Env, repository: String) -> Result<Response> {
    let detail_response = handle_repository_detail(env, repository.clone()).await?;
    if detail_response.status_code() != 200 {
        return Ok(detail_response);
    }
    let mut detail_response = detail_response;
    let detail: RepositoryDetailResponse = detail_response
        .json()
        .await
        .map_err(|e| Error::RustError(format!("Failed decoding detail JSON: {}", e)))?;
    let Some(latest_version) = detail.latest_version.clone() else {
        return error_response(404, "no_versions", "Repository has no uploaded versions");
    };

    let files = detail
        .versions
        .iter()
        .find(|version| version.version == latest_version)
        .map(|version| version.files.clone())
        .unwrap_or_default();

    json_response(
        200,
        &UploadCatalogResponse {
            repository,
            version: latest_version,
            files,
        },
    )
}

async fn handle_get_wasm_file(env: Env, wasm_file_id: i64) -> Result<Response> {
    let client = connect_to_db(&env).await?;

    let row = client
        .query_opt(
            "SELECT to_jsonb(wasm_files)->>'r2_key' AS r2_key FROM wasm_files WHERE id = $1",
            &[&wasm_file_id],
        )
        .await
        .map_err(|e| Error::RustError(format!("Failed querying wasm file metadata: {}", e)))?;

    let Some(row) = row else {
        return error_response(404, "not_found", "WASM file not found");
    };
    let r2_key: Option<String> = row.get(0);
    let Some(r2_key) = r2_key else {
        return error_response(404, "missing_object", "WASM file has no persisted object");
    };

    let bucket = env
        .bucket("WASM_BUCKET")
        .map_err(|_| Error::RustError("Missing R2 bucket binding WASM_BUCKET".to_string()))?;
    let object = bucket
        .get(r2_key)
        .execute()
        .await
        .map_err(|e| Error::RustError(format!("Failed reading object from R2: {}", e)))?;
    let Some(object) = object else {
        return error_response(404, "missing_object", "R2 object not found");
    };

    let body = object
        .body()
        .ok_or_else(|| Error::RustError("R2 object had no body".to_string()))?;
    let bytes = body
        .bytes()
        .await
        .map_err(|e| Error::RustError(format!("Failed reading R2 object body: {}", e)))?;
    ResponseBuilder::new()
        .with_header("Content-Type", "application/wasm")?
        .from_bytes(bytes)
}

async fn handle_get_wasm_file_hll_state(env: Env, wasm_file_id: i64) -> Result<Response> {
    let client = connect_to_db(&env).await?;

    let rows = client
        .query(
            "
        SELECT
            id,
            function_name,
            hll_bits,
            hll_hashes_json
        FROM wasm_functions
        WHERE wasm_file_id = $1
        ORDER BY id ASC
        ",
            &[&wasm_file_id],
        )
        .await
        .map_err(|e| Error::RustError(format!("Failed querying wasm function HLL state: {}", e)))?;

    if rows.is_empty() {
        return error_response(404, "not_found", "WASM file has no registered functions");
    }

    let mut functions = Vec::with_capacity(rows.len());
    for row in rows {
        let function_id: i64 = row.get("id");
        let function_name: String = row.get("function_name");
        let hll_bits: i32 = row.get("hll_bits");
        let hll_hashes_json: String = row.get("hll_hashes_json");
        let hll = HyperLogLog::from_json(hll_bits as u8, &hll_hashes_json);
        let hashes = hll.hashes().iter().map(|v| v.to_string()).collect();
        functions.push(FunctionHllStateResponse {
            function_id,
            function_name,
            hll_bits: hll_bits as u8,
            hashes,
        });
    }

    json_response(
        200,
        &WasmFileHllStateResponse {
            wasm_file_id,
            functions,
        },
    )
}

async fn handle_submit_test_result(mut req: Request, env: Env) -> Result<Response> {
    let body: SubmitHashRequest = req
        .json()
        .await
        .map_err(|e| Error::RustError(format!("Invalid JSON body: {}", e)))?;
    let seed = match parse_u64_string(&body.seed, "seed") {
        Ok(value) => value,
        Err(err) => return to_worker_error(err),
    };
    let hash = match parse_u64_string(&body.hash, "hash") {
        Ok(value) => value,
        Err(err) => return to_worker_error(err),
    };

    let client = connect_to_db(&env).await?;

    let mut function_id = body.function_id;
    let mut row = client
        .query_opt(
            "
        SELECT
            id,
            hll_bits,
            hll_hashes_json,
            COALESCE(NULLIF(to_jsonb(wasm_functions)->>'submitted_updates', '')::BIGINT, 0) AS submitted_updates,
            to_jsonb(wasm_functions)->>'lowest_hash' AS lowest_hash,
            to_jsonb(wasm_functions)->>'lowest_seed' AS lowest_seed
        FROM wasm_functions
        WHERE id = $1
        ",
            &[&function_id],
        )
        .await
        .map_err(|e| Error::RustError(format!("Failed querying function HLL: {}", e)))?;

    if row.is_none() {
        if let (Some(wasm_file_id), Some(function_name)) =
            (body.wasm_file_id, body.function_name.as_ref())
        {
            let name = function_name.trim();
            if !name.is_empty() {
                let file_row = client
                    .query_opt(
                        "SELECT repository_id, version_id FROM wasm_files WHERE id = $1",
                        &[&wasm_file_id],
                    )
                    .await
                    .map_err(|e| {
                        Error::RustError(format!("Failed querying wasm file metadata: {}", e))
                    })?;

                if let Some(file_row) = file_row {
                    let repository_id: i64 = file_row.get("repository_id");
                    let version_id: i64 = file_row.get("version_id");
                    let default_hll = HyperLogLog::new(DEFAULT_HLL_BITS).to_json();

                    let upsert_row = client
                        .query_one(
                            "
                        INSERT INTO wasm_functions (
                            wasm_file_id, repository_id, version_id, function_name, hll_bits, hll_hashes_json
                        )
                        VALUES ($1, $2, $3, $4, $5, $6)
                        ON CONFLICT (wasm_file_id, function_name)
                        DO UPDATE SET function_name = EXCLUDED.function_name
                        RETURNING id
                        ",
                            &[
                                &wasm_file_id,
                                &repository_id,
                                &version_id,
                                &name,
                                &(DEFAULT_HLL_BITS as i32),
                                &default_hll,
                            ],
                        )
                        .await
                        .map_err(|e| {
                            Error::RustError(format!("Failed upserting function metadata: {}", e))
                        })?;

                    function_id = upsert_row.get(0);
                    row = client
                        .query_opt(
                            "
                        SELECT
                            id,
                            hll_bits,
                            hll_hashes_json,
                            COALESCE(NULLIF(to_jsonb(wasm_functions)->>'submitted_updates', '')::BIGINT, 0) AS submitted_updates,
                            to_jsonb(wasm_functions)->>'lowest_hash' AS lowest_hash,
                            to_jsonb(wasm_functions)->>'lowest_seed' AS lowest_seed
                        FROM wasm_functions
                        WHERE id = $1
                        ",
                            &[&function_id],
                        )
                        .await
                        .map_err(|e| {
                            Error::RustError(format!("Failed querying function HLL: {}", e))
                        })?;
                }
            }
        }
    }

    let Some(row) = row else {
        return error_response(404, "function_not_found", "Function ID not found");
    };

    let hll_bits: i32 = row.get("hll_bits");
    let hll_bits = u8::try_from(hll_bits).unwrap_or(DEFAULT_HLL_BITS);
    let hll_json: String = row.get("hll_hashes_json");
    let submitted_updates: i64 = row.get("submitted_updates");
    let current_lowest_hash: Option<String> = row.get("lowest_hash");
    let current_lowest_seed: Option<String> = row.get("lowest_seed");

    let mut hll = HyperLogLog::from_json(hll_bits, &hll_json);
    let improved = hll.add_hash(hash);
    if improved {
        let new_hll_json = hll.to_json();
        let next_updates = submitted_updates + 1;
        let (lowest_hash, lowest_seed) = if let Some(existing) = current_lowest_hash {
            let existing_hash = existing.parse::<u64>().unwrap_or(u64::MAX);
            if hash < existing_hash {
                (hash.to_string(), seed.to_string())
            } else {
                (
                    existing,
                    current_lowest_seed.unwrap_or_else(|| seed.to_string()),
                )
            }
        } else {
            (hash.to_string(), seed.to_string())
        };
        let rich_update = client
            .execute(
                "
            UPDATE wasm_functions
            SET
                hll_hashes_json = $1,
                submitted_updates = $2,
                lowest_hash = $3,
                lowest_seed = $4,
                updated_at = NOW()
            WHERE id = $5
            ",
                &[
                    &new_hll_json,
                    &next_updates,
                    &lowest_hash,
                    &lowest_seed,
                    &function_id,
                ],
            )
            .await;

        if rich_update.is_err() {
            client
                .execute(
                    "UPDATE wasm_functions SET hll_hashes_json = $1 WHERE id = $2",
                    &[&new_hll_json, &function_id],
                )
                .await
                .map_err(|e| {
                    Error::RustError(format!(
                        "Failed updating function HLL (fallback mode): {}",
                        e
                    ))
                })?;
        }
    }

    json_response(
        200,
        &SubmitHashResponse {
            ok: true,
            improved,
            estimated_tests: hll.count(),
            submitted_updates: if improved {
                submitted_updates + 1
            } else {
                submitted_updates
            },
        },
    )
}

async fn handle_ci_upload(mut req: Request, env: Env) -> Result<Response> {
    let token = match extract_bearer_token(&req) {
        Ok(token) => token,
        Err(err) => return to_worker_error(err),
    };

    let claims = match verify_and_decode_oidc_token(&token).await {
        Ok(claims) => claims,
        Err(err) => return to_worker_error(err),
    };

    let form = match req.form_data().await {
        Ok(form) => form,
        Err(_) => {
            return error_response(400, "invalid_form", "Expected multipart/form-data request");
        }
    };

    let dry_run = parse_bool_field(form.get_field("dry_run"));

    let event_allowed = claims.event_name == "push"
        || claims.event_name == "workflow_dispatch"
        || (claims.event_name == "pull_request" && dry_run);
    if !event_allowed {
        return error_response(
            403,
            "event_not_allowed",
            "Only push/workflow_dispatch events are accepted, or pull_request when dry_run=true",
        );
    }

    if claims.event_name == "pull_request" && !dry_run {
        return error_response(
            403,
            "dry_run_required",
            "pull_request uploads must set dry_run=true",
        );
    }

    if let Some(visibility) = &claims.repository_visibility {
        if visibility != "public" {
            return error_response(
                403,
                "private_repo_not_allowed",
                "CI uploads only accept public repositories",
            );
        }
    } else if let Err(err) = check_repository_public(claims.repository_id).await {
        return to_worker_error(err);
    }

    match check_and_mark_replay(&env, &claims.jti).await {
        Ok(true) => {
            return error_response(409, "replay_detected", "OIDC token jti already used");
        }
        Ok(false) => {}
        Err(err) => return to_worker_error(err),
    }

    let file_bytes = match form.get("file") {
        Some(FormEntry::File(file)) => match file.bytes().await {
            Ok(bytes) => bytes,
            Err(_) => return error_response(400, "invalid_file", "Failed reading file bytes"),
        },
        Some(FormEntry::Field(_)) => {
            return error_response(400, "invalid_file", "`file` must be a file upload part");
        }
        None => return error_response(400, "missing_file", "Missing `file` multipart part"),
    };

    if file_bytes.is_empty() {
        return error_response(400, "invalid_file", "Uploaded file is empty");
    }

    if file_bytes.len() > MAX_UPLOAD_BYTES {
        return error_response(
            400,
            "file_too_large",
            format!(
                "Uploaded file exceeds max size of {} bytes",
                MAX_UPLOAD_BYTES
            ),
        );
    }

    if let Err(err) = validate_wasm(&file_bytes) {
        return to_worker_error(err);
    }

    let wasm_sha256 = hex::encode(Sha256::digest(&file_bytes));
    let version = form
        .get_field("version")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| claims.ref_name.clone());

    if let Some(declared_sha256) = form.get_field("declared_sha256") {
        let normalized = declared_sha256.trim().to_ascii_lowercase();
        if normalized.len() != 64 || !normalized.chars().all(|c| c.is_ascii_hexdigit()) {
            return error_response(
                400,
                "invalid_declared_sha256",
                "declared_sha256 must be a 64-char lowercase hex string",
            );
        }

        if normalized != wasm_sha256 {
            return error_response(
                400,
                "sha256_mismatch",
                "declared_sha256 does not match uploaded file",
            );
        }
    }

    let function_names = match extract_u64_to_u64_function_names(&file_bytes) {
        Ok(names) => names,
        Err(err) => return to_worker_error(err),
    };

    let mut persisted = false;
    let mut r2_key = None;
    let mut wasm_file_id = None;

    if !dry_run {
        let client = match connect_to_db(&env).await {
            Ok(client) => client,
            Err(e) => return Response::error(format!("Failed to connect to database: {}", e), 500),
        };

        let storage_key = format!(
            "{}/{}/{}.wasm",
            claims.repository.replace('/', "__"),
            version.replace('/', "_"),
            wasm_sha256
        );
        if let Err(err) = store_wasm_in_r2(&env, &storage_key, &file_bytes).await {
            return to_worker_error(err);
        }
        persisted = true;
        r2_key = Some(storage_key);

        wasm_file_id = match insert_wasm_catalog(
            &client,
            &claims.repository,
            &version,
            &wasm_sha256,
            r2_key.as_deref(),
            &function_names,
        )
        .await
        {
            Ok((file_id, _)) => Some(file_id),
            Err(err) => return to_worker_error(err),
        };
    }

    let payload = CiUploadResponse {
        ok: true,
        wasm_sha256,
        wasm_size_bytes: file_bytes.len(),
        wasm_valid: true,
        dry_run,
        persisted,
        repository: claims.repository,
        repository_id: claims.repository_id,
        run_id: normalize_optional_value(&claims.run_id),
        run_attempt: normalize_optional_value(&claims.run_attempt),
        event_name: claims.event_name,
        r#ref: claims.ref_name,
        workflow_ref: claims.workflow_ref,
        received_at: now_iso_timestamp(),
        repository_version: version,
        function_count: function_names.len(),
        function_names,
        wasm_file_id,
        r2_key,
    };

    json_response(200, &payload)
}

fn deserialize_u64_from_string_or_number<'de, D>(
    deserializer: D,
) -> std::result::Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    match value {
        serde_json::Value::Number(number) => number
            .as_u64()
            .ok_or_else(|| serde::de::Error::custom("repository_id number must be unsigned")),
        serde_json::Value::String(string) => string.parse::<u64>().map_err(|_| {
            serde::de::Error::custom("repository_id string must be a valid unsigned integer")
        }),
        _ => Err(serde::de::Error::custom(
            "repository_id must be a string or number",
        )),
    }
}

#[event(fetch)]
async fn fetch(req: Request, env: Env, _ctx: worker::Context) -> Result<Response> {
    console_error_panic_hook::set_once();

    let router = Router::new();

    router
        .post_async("/api/uppercase", |mut req, _ctx| async move {
            let body: UppercaseRequest = req.json().await?;
            let result = body.text.to_uppercase();
            let response = UppercaseResponse { result };
            Response::from_json(&response)
        })
        .get_async("/api/repositories", |_req, ctx| async move {
            match handle_list_repositories(ctx.env).await {
                Ok(response) => Ok(response),
                Err(err) => error_response(
                    500,
                    "internal_error",
                    format!("Failed listing repositories: {}", err),
                ),
            }
        })
        .get_async("/api/repositories/:owner/:repo", |_req, ctx| async move {
            let owner = ctx
                .param("owner")
                .map(|value| value.to_string())
                .unwrap_or_default();
            let repo = ctx
                .param("repo")
                .map(|value| value.to_string())
                .unwrap_or_default();
            let repository = format!("{}/{}", owner, repo);
            match handle_repository_detail(ctx.env, repository).await {
                Ok(response) => Ok(response),
                Err(err) => error_response(
                    500,
                    "internal_error",
                    format!("Failed loading repository detail: {}", err),
                ),
            }
        })
        .get_async(
            "/api/repositories/:owner/:repo/latest-catalog",
            |_req, ctx| async move {
                let owner = ctx
                    .param("owner")
                    .map(|value| value.to_string())
                    .unwrap_or_default();
                let repo = ctx
                    .param("repo")
                    .map(|value| value.to_string())
                    .unwrap_or_default();
                let repository = format!("{}/{}", owner, repo);
                match handle_latest_catalog(ctx.env, repository).await {
                    Ok(response) => Ok(response),
                    Err(err) => error_response(
                        500,
                        "internal_error",
                        format!("Failed loading latest catalog: {}", err),
                    ),
                }
            },
        )
        .get_async("/api/wasm-files/:id", |_req, ctx| async move {
            let id = match ctx.param("id").and_then(|value| value.parse::<i64>().ok()) {
                Some(value) => value,
                None => return error_response(400, "invalid_id", "Invalid wasm file id"),
            };
            handle_get_wasm_file(ctx.env, id).await
        })
        .get_async("/api/wasm-files/:id/hll-state", |_req, ctx| async move {
            let id = match ctx.param("id").and_then(|value| value.parse::<i64>().ok()) {
                Some(value) => value,
                None => return error_response(400, "invalid_id", "Invalid wasm file id"),
            };
            handle_get_wasm_file_hll_state(ctx.env, id).await
        })
        .post_async("/api/test-results", |req, ctx| async move {
            match handle_submit_test_result(req, ctx.env).await {
                Ok(response) => Ok(response),
                Err(err) => error_response(
                    500,
                    "internal_error",
                    format!("Failed submitting test result: {}", err),
                ),
            }
        })
        .post_async("/api/ci-upload", |req, ctx| async move {
            handle_ci_upload(req, ctx.env).await
        })
        .get_async("/*catchall", |_req, _ctx| async move {
            Response::from_html(include_str!("../../public/index.html"))
        })
        .run(req, env)
        .await
}
