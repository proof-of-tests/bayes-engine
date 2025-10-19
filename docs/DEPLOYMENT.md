# Deployment Guide

This document explains how to deploy the webapp to CloudFlare Workers using GitHub Actions.

## Prerequisites

- A CloudFlare account with Workers enabled
- Repository access to configure GitHub secrets

## Required Secrets

You need to configure two GitHub repository secrets:

### 1. CLOUDFLARE_API_TOKEN

This is an API token with permissions to deploy Workers.

**How to create:**

1. Go to [CloudFlare Dashboard](https://dash.cloudflare.com/profile/api-tokens)
2. Click "Create Token"
3. Use the "Edit Cloudflare Workers" template, or create a custom token with:
   - Account - Cloudflare Workers Scripts - Edit
   - Account - Cloudflare Workers KV Storage - Edit (if using KV)
   - Account - Account Settings - Read
4. Click "Continue to summary" and "Create Token"
5. Copy the token (you won't be able to see it again)

**Required permissions:**

- Account - Workers Scripts - Edit
- Account - Workers KV - Edit (optional, for KV storage)
- Account - Account Settings - Read
- User - User Details - Read

### 2. CLOUDFLARE_ACCOUNT_ID

Your CloudFlare account ID.

**How to find:**

1. Go to your [CloudFlare Dashboard](https://dash.cloudflare.com/)
2. Select any website/zone (or go to Workers & Pages)
3. Look in the right sidebar for "Account ID"
4. Copy the account ID

Alternatively, run locally:

```bash
wrangler whoami
```

## Setting Up GitHub Secrets

1. Go to your repository on GitHub
2. Navigate to Settings → Secrets and variables → Actions
3. Click "New repository secret"
4. Add both secrets:
   - Name: `CLOUDFLARE_API_TOKEN`, Value: your API token
   - Name: `CLOUDFLARE_ACCOUNT_ID`, Value: your account ID

## Deployment Workflow

The deployment workflow (`.github/workflows/deploy.yml`) will:

1. Trigger automatically on:
   - Pushes to the `main` branch (full deployment)
   - Pull requests to `main` (dry-run deployment for validation)
2. Build the webapp using Nix (reproducible, no need for worker-build installation)
3. Deploy to CloudFlare Workers using wrangler

**Note:** Pull requests run with `--dry-run` flag, which validates the deployment without actually publishing changes.
This ensures PRs can verify deployment configuration before merging.

You can also trigger deployments manually:

- Go to Actions → Deploy to CloudFlare Workers → Run workflow

## Local Deployment

To deploy locally (as before):

```bash
wrangler deploy
```

This uses your local wrangler authentication.

## Troubleshooting

### "Authentication error" in GitHub Actions

- Verify both secrets are set correctly
- Check that the API token hasn't expired
- Ensure the token has the required permissions

### Build failures

- Check the build logs in GitHub Actions
- Try building locally: `nix build .#webapp`
- Ensure all tests pass: `nix flake check`

### Deployment succeeds but site doesn't work

- Check CloudFlare Workers dashboard for errors
- View logs: `wrangler tail`
- Verify the worker route is configured correctly
