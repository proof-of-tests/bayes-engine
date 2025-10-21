# Deployment Guide

This project is deployed to CloudFlare Workers, a serverless platform that runs code at the edge.

## Overview

### CloudFlare Workers

CloudFlare Workers is a serverless platform that executes code at CloudFlare's edge network (200+ locations worldwide).
Key features:

- **Edge execution**: Code runs close to users for low latency
- **WebAssembly support**: Runs WASM binaries efficiently
- **Automatic scaling**: Handles traffic spikes without configuration
- **Global distribution**: Deployed to all edge locations
- **Static asset serving**: Hosts HTML, CSS, WASM, and other assets

### Architecture

```
User Request
    ↓
CloudFlare Edge (closest location)
    ↓
Worker (server.wasm) ← Routes request
    ↓
Static Assets (assets/) ← HTML, CSS, client.wasm
    ↓
Client WASM (client.wasm) ← Dioxus web app
```

## Deployment Configuration

### wrangler.toml

The main configuration file:

```toml
name = "bayes-engine"
main = "result/worker/worker.js"
compatibility_date = "2024-10-01"

[build]
command = "nix build .#webapp"

[assets]
directory = "result/assets"

[env.e2e]
# E2E testing environment (no build)
```

### Key Configuration Options

- **name**: Worker name (appears in CloudFlare dashboard)
- **main**: Entry point JavaScript file (loads server.wasm)
- **compatibility_date**: CloudFlare API version
- **build.command**: Command to build webapp before deployment
- **assets.directory**: Directory containing static assets

## Deployment Workflows

### Local Development

```bash
# Build webapp
nix build .#webapp

# Start local development server
nix run .#wrangler-dev

# Access at http://localhost:8787
```

The local server:

- Watches for file changes
- Hot-reloads on updates
- Serves from `result/` directory
- Uses `env.e2e` configuration (no build command)

### Production Deployment

Triggered automatically on push to `main` branch via `.github/workflows/deploy.yml`.

#### Deployment Steps

1. **Build webapp**: `nix build .#webapp`
2. **Check content hash**: Compare with production to skip if unchanged
3. **Deploy to CloudFlare**: `wrangler deploy`
4. **Verify**: Test production URL

#### Deploy Workflow

```yaml
on:
  push:
    branches: [main]

jobs:
  deploy:
    - Build webapp
    - Deploy to CloudFlare Workers (Production)
```

### PR Preview Deployments

Each PR gets a preview deployment at a unique URL.

#### Preview Workflow

1. **Build webapp**: `nix build .#webapp`
2. **Check if changed**: Compare content hash with production
3. **Deploy preview**: `wrangler deploy --name bayes-engine-pr-{number}`
4. **Comment on PR**: Add preview URL to PR

#### Preview URLs

```
Production: https://bayes-engine.lemmih.workers.dev
PR #123:    https://bayes-engine-pr-123.lemmih.workers.dev
```

#### Preview Lifecycle

- **Created**: When PR is opened or updated (if content changed)
- **Skipped**: If webapp content unchanged from production
- **Deleted**: When PR is closed or merged (via `cleanup-preview.yml`)

### Manual Deployment

```bash
# Build first
nix build .#webapp

# Deploy to production
wrangler deploy

# Deploy to specific environment
wrangler deploy --env e2e

# Deploy with custom name
wrangler deploy --name bayes-engine-test
```

## CloudFlare Dashboard

### Access

