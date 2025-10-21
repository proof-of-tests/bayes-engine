# Nix Development Guide

This project uses Nix flakes for reproducible builds, development environments, and CI/CD workflows.

## Overview

Nix provides:

- **Reproducible builds**: Everyone gets the exact same development environment
- **Declarative dependencies**: All tools and libraries are specified in `flake.nix`
- **Hermetic builds**: Builds are isolated from the host system
- **Caching**: Build artifacts are cached for faster subsequent builds

## Essential Commands

### Development

```bash
# Enter development shell with all tools available
nix develop

# Run the webapp locally with wrangler
nix run .#wrangler-dev

# Build the complete webapp (client + server WASM)
nix build .#webapp

# Build e2e tests binary
nix build .#e2eTests

# Report sizes of WASM and static assets
nix run .#report-sizes
```

### Testing

```bash
# Run ALL checks (formatting, linting, tests, builds)
nix flake check

# Run end-to-end tests
nix run .#run-e2e-tests
```

### Formatting and Linting

```bash
# Format all Nix files
nix fmt

# Check individual aspects
nix build .#checks.x86_64-linux.markdown-format
nix build .#checks.x86_64-linux.nix-format
nix build .#checks.x86_64-linux.nix-lint
nix build .#checks.x86_64-linux.shellcheck
nix build .#checks.x86_64-linux.rust-fmt
nix build .#checks.x86_64-linux.rust-clippy
nix build .#checks.x86_64-linux.rust-test
```

## CI/CD Integration

### Pre-commit Validation

Before committing code, the following commands are run automatically (conceptually - not yet implemented as git hooks):

1. `nix flake check` - Runs all formatting, linting, and test checks
2. `nix run .#run-e2e-tests` - Runs end-to-end tests with Selenium WebDriver

These same checks run in CI, so running them locally ensures your PR will pass.

### CI Workflow

The CI pipeline (`.github/workflows/ci.yml`) runs:

```yaml
- nix flake check      # All checks in parallel
- nix run .#run-e2e-tests  # End-to-end tests
```

## Flake Structure

### Checks

The `checks` output in `flake.nix` defines validation steps:

- **markdown-format**: Validates markdown formatting with mdformat
- **nix-format**: Checks Nix file formatting with nixpkgs-fmt
- **nix-lint**: Lints Nix files with statix
- **shellcheck**: Validates shell scripts
- **rust-build**: Builds Rust workspace with Crane
- **rust-fmt**: Checks Rust formatting with rustfmt
- **rust-clippy**: Lints Rust code with Clippy
- **rust-test**: Runs Rust unit tests
- **rust-audit**: Runs cargo audit for security vulnerabilities
- **webapp-build**: Builds complete webapp (client + server WASM)

### Packages

- `default` / `bayes-engine`: Native Rust build
- `webapp`: Complete webapp build (WASM + assets)
- `wasmBuild`: Raw WASM binaries (client + server)
- `e2eTests`: End-to-end test binary

### Apps

- `wrangler-dev`: Run webapp locally with hot reload
- `report-sizes`: Report WASM and asset sizes
- `run-e2e-tests`: Run end-to-end tests

## Build Process

### WASM Compilation

The project compiles two WASM modules:

1. **client.wasm**: Dioxus web UI (runs in browser)
2. **server.wasm**: CloudFlare Worker (runs on edge)

Build steps:

1. `craneLibWasm.buildPackage`: Compile Rust to WASM
2. `wasm-bindgen`: Generate JavaScript bindings
3. `wasm-opt`: Optimize WASM size (from binaryen package)

### Asset Processing

The webapp includes:

- WASM binaries (client + server)
- JavaScript glue code (from wasm-bindgen)
- Static assets (HTML, CSS, images)

## Troubleshooting

### Build fails with "derivation ... not found"

Clear the Nix cache and rebuild:

```bash
nix flake update
nix build .#webapp --rebuild
```

### "command not found" in development shell

Enter the development shell explicitly:

```bash
nix develop
```

### Tests fail locally but pass in CI

Ensure you're on the latest flake:

```bash
git pull origin main
nix flake update
nix flake check
```

### WASM build fails

Check that the Rust toolchain includes the wasm32-unknown-unknown target (defined in `rust-toolchain.toml`).

## Best Practices

1. **Always run `nix flake check` before committing**
2. **Use `nix develop` for consistent tooling**
3. **Don't commit build artifacts** (e.g., `result/`, `build/`)
4. **Keep `flake.lock` up to date** by running `nix flake update` periodically
5. **Test locally with `nix run .#run-e2e-tests`** to catch issues before CI

## Additional Resources

- [Nix Flakes documentation](https://nixos.wiki/wiki/Flakes)
- [Crane (Rust + Nix)](https://github.com/ipetkov/crane)
- [CloudFlare Workers + Nix](https://github.com/cloudflare/workers-sdk)
