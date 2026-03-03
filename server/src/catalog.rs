//! Catalog data storage using CloudFlare KV
//!
//! Stores repository metadata, version information, and file catalogs.
//! This is read-heavy data that rarely changes (only on CI uploads).

use serde::{Deserialize, Serialize};
use worker::{kv::KvStore, Result};

/// Repository metadata stored in KV
#[derive(Clone, Serialize, Deserialize)]
pub struct RepoMetadata {
    pub github_repo: String,
    pub versions: Vec<String>,
    pub latest_version: Option<String>,
    pub created_at: String,
}

/// Version metadata with file list
#[derive(Clone, Serialize, Deserialize)]
pub struct VersionMetadata {
    pub version: String,
    pub files: Vec<FileMetadata>,
    pub created_at: String,
}

/// File metadata including exported functions
#[derive(Clone, Serialize, Deserialize)]
pub struct FileMetadata {
    pub r2_key: String,
    pub sha256: String,
    pub uploaded_at: String,
    pub functions: Vec<String>,
}

/// KV key prefixes
const REPO_PREFIX: &str = "repo:";
const VERSION_PREFIX: &str = "version:";
const REPOS_LIST_KEY: &str = "repos:list";
const CI_JTI_PREFIX: &str = "ci-jti:";

/// Replay protection TTL in seconds (10 minutes)
const REPLAY_TTL_SECS: u64 = 600;

/// Get list of all repository names
pub async fn list_repos(kv: &KvStore) -> Result<Vec<String>> {
    let value = kv.get(REPOS_LIST_KEY).text().await?;
    match value {
        Some(json) => {
            let repos: Vec<String> = serde_json::from_str(&json).unwrap_or_default();
            Ok(repos)
        }
        None => Ok(Vec::new()),
    }
}

/// Get repository metadata
pub async fn get_repo(kv: &KvStore, github_repo: &str) -> Result<Option<RepoMetadata>> {
    let key = format!("{}{}", REPO_PREFIX, github_repo);
    let value = kv.get(&key).text().await?;
    match value {
        Some(json) => {
            let meta: RepoMetadata = serde_json::from_str(&json).unwrap_or_else(|_| RepoMetadata {
                github_repo: github_repo.to_string(),
                versions: Vec::new(),
                latest_version: None,
                created_at: String::new(),
            });
            Ok(Some(meta))
        }
        None => Ok(None),
    }
}

/// Get version metadata including files
pub async fn get_version(
    kv: &KvStore,
    github_repo: &str,
    version: &str,
) -> Result<Option<VersionMetadata>> {
    let key = format!("{}{}:{}", VERSION_PREFIX, github_repo, version);
    let value = kv.get(&key).text().await?;
    match value {
        Some(json) => {
            let meta: VersionMetadata =
                serde_json::from_str(&json).unwrap_or_else(|_| VersionMetadata {
                    version: version.to_string(),
                    files: Vec::new(),
                    created_at: String::new(),
                });
            Ok(Some(meta))
        }
        None => Ok(None),
    }
}

/// Store repository metadata
pub async fn put_repo(kv: &KvStore, meta: &RepoMetadata) -> Result<()> {
    let key = format!("{}{}", REPO_PREFIX, meta.github_repo);
    let json = serde_json::to_string(meta).unwrap_or_else(|_| "{}".to_string());
    kv.put(&key, json)?.execute().await?;
    Ok(())
}

/// Store version metadata
pub async fn put_version(kv: &KvStore, github_repo: &str, meta: &VersionMetadata) -> Result<()> {
    let key = format!("{}{}:{}", VERSION_PREFIX, github_repo, meta.version);
    let json = serde_json::to_string(meta).unwrap_or_else(|_| "{}".to_string());
    kv.put(&key, json)?.execute().await?;
    Ok(())
}

/// Update the global repos list
pub async fn update_repos_list(kv: &KvStore, repos: &[String]) -> Result<()> {
    let json = serde_json::to_string(repos).unwrap_or_else(|_| "[]".to_string());
    kv.put(REPOS_LIST_KEY, json)?.execute().await?;
    Ok(())
}

/// Add a repository to the repos list if not already present
pub async fn ensure_repo_in_list(kv: &KvStore, github_repo: &str) -> Result<()> {
    let mut repos = list_repos(kv).await?;
    if !repos.contains(&github_repo.to_string()) {
        repos.push(github_repo.to_string());
        repos.sort();
        update_repos_list(kv, &repos).await?;
    }
    Ok(())
}

/// Check and mark JTI for replay protection
/// Returns true if this JTI was already used (replay detected)
pub async fn check_and_mark_replay(kv: &KvStore, jti_hash: &str) -> Result<bool> {
    let key = format!("{}{}", CI_JTI_PREFIX, jti_hash);

    // Check if already exists
    let existing = kv.get(&key).text().await?;
    if existing.is_some() {
        return Ok(true); // Replay detected
    }

    // Mark as used with TTL
    kv.put(&key, "1")?
        .expiration_ttl(REPLAY_TTL_SECS)
        .execute()
        .await?;

    Ok(false)
}
