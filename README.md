# Dioxus on CloudFlare Workers

A Hello World web application built with [Dioxus](https://dioxuslabs.com/) and [workers-rs](https://github.com/cloudflare/workers-rs), compiled to WebAssembly and deployable to CloudFlare Workers.

## Features

- **Dioxus 0.6**: Modern Rust UI framework with server-side rendering
- **CloudFlare Workers**: Edge computing platform for fast, global deployment
- **WebAssembly**: Compiled to WASM for efficient execution
- **Zero JavaScript**: Pure Rust implementation

## Prerequisites

Before you begin, ensure you have the following installed:

1. **Rust** (latest stable version)

   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

2. **wasm32-unknown-unknown target**

   ```bash
   rustup target add wasm32-unknown-unknown
   ```

3. **Wrangler CLI** (CloudFlare Workers CLI)

   ```bash
   npm install -g wrangler
   ```

4. **worker-build** (will be installed automatically during build, but you can install it manually)

   ```bash
   cargo install worker-build
   ```

## Local Development

### Build the Project

```bash
# Build in debug mode
worker-build

# Build in release mode (optimized)
worker-build --release
```

### Test Locally

Run the worker locally using Wrangler:

```bash
wrangler dev
```

This will start a local server at `http://localhost:8787`. Open this URL in your browser to see the Hello World app.

## Deployment to CloudFlare Workers

### 1. Login to CloudFlare

```bash
wrangler login
```

This will open a browser window for authentication.

### 2. Deploy

```bash
wrangler deploy
```

The command will:

1. Build your project using `worker-build`
2. Compile the Rust code to WebAssembly
3. Deploy to CloudFlare Workers
4. Provide you with a URL where your app is live

### 3. View Your Deployment

After deployment, you'll see output like:

```
Published agent-1 (X.XX sec)
  https://agent-1.<your-subdomain>.workers.dev
```

Visit the provided URL to see your app running on CloudFlare's edge network!

## Project Structure

```
.
├── Cargo.toml          # Rust dependencies and project configuration
├── wrangler.toml       # CloudFlare Workers configuration
├── src/
│   └── lib.rs          # Main application code (Dioxus app + Workers handler)
└── README.md           # This file
```

## How It Works

1. **Dioxus SSR**: The app uses Dioxus's server-side rendering to generate HTML
2. **Workers Handler**: The `#[event(fetch)]` macro creates a CloudFlare Workers request handler
3. **WASM Compilation**: Rust code is compiled to WebAssembly via `wasm32-unknown-unknown`
4. **Edge Deployment**: CloudFlare Workers runs the WASM binary on their edge network

## Customization

### Modify the App

Edit `src/lib.rs` to change the UI. The `App` component uses Dioxus's `rsx!` macro:

```rust
#[component]
fn App() -> Element {
    rsx! {
        div { class: "container",
            h1 { "Your Custom Title" }
            p { "Your custom content here" }
        }
    }
}
```

### Add Routes

Extend the route matching in the `fetch` handler:

```rust
match path.as_str() {
    "/" => { /* home page */ },
    "/about" => { /* about page */ },
    _ => Response::error("Not Found", 404),
}
```

### Add Styling

Modify the inline CSS in the `full_html` format string, or serve external CSS files.

## Troubleshooting

### Build Errors

If you encounter build errors:

1. Ensure `wasm32-unknown-unknown` target is installed
2. Clear the build cache: `cargo clean`
3. Update dependencies: `cargo update`

### Deployment Issues

If deployment fails:

1. Verify you're logged in: `wrangler whoami`
2. Check your CloudFlare account has Workers enabled
3. Review `wrangler.toml` configuration

## Resources

- [Dioxus Documentation](https://dioxuslabs.com/learn/0.6/)
- [workers-rs Repository](https://github.com/cloudflare/workers-rs)
- [CloudFlare Workers Docs](https://developers.cloudflare.com/workers/)
- [Wrangler CLI Docs](https://developers.cloudflare.com/workers/wrangler/)

## License

See LICENSE file for details.