1. Go to [dash.cloudflare.com](https://dash.cloudflare.com)
2. Navigate to Workers & Pages
3. Find "bayes-engine" worker

### Monitoring

The dashboard shows:

- **Requests**: Total requests per time period
- **Errors**: Error rate and count
- **CPU Time**: Execution time metrics
- **Invocations**: Number of worker invocations
- **Success Rate**: Percentage of successful requests

### Logs

View real-time logs:

```bash
# Tail production logs
wrangler tail

# Tail specific environment
wrangler tail --env e2e

# Filter logs
wrangler tail --status error
```

## Environment Variables & Secrets

### Setting Secrets

```bash
# Add secret (secure, encrypted)
wrangler secret put SECRET_NAME

# List secrets
wrangler secret list

# Delete secret
wrangler secret delete SECRET_NAME
```

### Environment Variables

Add to `wrangler.toml`:

```toml
[env.production.vars]
ENVIRONMENT = "production"
DEBUG = "false"

[env.staging.vars]
ENVIRONMENT = "staging"
DEBUG = "true"
```

Access in worker code:

```rust
let env_value = env.var("ENVIRONMENT")?.to_string();
```

## Hyperdrive (Future)

Hyperdrive is CloudFlare's database acceleration service. We plan to integrate it soon.

### What is Hyperdrive?

Hyperdrive provides:

- **Connection pooling**: Reuses database connections across requests
- **Smart caching**: Caches read queries automatically
- **Regional databases**: Connect to databases anywhere
- **Lower latency**: Reduces database round-trip time

### Planned Integration

Once integrated, the architecture will be:

```
User Request
    ↓
CloudFlare Worker (server.wasm)
    ↓
Hyperdrive (connection pool + cache)
    ↓
Database (PostgreSQL, MySQL, etc.)
```

### Configuration (Placeholder)

When implemented, configuration will look like:

```toml
[[hyperdrive]]
binding = "DB"
id = "hyperdrive-id"
```

### Usage (Placeholder)

```rust
// Future usage example
let db = env.hyperdrive("DB")?;
let result = db.query("SELECT * FROM users").await?;
```

## Deployment Checklist

### Before Deploying

- [ ] All tests pass (`nix flake check`)
- [ ] E2E tests pass (`nix run .#run-e2e-tests`)
- [ ] Changes reviewed and approved
- [ ] No secrets in code (use `wrangler secret` instead)

### After Deploying

- [ ] Check deployment succeeded in GitHub Actions
- [ ] Verify production URL works
- [ ] Check CloudFlare dashboard for errors
- [ ] Monitor logs for issues

## Troubleshooting

### Deployment Fails

Check GitHub Actions logs:

```bash
# View workflow runs
gh run list

# View specific run logs
gh run view <run-id>

# Re-run failed jobs
gh run rerun <run-id>
```

### Worker Errors

View worker logs:

```bash
# Tail logs
wrangler tail

# Check for specific errors
wrangler tail --status error
```

### Asset Loading Issues

Verify assets are included:

```bash
# List assets
ls -la result/assets/

# Check content-hash
cat result/assets/content-hash.txt

# Verify WASM files
ls -la result/worker/worker.js
ls -la result/worker/worker.wasm
```

### Performance Issues

Check CloudFlare dashboard:

1. CPU Time: Look for slow requests
2. Error Rate: Check for failures
3. Logs: Find specific error messages

### Preview Deployment Not Created

Check if content changed:

```bash
# Compare local and production
cat result/assets/content-hash.txt
curl https://bayes-engine.lemmih.workers.dev/content-hash.txt
```

Preview is skipped if hashes match (no changes).

## Best Practices

1. **Test locally first**: Always test with `nix run .#wrangler-dev`
2. **Use PR previews**: Review changes in preview environment
3. **Monitor after deployment**: Check logs and metrics
4. **Keep secrets out of code**: Use `wrangler secret put`
5. **Use content hashing**: Leverage automatic cache busting
6. **Check bundle size**: Run `nix run .#report-sizes` regularly
7. **Handle errors gracefully**: Return user-friendly error messages
8. **Set compatibility_date**: Keep CloudFlare API version explicit

## Performance Optimization

### WASM Size

```bash
# Check WASM sizes
nix run .#report-sizes

# Optimize build in Cargo.toml
[profile.release]
opt-level = "z"
lto = true
codegen-units = 1
```

### Asset Optimization

- **Compress assets**: Use gzip/brotli
- **Minimize JavaScript**: Already done by wasm-bindgen
- **Optimize WASM**: Already done by wasm-opt
- **Cache static assets**: CloudFlare does this automatically

### Cold Start Optimization

- **Keep worker small**: Minimize dependencies
- **Use lazy loading**: Load heavy code on-demand
- **Profile startup**: Use `console.time()` in worker.js

## CI/CD Secrets

Required GitHub secrets (set in repository settings):

- `CLOUDFLARE_API_TOKEN`: API token for deployment
- `CLOUDFLARE_ACCOUNT_ID`: Account ID from CloudFlare dashboard

### Creating API Token

1. Go to CloudFlare dashboard
2. Navigate to My Profile > API Tokens
3. Create Token > Edit CloudFlare Workers template
4. Copy token and add to GitHub secrets

## Resources

- [CloudFlare Workers Documentation](https://developers.cloudflare.com/workers/)
- [Wrangler CLI Reference](https://developers.cloudflare.com/workers/wrangler/)
- [Workers Rust SDK](https://github.com/cloudflare/workers-rs)
- [Hyperdrive Documentation](https://developers.cloudflare.com/hyperdrive/)
- [CloudFlare Status](https://www.cloudflarestatus.com/)
