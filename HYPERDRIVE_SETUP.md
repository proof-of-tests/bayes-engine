# Hyperdrive Database Setup

This document explains how to set up PostgreSQL database connectivity via CloudFlare's Hyperdrive service.

## Current Status: Investigation Required

**Important Note**: Initial investigation revealed that `tokio-postgres` (the primary Rust Postgres client) is
incompatible with WASM targets that CloudFlare Workers use. The `mio` crate (Tokio's I/O layer) explicitly does not
support `wasm32-unknown-unknown` targets.

### Alternative Approaches to Consider:

1. **CloudFlare D1**: Use CloudFlare's native D1 database (SQLite) which has full Workers support
2. **HTTP API Bridge**: Create a separate HTTP service that handles Postgres connections
3. **JavaScript Interop**: Use JavaScript libraries via wasm-bindgen for database access
4. **Different Platform**: Consider platforms that support native Rust (Deno Deploy, Fly.io, etc.)

The infrastructure below is provided for reference and future use if/when WASM-compatible solutions become available.

## Production Setup (Neon + CloudFlare Hyperdrive)

### Prerequisites

- CloudFlare Workers account with Hyperdrive access
- Neon PostgreSQL database (or any PostgreSQL provider)
- Wrangler CLI installed

### Steps

1. **Create Hyperdrive Configuration**

   Use the Wrangler CLI to create a Hyperdrive configuration:

   ```bash
   wrangler hyperdrive create bayes-db \
     --connection-string="postgresql://username:password@host/database?sslmode=require"
   ```

   This command will output a Hyperdrive ID that looks like: `1234abcd5678efgh9012ijkl3456mnop`

2. **Update wrangler.toml**

   Update the `id` field in the `[[hyperdrive]]` section of `wrangler.toml`:

   ```toml
   [[hyperdrive]]
   binding = "DB"
   id = "your-hyperdrive-id-here"  # Replace with the ID from step 1
   ```

3. **Deploy**

   Deploy your worker:

   ```bash
   wrangler deploy
   ```

## Local Development

For local development and E2E testing, you can use a local PostgreSQL instance via Docker.

### Using Docker Compose

1. **Start Local PostgreSQL**

   ```bash
   docker-compose up -d
   ```

   This starts a PostgreSQL 16 instance on port 5432 with:

   - Username: `hyperdrive-user`
   - Password: `localdev_password`
   - Database: `neondb`

2. **Create Local Hyperdrive Configuration**

   Create a Hyperdrive configuration pointing to your local database:

   ```bash
   wrangler hyperdrive create bayes-db-local \
     --connection-string="postgresql://hyperdrive-user:localdev_password@localhost:5432/neondb"
   ```

3. **Update wrangler.toml for Local Development**

   You can either:

   - Use the same binding with a different ID for local development
   - Create an environment-specific configuration

   Option A: Update the existing binding (simpler for local dev):

   ```toml
   [[hyperdrive]]
   binding = "DB"
   id = "your-local-hyperdrive-id"
   ```

   Option B: Create a dev environment (better for team development):

   ```toml
   # Production
   [[hyperdrive]]
   binding = "DB"
   id = "production-hyperdrive-id"

   # Local development
   [env.dev.hyperdrive]
   binding = "DB"
   id = "local-hyperdrive-id"
   ```

   Then use: `wrangler dev --env dev`

4. **Run Wrangler Dev**

   ```bash
   wrangler dev
   ```

### Stopping Local PostgreSQL

```bash
docker-compose down
```

To remove data volumes:

```bash
docker-compose down -v
```

## Testing Database Connectivity

Once set up, you can test the database connection by visiting:

```
http://localhost:8787/api/db-test
```

This endpoint will:

1. Connect to the database via Hyperdrive
2. Create a test table if it doesn't exist
3. Insert a test record
4. Read back recent records
5. Return a JSON response with the results

## Troubleshooting

### Connection Errors

If you see connection errors:

1. **Check Hyperdrive Configuration**

   ```bash
   wrangler hyperdrive list
   wrangler hyperdrive get <hyperdrive-id>
   ```

2. **Verify Database Accessibility**

   - For local: Check if Docker container is running: `docker ps`
   - For remote: Verify connection string and firewall rules

3. **Check Wrangler Logs**

   - Look at `wrangler.log` for detailed error messages
   - Use `wrangler tail` to see live logs in production

### SSL/TLS Issues

- Neon and most cloud PostgreSQL providers require SSL
- Use `sslmode=require` in your connection string
- CloudFlare Hyperdrive handles TLS passthrough automatically

## Security Notes

- **Never commit database credentials to git**
- The `.dev.vars` file is in `.gitignore` - use it for local secrets if needed
- Hyperdrive stores credentials securely in CloudFlare's infrastructure
- Use different databases for production, staging, and development
- Rotate passwords regularly
- Use read-only credentials where possible
