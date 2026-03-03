//! HLL hash storage using CloudFlare D1
//!
//! Stores HyperLogLog register values with atomic updates.
//! Each register is stored as a separate row to enable lock-free atomic updates.

use hyperloglog::{HyperLogLog, DEFAULT_HLL_BITS};
use worker::{d1::D1Database, Result};

/// Maximum value for u64, used as initial hash value
const U64_MAX_STR: &str = "18446744073709551615";

/// Format a u64 as a zero-padded 20-character string for correct lexicographic comparison
pub fn format_hash(hash: u64) -> String {
    format!("{:020}", hash)
}

/// Parse a zero-padded hash string back to u64
pub fn parse_hash(s: &str) -> u64 {
    s.parse().unwrap_or(u64::MAX)
}

/// Initialize the database schema
pub async fn ensure_schema(db: &D1Database) -> Result<()> {
    db.exec(
        "CREATE TABLE IF NOT EXISTS function_hashes (
            r2_key TEXT NOT NULL,
            function_name TEXT NOT NULL,
            register_idx INTEGER NOT NULL,
            min_hash TEXT NOT NULL DEFAULT '18446744073709551615',
            seed TEXT,
            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            PRIMARY KEY (r2_key, function_name, register_idx)
        )",
    )
    .await?;

    db.exec("CREATE INDEX IF NOT EXISTS idx_hashes_by_file ON function_hashes(r2_key)")
        .await?;

    db.exec(
        "CREATE TABLE IF NOT EXISTS function_stats (
            r2_key TEXT NOT NULL,
            function_name TEXT NOT NULL,
            submitted_updates INTEGER DEFAULT 0,
            lowest_hash TEXT,
            lowest_seed TEXT,
            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            PRIMARY KEY (r2_key, function_name)
        )",
    )
    .await?;

    db.exec("CREATE INDEX IF NOT EXISTS idx_stats_by_file ON function_stats(r2_key)")
        .await?;

    Ok(())
}

/// Get HLL state for a function, reconstructing from individual register rows
pub async fn get_hll_state(
    db: &D1Database,
    r2_key: &str,
    function_name: &str,
) -> Result<HyperLogLog> {
    let stmt = db.prepare(
        "SELECT register_idx, min_hash FROM function_hashes 
         WHERE r2_key = ? AND function_name = ?
         ORDER BY register_idx",
    );

    let results = stmt
        .bind(&[r2_key.into(), function_name.into()])?
        .all()
        .await?;

    let mut hll = HyperLogLog::new(DEFAULT_HLL_BITS);
    let hashes = hll.hashes_mut();

    for row in results.results::<RegisterRow>()? {
        if (row.register_idx as usize) < hashes.len() {
            hashes[row.register_idx as usize] = parse_hash(&row.min_hash);
        }
    }

    Ok(hll)
}

/// Get function stats (submitted_updates, lowest_hash, etc.)
pub async fn get_function_stats(
    db: &D1Database,
    r2_key: &str,
    function_name: &str,
) -> Result<Option<FunctionStats>> {
    let stmt = db.prepare(
        "SELECT submitted_updates, lowest_hash, lowest_seed FROM function_stats 
         WHERE r2_key = ? AND function_name = ?",
    );

    let results = stmt
        .bind(&[r2_key.into(), function_name.into()])?
        .all()
        .await?;
    let rows = results.results::<FunctionStats>()?;
    Ok(rows.into_iter().next())
}

/// Submit a hash update - atomically updates only if the new hash is lower
/// Returns true if the hash was improved
pub async fn submit_hash(
    db: &D1Database,
    r2_key: &str,
    function_name: &str,
    seed: u64,
    hash: u64,
) -> Result<bool> {
    // Calculate which register this hash belongs to
    let bits = DEFAULT_HLL_BITS;
    let mask = (1usize << bits) - 1;
    let register_idx = (hash as usize) & mask;

    let hash_str = format_hash(hash);
    let seed_str = seed.to_string();

    // Atomic upsert - only updates if new hash is smaller (lexicographically)
    let stmt = db.prepare(
        "INSERT INTO function_hashes (r2_key, function_name, register_idx, min_hash, seed, updated_at)
         VALUES (?, ?, ?, ?, ?, datetime('now'))
         ON CONFLICT (r2_key, function_name, register_idx) DO UPDATE SET
           min_hash = CASE WHEN excluded.min_hash < function_hashes.min_hash THEN excluded.min_hash ELSE function_hashes.min_hash END,
           seed = CASE WHEN excluded.min_hash < function_hashes.min_hash THEN excluded.seed ELSE function_hashes.seed END,
           updated_at = CASE WHEN excluded.min_hash < function_hashes.min_hash THEN excluded.updated_at ELSE function_hashes.updated_at END",
    );

    stmt.bind(&[
        r2_key.into(),
        function_name.into(),
        (register_idx as i64).into(),
        hash_str.clone().into(),
        seed_str.clone().into(),
    ])?
    .run()
    .await?;

    // Check if we actually improved (read back the current value)
    let check_stmt = db.prepare(
        "SELECT min_hash FROM function_hashes WHERE r2_key = ? AND function_name = ? AND register_idx = ?",
    );
    let results = check_stmt
        .bind(&[
            r2_key.into(),
            function_name.into(),
            (register_idx as i64).into(),
        ])?
        .all()
        .await?;

    let improved = results
        .results::<MinHashRow>()?
        .first()
        .map(|row| row.min_hash == hash_str)
        .unwrap_or(false);

    // Update stats if improved
    if improved {
        update_stats(db, r2_key, function_name, hash, seed).await?;
    }

    Ok(improved)
}

