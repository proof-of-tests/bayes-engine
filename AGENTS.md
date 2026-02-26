# Bayes Engine - Agent Development Guide

This document provides project-specific context and guidelines for AI agents working on bayes-engine, a distributed
fuzzy testing platform using proof-of-work.

## Project Overview

### What is Bayes Engine?

Bayes Engine is a **distributed fuzzy testing platform** that uses proof-of-work to verify test execution. Instead of
trusting that tests were run, we use cryptographic hashes to prove computational work was performed.

### How It Works

1. **WASM Test Functions**: Projects compile test functions to WebAssembly with signature `u64 -> u64` (seed in, hash
   out)
2. **Distributed Execution**: Browsers and CLI clients execute these functions with random seeds
3. **Hash Submission**: Low hash values are submitted to the server as proof of work
4. **Cardinality Estimation**: HyperLogLog structures estimate total unique test executions

### Why Proof-of-Work for Tests?

- **Verifiable**: Anyone can verify that computational work was performed
- **Trustless**: No need to trust test runners - the math proves the work
- **Distributed**: Tests can run anywhere (browsers, CI, CLI) and contribute to the same estimate
- **Incentive-compatible**: More testing = lower hashes found = higher estimates

## Architecture

### Components

```
                                 +------------------+
                                 |   GitHub Repo    |
                                 |  (WASM source)   |
                                 +--------+---------+
                                          |
                                    GitHub Actions
                                    (OIDC auth)
                                          |
                                          v
+-------------+    GET /api/    +------------------+    Hyperdrive    +------------+
|   Browser   | <------------> |  CloudFlare      | <--------------> |  Postgres  |
|   (Dioxus)  |                |  Worker (WASM)   |                  |   (Neon)   |
+-------------+                +--------+---------+                  +------------+
      |                                 |
      |   POST /api/test-results        |  R2 Storage
      +-------------------------------->|  (WASM files)
                                        v
+-------------+                +------------------+
|    CLI      | <------------> |   R2 Bucket      |
| (wasmtime)  |  GET wasm      | bayes-engine-wasm|
+-------------+                +------------------+
```

### Data Flow

1. **Upload Flow** (GitHub Actions -> Server -> R2/Postgres)

   - GitHub Actions builds WASM test module
   - Requests OIDC token with audience `bayes-engine-ci-upload`
   - POSTs to `/api/ci-upload` with WASM file
   - Server validates token, checks repo is public
   - Stores WASM in R2, metadata in Postgres

2. **Execution Flow** (Client/CLI -> Server)

   - Client fetches WASM from `/api/wasm-files/:id`
   - Executes functions with random seeds
   - Tracks local HyperLogLog state
   - Submits improving hashes to `/api/test-results`

3. **Display Flow** (Server -> Client)

   - Client fetches repository list from `/api/repositories`
   - Displays estimated test counts from HyperLogLog
   - Shows real-time updates as tests run

### Project Structure

```
.
├── AGENTS.md             # This file - project context for AI agents
├── client/               # Dioxus web UI (compiles to WASM)
│   └── src/
│       ├── lib.rs        # Main app, routing, RepoRunner component
│       └── hyperloglog.rs # Client-side HLL implementation
├── server/               # CloudFlare Worker (compiles to WASM)
│   └── src/lib.rs        # API endpoints, OIDC validation, DB queries
├── cli/                  # Native Rust CLI runner
│   └── src/main.rs       # Multi-threaded wasmtime executor
├── examples/
│   └── pow-test-functions/ # Sample WASM test functions
├── e2e_tests/            # Selenium-based browser tests
├── agents/               # Detailed workflow guides
│   ├── WORKFLOW.md       # Git workflow, branching, PRs
│   ├── NIX.md            # Nix commands and builds
│   ├── TESTING.md        # Testing strategies
│   ├── DEPLOYMENT.md     # CloudFlare deployment
│   └── TOOLS.md          # AI agent tool usage
├── nix/                  # Build scripts
├── .github/workflows/    # CI/CD pipelines
├── flake.nix             # Nix flake configuration
└── wrangler.toml         # CloudFlare Workers config
```

## Core Concepts

### WASM Test Functions

Test functions must have the signature `fn(u64) -> u64`:

- **Input**: A 64-bit seed value
- **Output**: A 64-bit hash value
- **Requirement**: Deterministic (same seed = same hash)

```rust
// Example from examples/pow-test-functions/src/lib.rs
#[no_mangle]
pub extern "C" fn splitmix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9e3779b97f4a7c15);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
    x ^ (x >> 31)
}
```

### HyperLogLog Cardinality Estimation

We use a **min-hash variant** of HyperLogLog to estimate unique test executions:

1. **Register Selection**: Lower `bits` of hash select which register (bucket)
2. **Min-Hash Storage**: Each register stores the minimum hash seen
3. **Estimation**: Count leading zeros in remaining bits to estimate cardinality

```
Hash: 0x00000ABC12345678
      └─────┘└────────┘
       rho    register (if bits=12)

Lower hash = more leading zeros = rarer event = higher estimate
```

**Key parameters:**

- `HLL_BITS = 12` (default) -> 4096 registers
- `MAX_HLL_BITS = 20` -> up to 1M registers
- More bits = better precision, more storage

### Proof-of-Work Model

The probability of finding a hash with `k` leading zeros is `1/2^k`. By collecting minimum hashes per register, we can
estimate how many total hashes were computed.

**Why this works:**

- Finding hash `0x0001...` requires ~65K attempts on average
- Finding hash `0x00001...` requires ~1M attempts on average
- HyperLogLog aggregates these across registers for accurate estimation

### GitHub OIDC Authentication

