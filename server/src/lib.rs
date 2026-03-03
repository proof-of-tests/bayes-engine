mod catalog;
mod hll_store;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use catalog::{FileMetadata, RepoMetadata, VersionMetadata};
use hll_store::FunctionStats;
use hyperloglog::DEFAULT_HLL_BITS;
use js_sys::{Array, Function, Object, Reflect, Uint8Array};
use serde::{Deserialize, Deserializer, Serialize};
use sha2::{Digest, Sha256};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use worker::*;

const OIDC_CONFIG_URL: &str =
    "https://token.actions.githubusercontent.com/.well-known/openid-configuration";
const EXPECTED_OIDC_ISSUER: &str = "https://token.actions.githubusercontent.com";
const EXPECTED_OIDC_AUDIENCE: &str = "bayes-engine-ci-upload";
const GITHUB_API_BASE: &str = "https://api.github.com";
const MAX_UPLOAD_BYTES: usize = 10 * 1024 * 1024;

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
    r2_key: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
struct FunctionSummary {
    r2_key: String,
    name: String,
    estimated_tests: f64,
    submitted_updates: i64,
    lowest_hash: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
struct WasmFileSummary {
    r2_key: String,
    sha256: String,
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
    r2_key: String,
    function_name: String,
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
    function_name: String,
    hll_bits: u8,
    hashes: Vec<String>,
}

#[derive(Serialize)]
struct WasmFileHllStateResponse {
    r2_key: String,
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

async fn fetch_json<T: serde::de::DeserializeOwned>(
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
    metadata: &R2WasmMetadata,
) -> std::result::Result<(), ApiError> {
    let bucket = env.bucket("WASM_BUCKET").map_err(|_| {
        ApiError::new(
            500,
            "missing_bucket",
            "Missing R2 bucket binding WASM_BUCKET",
        )
    })?;

    // Store metadata as custom metadata on the R2 object
    let metadata_json = serde_json::to_string(metadata).unwrap_or_else(|_| "{}".to_string());

    bucket
        .put(r2_key, file_bytes.to_vec())
        .http_metadata(HttpMetadata {
            content_type: Some("application/wasm".to_string()),
            ..Default::default()
        })
        .custom_metadata([("bayes-metadata".to_string(), metadata_json)])
        .execute()
        .await
        .map_err(|e| ApiError::new(500, "r2_put_failed", format!("{}", e)))?;
    Ok(())
}

#[derive(Serialize, Deserialize)]
struct R2WasmMetadata {
    repository: String,
    version: String,
    sha256: String,
    uploaded_at: String,
    functions: Vec<String>,
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

async fn handle_list_repositories(env: Env) -> Result<Response> {
    let kv = env.kv("CATALOG")?;
    let db = env.d1("HLL_DB")?;

    // Ensure schema exists
    if let Err(e) = hll_store::ensure_schema(&db).await {
        console_log!("[WARN] Failed to ensure D1 schema: {:?}", e);
    }

    let repo_names = catalog::list_repos(&kv).await?;

    let mut repositories = Vec::new();
    let mut total_estimated_tests = 0.0;
    let mut version_count = 0usize;
    let mut file_count = 0usize;
    let mut function_count = 0usize;

    for repo_name in &repo_names {
        if let Some(repo_meta) = catalog::get_repo(&kv, repo_name).await? {
            let mut repo_estimated_tests = 0.0;
            let mut repo_submitted_updates = 0i64;
            let mut repo_file_count = 0usize;
            let mut repo_function_count = 0usize;
            let mut latest_estimated_tests = 0.0;

            for version in &repo_meta.versions {
                if let Some(version_meta) = catalog::get_version(&kv, repo_name, version).await? {
                    version_count += 1;

                    for file in &version_meta.files {
                        repo_file_count += 1;
                        file_count += 1;

                        // Get HLL states for this file
                        let states = hll_store::get_file_hll_states(&db, &file.r2_key).await?;
                        for (_, hll, stats) in &states {
                            repo_function_count += 1;
                            function_count += 1;
                            let estimate = hll.count();
                            repo_estimated_tests += estimate;
                            repo_submitted_updates += stats.submitted_updates;

                            if Some(version.clone()) == repo_meta.latest_version {
                                latest_estimated_tests += estimate;
                            }
                        }
                    }
                }
            }

            total_estimated_tests += repo_estimated_tests;

            repositories.push(RepositorySummary {
                github_repo: repo_name.clone(),
                latest_version: repo_meta.latest_version.clone(),
                latest_estimated_tests,
                total_estimated_tests: repo_estimated_tests,
                version_count: repo_meta.versions.len(),
                file_count: repo_file_count,
                function_count: repo_function_count,
                submitted_updates: repo_submitted_updates,
            });
        }
    }

    // Sort by latest_estimated_tests descending
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
    let kv = env.kv("CATALOG")?;
    let db = env.d1("HLL_DB")?;

    let repo_meta = match catalog::get_repo(&kv, &repository).await? {
        Some(meta) => meta,
        None => return error_response(404, "not_found", "Repository not found"),
    };

    let mut versions = Vec::new();
    let mut total_estimated_tests = 0.0;
    let mut total_submitted_updates = 0i64;
    let mut latest_estimated_tests = 0.0;

    for version_name in &repo_meta.versions {
        if let Some(version_meta) = catalog::get_version(&kv, &repository, version_name).await? {
            let mut version_estimated_tests = 0.0;
            let mut version_submitted_updates = 0i64;
            let mut files = Vec::new();

            for file in &version_meta.files {
                let states = hll_store::get_file_hll_states(&db, &file.r2_key).await?;

                let functions: Vec<FunctionSummary> = states
                    .iter()
                    .map(|(name, hll, stats)| {
                        let estimate = hll.count();
                        version_estimated_tests += estimate;
                        version_submitted_updates += stats.submitted_updates;

                        FunctionSummary {
                            r2_key: file.r2_key.clone(),
                            name: name.clone(),
                            estimated_tests: estimate,
                            submitted_updates: stats.submitted_updates,
                            lowest_hash: stats.lowest_hash.clone(),
                        }
                    })
                    .collect();

                files.push(WasmFileSummary {
                    r2_key: file.r2_key.clone(),
                    sha256: file.sha256.clone(),
                    uploaded_at: file.uploaded_at.clone(),
                    functions,
                });
            }

            let is_latest = Some(version_name.clone()) == repo_meta.latest_version;
            if is_latest {
                latest_estimated_tests = version_estimated_tests;
            }

            total_estimated_tests += version_estimated_tests;
            total_submitted_updates += version_submitted_updates;

            versions.push(VersionSummary {
                version: version_name.clone(),
                is_latest,
                estimated_tests: version_estimated_tests,
                submitted_updates: version_submitted_updates,
                file_count: files.len(),
                function_count: files.iter().map(|f| f.functions.len()).sum(),
                files,
            });
        }
    }

    // Sort versions descending
    versions.sort_by(|a, b| b.version.cmp(&a.version));

    json_response(
        200,
        &RepositoryDetailResponse {
            repository,
            latest_version: repo_meta.latest_version,
            total_estimated_tests,
            latest_estimated_tests,
            submitted_updates: total_submitted_updates,
            versions,
        },
    )
}

async fn handle_latest_catalog(env: Env, repository: String) -> Result<Response> {
    let kv = env.kv("CATALOG")?;
    let db = env.d1("HLL_DB")?;

    let repo_meta = match catalog::get_repo(&kv, &repository).await? {
        Some(meta) => meta,
        None => return error_response(404, "not_found", "Repository not found"),
    };

    let latest_version = match &repo_meta.latest_version {
        Some(v) => v.clone(),
        None => return error_response(404, "no_versions", "Repository has no uploaded versions"),
    };

    let version_meta = match catalog::get_version(&kv, &repository, &latest_version).await? {
        Some(meta) => meta,
        None => return error_response(404, "version_not_found", "Latest version not found"),
    };

    let mut files = Vec::new();
    for file in &version_meta.files {
        let states = hll_store::get_file_hll_states(&db, &file.r2_key).await?;

        let functions: Vec<FunctionSummary> = states
            .iter()
            .map(|(name, hll, stats)| FunctionSummary {
                r2_key: file.r2_key.clone(),
                name: name.clone(),
                estimated_tests: hll.count(),
                submitted_updates: stats.submitted_updates,
                lowest_hash: stats.lowest_hash.clone(),
            })
            .collect();

        files.push(WasmFileSummary {
            r2_key: file.r2_key.clone(),
            sha256: file.sha256.clone(),
            uploaded_at: file.uploaded_at.clone(),
            functions,
        });
    }

    json_response(
        200,
        &UploadCatalogResponse {
            repository,
            version: latest_version,
            files,
        },
    )
}

async fn handle_get_wasm_file(env: Env, r2_key: String) -> Result<Response> {
    let bucket = env
        .bucket("WASM_BUCKET")
        .map_err(|_| Error::RustError("Missing R2 bucket binding WASM_BUCKET".to_string()))?;

    let object = bucket
        .get(&r2_key)
        .execute()
        .await
        .map_err(|e| Error::RustError(format!("Failed reading object from R2: {}", e)))?;

    let Some(object) = object else {
        return error_response(404, "not_found", "WASM file not found");
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

async fn handle_get_wasm_file_hll_state(env: Env, r2_key: String) -> Result<Response> {
    let db = env.d1("HLL_DB")?;

    let states = hll_store::get_file_hll_states(&db, &r2_key).await?;

    if states.is_empty() {
        return error_response(404, "not_found", "WASM file has no registered functions");
    }

    let functions: Vec<FunctionHllStateResponse> = states
        .into_iter()
        .map(|(name, hll, _)| FunctionHllStateResponse {
            function_name: name,
            hll_bits: DEFAULT_HLL_BITS,
            hashes: hll.hashes().iter().map(|v| v.to_string()).collect(),
        })
        .collect();

    json_response(200, &WasmFileHllStateResponse { r2_key, functions })
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

    let db = env.d1("HLL_DB")?;

    // Ensure schema exists
    if let Err(e) = hll_store::ensure_schema(&db).await {
        console_log!("[WARN] Failed to ensure D1 schema: {:?}", e);
    }

    // Submit the hash atomically
    let improved =
        hll_store::submit_hash(&db, &body.r2_key, &body.function_name, seed, hash).await?;

    // Get updated HLL state for the estimate
    let hll = hll_store::get_hll_state(&db, &body.r2_key, &body.function_name).await?;
    let stats = hll_store::get_function_stats(&db, &body.r2_key, &body.function_name)
        .await?
        .unwrap_or(FunctionStats {
            submitted_updates: 0,
            lowest_hash: None,
            lowest_seed: None,
        });

    json_response(
        200,
        &SubmitHashResponse {
            ok: true,
            improved,
            estimated_tests: hll.count(),
            submitted_updates: stats.submitted_updates,
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

    // Check replay protection
    let kv = env.kv("CATALOG")?;
    let jti_hash = hex::encode(Sha256::digest(claims.jti.as_bytes()));
    match catalog::check_and_mark_replay(&kv, &jti_hash).await {
        Ok(true) => {
            return error_response(409, "replay_detected", "OIDC token jti already used");
        }
        Ok(false) => {}
        Err(e) => {
            console_log!("[WARN] Replay check failed: {:?}", e);
        }
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

    if !dry_run {
        let storage_key = format!(
            "{}/{}/{}.wasm",
            claims.repository.replace('/', "__"),
            version.replace('/', "_"),
            wasm_sha256
        );

        let metadata = R2WasmMetadata {
            repository: claims.repository.clone(),
            version: version.clone(),
            sha256: wasm_sha256.clone(),
            uploaded_at: now_iso_timestamp(),
            functions: function_names.clone(),
        };

        if let Err(err) = store_wasm_in_r2(&env, &storage_key, &file_bytes, &metadata).await {
            return to_worker_error(err);
        }
        persisted = true;
        r2_key = Some(storage_key.clone());

        // Update KV catalog
        let kv = env.kv("CATALOG")?;

        // Ensure repo is in the list
        catalog::ensure_repo_in_list(&kv, &claims.repository).await?;

        // Update repo metadata
        let mut repo_meta = catalog::get_repo(&kv, &claims.repository)
            .await?
            .unwrap_or_else(|| RepoMetadata {
                github_repo: claims.repository.clone(),
                versions: Vec::new(),
                latest_version: None,
                created_at: now_iso_timestamp(),
            });

        if !repo_meta.versions.contains(&version) {
            repo_meta.versions.push(version.clone());
        }
        repo_meta.latest_version = Some(version.clone());
        catalog::put_repo(&kv, &repo_meta).await?;

        // Update version metadata
        let mut version_meta = catalog::get_version(&kv, &claims.repository, &version)
            .await?
            .unwrap_or_else(|| VersionMetadata {
                version: version.clone(),
                files: Vec::new(),
                created_at: now_iso_timestamp(),
            });

        // Check if this file already exists in the version
        if !version_meta.files.iter().any(|f| f.sha256 == wasm_sha256) {
            version_meta.files.push(FileMetadata {
                r2_key: storage_key.clone(),
                sha256: wasm_sha256.clone(),
                uploaded_at: now_iso_timestamp(),
                functions: function_names.clone(),
            });
        }
        catalog::put_version(&kv, &claims.repository, &version_meta).await?;

        // Initialize HLL registers in D1
        let db = env.d1("HLL_DB")?;
        hll_store::ensure_schema(&db).await?;

        for function_name in &function_names {
            hll_store::init_function_registers(&db, &storage_key, function_name).await?;
        }
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
        .get_async("/api/repositories", |_req, ctx| async move {
            match handle_list_repositories(ctx.env).await {
                Ok(response) => Ok(response),
                Err(err) => {
                    console_log!("[ERROR] GET /api/repositories failed: {}", err);
                    error_response(
                        500,
                        "internal_error",
                        format!("Failed listing repositories: {}", err),
                    )
                }
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
                Err(err) => {
                    console_log!("[ERROR] GET /api/repositories/:owner/:repo failed: {}", err);
                    error_response(
                        500,
                        "internal_error",
                        format!("Failed loading repository detail: {}", err),
                    )
                }
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
                    Err(err) => {
                        console_log!(
                            "[ERROR] GET /api/repositories/:owner/:repo/latest-catalog failed: {}",
                            err
                        );
                        error_response(
                            500,
                            "internal_error",
                            format!("Failed loading latest catalog: {}", err),
                        )
                    }
                }
            },
        )
        .get_async("/api/wasm/*r2_key", |_req, ctx| async move {
            let r2_key = ctx
                .param("r2_key")
                .map(|value| value.to_string())
                .unwrap_or_default();
            handle_get_wasm_file(ctx.env, r2_key).await
        })
        .get_async("/api/wasm-hll/*r2_key", |_req, ctx| async move {
            let r2_key = ctx
                .param("r2_key")
                .map(|value| value.to_string())
                .unwrap_or_default();
            handle_get_wasm_file_hll_state(ctx.env, r2_key).await
        })
        .post_async("/api/test-results", |req, ctx| async move {
            match handle_submit_test_result(req, ctx.env).await {
                Ok(response) => Ok(response),
                Err(err) => {
                    console_log!("[ERROR] POST /api/test-results failed: {}", err);
                    error_response(
                        500,
                        "internal_error",
                        format!("Failed submitting test result: {}", err),
                    )
                }
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