/// Update function stats after a hash improvement
async fn update_stats(
    db: &D1Database,
    r2_key: &str,
    function_name: &str,
    hash: u64,
    seed: u64,
) -> Result<()> {
    let hash_str = format_hash(hash);
    let seed_str = seed.to_string();

    let stmt = db.prepare(
        "INSERT INTO function_stats (r2_key, function_name, submitted_updates, lowest_hash, lowest_seed, updated_at)
         VALUES (?, ?, 1, ?, ?, datetime('now'))
         ON CONFLICT (r2_key, function_name) DO UPDATE SET
           submitted_updates = function_stats.submitted_updates + 1,
           lowest_hash = CASE WHEN excluded.lowest_hash < function_stats.lowest_hash OR function_stats.lowest_hash IS NULL 
                         THEN excluded.lowest_hash ELSE function_stats.lowest_hash END,
           lowest_seed = CASE WHEN excluded.lowest_hash < function_stats.lowest_hash OR function_stats.lowest_hash IS NULL 
                         THEN excluded.lowest_seed ELSE function_stats.lowest_seed END,
           updated_at = datetime('now')",
    );

    stmt.bind(&[
        r2_key.into(),
        function_name.into(),
        hash_str.into(),
        seed_str.into(),
    ])?
    .run()
    .await?;

    Ok(())
}

/// Initialize HLL registers for a new function (all set to MAX)
pub async fn init_function_registers(
    db: &D1Database,
    r2_key: &str,
    function_name: &str,
) -> Result<()> {
    let num_registers = 1usize << DEFAULT_HLL_BITS;

    for register_idx in 0..num_registers {
        let stmt = db.prepare(
            "INSERT OR IGNORE INTO function_hashes (r2_key, function_name, register_idx, min_hash, updated_at)
             VALUES (?, ?, ?, ?, datetime('now'))",
        );

        stmt.bind(&[
            r2_key.into(),
            function_name.into(),
            (register_idx as i64).into(),
            U64_MAX_STR.into(),
        ])?
        .run()
        .await?;
    }

    // Initialize stats entry
    let stmt = db.prepare(
        "INSERT OR IGNORE INTO function_stats (r2_key, function_name, submitted_updates, updated_at)
         VALUES (?, ?, 0, datetime('now'))",
    );
    stmt.bind(&[r2_key.into(), function_name.into()])?
        .run()
        .await?;

    Ok(())
}

/// Get all HLL states for functions in a file
pub async fn get_file_hll_states(
    db: &D1Database,
    r2_key: &str,
) -> Result<Vec<(String, HyperLogLog, FunctionStats)>> {
    // Get all unique function names for this file
    let stmt = db.prepare(
        "SELECT DISTINCT function_name FROM function_hashes WHERE r2_key = ? ORDER BY function_name",
    );
    let results = stmt.bind(&[r2_key.into()])?.all().await?;
    let function_names: Vec<String> = results
        .results::<FunctionNameRow>()?
        .into_iter()
        .map(|r| r.function_name)
        .collect();

    let mut states = Vec::new();
    for function_name in function_names {
        let hll = get_hll_state(db, r2_key, &function_name).await?;
        let stats = get_function_stats(db, r2_key, &function_name)
            .await?
            .unwrap_or(FunctionStats {
                submitted_updates: 0,
                lowest_hash: None,
                lowest_seed: None,
            });
        states.push((function_name, hll, stats));
    }

    Ok(states)
}

// Row types for D1 query results
#[derive(serde::Deserialize)]
struct RegisterRow {
    register_idx: i64,
    min_hash: String,
}

#[derive(serde::Deserialize)]
struct MinHashRow {
    min_hash: String,
}

#[derive(serde::Deserialize)]
struct FunctionNameRow {
    function_name: String,
}

#[derive(serde::Deserialize, Clone)]
pub struct FunctionStats {
    pub submitted_updates: i64,
    pub lowest_hash: Option<String>,
    #[allow(dead_code)]
    pub lowest_seed: Option<String>,
}