CI uploads are authenticated using GitHub's OIDC tokens:

1. GitHub Actions requests token with audience `bayes-engine-ci-upload`
2. Server fetches GitHub's JWKS and validates RS256 signature
3. Claims include `repository`, `repository_id`, `event_name`
4. Only public repositories are accepted
5. JTI (JWT ID) is tracked to prevent replay attacks

## API Reference

### Repository Endpoints

| Method | Endpoint | Description | |--------|----------|-------------| | GET | `/api/repositories` | List all
repositories with aggregated stats | | GET | `/api/repositories/:owner/:repo` | Repository detail with all versions | |
GET | `/api/repositories/:owner/:repo/latest-catalog` | Latest version's file catalog |

### WASM File Endpoints

| Method | Endpoint | Description | |--------|----------|-------------| | GET | `/api/wasm-files/:id` | Download WASM
binary from R2 | | POST | `/api/ci-upload` | Upload WASM from GitHub Actions (OIDC auth) |

### Test Result Endpoints

| Method | Endpoint | Description | |--------|----------|-------------| | POST | `/api/test-results` | Submit hash
result to update HLL state |

### Request/Response Examples

**Submit Test Result:**

```json
// POST /api/test-results
{
  "function_id": 123,
  "wasm_file_id": 456,
  "function_name": "splitmix",
  "seed": "12345678901234567890",
  "hash": "98765432109876543210"
}

// Response
{
  "ok": true,
  "improved": true,
  "estimated_tests": 1048576.0,
  "submitted_updates": 42
}
```

## Database Schema

### Tables

```sql
-- Repositories registered for testing
repositories (
  id BIGSERIAL PRIMARY KEY,
  github_repo TEXT NOT NULL UNIQUE,  -- e.g., "owner/repo"
  created_at TIMESTAMPTZ
)

-- Version tags (usually git SHA or tag)
repository_versions (
  id BIGSERIAL PRIMARY KEY,
  repository_id BIGINT REFERENCES repositories(id),
  version TEXT NOT NULL,
  created_at TIMESTAMPTZ,
  UNIQUE(repository_id, version)
)

-- Uploaded WASM files
wasm_files (
  id BIGSERIAL PRIMARY KEY,
  repository_id BIGINT REFERENCES repositories(id),
  version_id BIGINT REFERENCES repository_versions(id),
  file_sha256 TEXT NOT NULL,
  r2_key TEXT,  -- R2 object key
  uploaded_at TIMESTAMPTZ,
  UNIQUE(repository_id, version_id, file_sha256)
)

-- Individual test functions with HLL state
wasm_functions (
  id BIGSERIAL PRIMARY KEY,
  wasm_file_id BIGINT REFERENCES wasm_files(id),
  repository_id BIGINT REFERENCES repositories(id),
  version_id BIGINT REFERENCES repository_versions(id),
  function_name TEXT NOT NULL,
  hll_bits INTEGER DEFAULT 12,
  hll_hashes_json TEXT,  -- JSON array of min-hashes
  submitted_updates BIGINT DEFAULT 0,
  lowest_hash TEXT,
  lowest_seed TEXT,
  updated_at TIMESTAMPTZ,
  UNIQUE(wasm_file_id, function_name)
)
```

## Development Guide

### Quick Start

```bash
# Create feature branch
git fetch origin
git checkout -b feat/my-feature origin/main

# Enter development shell
nix develop

# Make changes, then test
nix flake check
nix run .#run-e2e-tests

# Commit and push
git add .
git commit -m "feat: add my feature"
git push -u origin feat/my-feature

# Create PR
gh pr create --title "feat: add my feature" --body "Brief description"
```

### Testing

```bash
# Run all checks (formatting, linting, tests, build)
nix flake check

# Run end-to-end tests (starts local server)
nix run .#run-e2e-tests

# Run Rust unit tests only
cargo test

# Run local dev server
nix run .#wrangler-dev
```

### Deployment

- **Production**: Auto-deployed on push to `main` at `bayes.lemmih.com`
- **PR Previews**: Each PR gets `bayes-engine-pr-{N}.lemmih.workers.dev`
- **Platform**: CloudFlare Workers (edge serverless)

## Tips for AI Agents

### Before Making Changes

- **Read before writing**: Always read existing files before modifying them
- **Understand the data flow**: Trace how data moves through the system
- **Check for duplicates**: Search for similar implementations before adding new code

### Code Standards

- **Clarity over cleverness**: Write code that's easy to understand
- **Handle errors explicitly**: Use proper error types, don't ignore failures
- **Follow existing patterns**: Match the style in the codebase

### Testing

- **Run checks**: Always run `nix flake check` before committing
- **E2E tests**: Run `nix run .#run-e2e-tests` for integration testing
- **Test edge cases**: Consider error conditions and boundary values

### Documentation

- **Update docs**: Keep documentation in sync with code changes
- **Explain why**: Comments should explain reasoning, not just what code does
- **Use the guides**: Refer to `agents/*.md` for detailed workflows

### Workflow

- **Small commits**: Make focused commits with clear messages
- **Conventional commits**: Use `feat:`, `fix:`, `docs:`, etc. prefixes
- **Ask when unclear**: Use questions if requirements are ambiguous

## Experience Log

Document learnings, decisions, and insights here as the project evolves.

### [2025-10-21] Initial Documentation Structure

- Created dedicated documentation guides in `agents/` directory
- Established workflow for development, testing, and deployment

### [2026-02-26] AGENTS.md Rewrite

- Rewrote AGENTS.md with project-specific context (issue #87)
- Added architecture diagrams, core concepts, API reference
- Documented HyperLogLog algorithm and proof-of-work model
- Added database schema documentation
