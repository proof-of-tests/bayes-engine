use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use js_sys::{Array, Function, Object, Reflect, Uint8Array};
use once_cell::sync::Lazy;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
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

#[derive(Deserialize)]
struct MessageRequest {
    message: String,
}

#[derive(Serialize)]
struct Message {
    message: String,
}

#[derive(Serialize)]
struct MessagesResponse {
    messages: Vec<Message>,
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

    let payload = CiUploadResponse {
        ok: true,
        wasm_sha256,
        wasm_size_bytes: file_bytes.len(),
        wasm_valid: true,
        dry_run,
        persisted: false,
        repository: claims.repository,
        repository_id: claims.repository_id,
        run_id: normalize_optional_value(&claims.run_id),
        run_attempt: normalize_optional_value(&claims.run_attempt),
        event_name: claims.event_name,
        r#ref: claims.ref_name,
        workflow_ref: claims.workflow_ref,
        received_at: now_iso_timestamp(),
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
        .post_async("/api/ci-upload", |req, ctx| async move {
            handle_ci_upload(req, ctx.env).await
        })
        .get_async("/api/messages", |_req, ctx| async move {
            let env = ctx.env;

            let client = match connect_to_db(&env).await {
                Ok(client) => client,
                Err(e) => {
                    return Response::error(format!("Failed to connect to database: {}", e), 500);
                }
            };

            match client
                .query("SELECT message FROM messages ORDER BY message", &[])
                .await
            {
                Ok(rows) => {
                    let messages: Vec<Message> = rows
                        .iter()
                        .map(|row| {
                            let message: String = row.get(0);
                            Message { message }
                        })
                        .collect();

                    let response = MessagesResponse { messages };
                    Response::from_json(&response)
                }
                Err(e) => Response::error(format!("Failed to query messages: {}", e), 500),
            }
        })
        .post_async("/api/messages", |mut req, ctx| async move {
            let env = ctx.env;

            let body: MessageRequest = match req.json().await {
                Ok(b) => b,
                Err(e) => {
                    return Response::error(format!("Invalid request: {}", e), 400);
                }
            };

            let client = match connect_to_db(&env).await {
                Ok(client) => client,
                Err(e) => {
                    return Response::error(format!("Failed to connect to database: {}", e), 500);
                }
            };

            match client
                .execute(
                    "INSERT INTO messages (message) VALUES ($1)",
                    &[&body.message],
                )
                .await
            {
                Ok(_) => {
                    let response = Message {
                        message: body.message,
                    };
                    Response::from_json(&response)
                }
                Err(e) => Response::error(format!("Failed to insert message: {}", e), 500),
            }
        })
        .get_async("/*catchall", |_req, _ctx| async move {
            Response::from_html(include_str!("../../public/index.html"))
        })
        .run(req, env)
        .await
}
