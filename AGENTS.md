# Agent Development Guide

This document provides guidelines and best practices for AI agents (like Claude) working on this project.

## Quick Links

- **@agents/WORKFLOW.md** - Git workflow, branching, committing, and PRs
- **@agents/NIX.md** - Nix commands, builds, and CI integration
- **@agents/TESTING.md** - Unit tests, integration tests, and E2E tests
- **@agents/DEPLOYMENT.md** - CloudFlare Workers deployment and monitoring
- **@agents/TOOLS.md** - Claude Code tool best practices

## Project Structure

```
.
├── agents/           # Agent documentation (you are here)
├── client/           # Dioxus web UI (WASM)
├── server/           # CloudFlare Worker (WASM)
├── e2e_tests/        # End-to-end tests (Selenium)
├── nix/              # Nix build scripts
├── .github/          # CI/CD workflows
├── flake.nix         # Nix flake configuration
├── wrangler.toml     # CloudFlare Workers config
├── Cargo.toml        # Rust workspace
└── LICENSE           # Public domain license
```

## Quick Start

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

## Development Workflow Summary

See @agents/WORKFLOW.md for complete details.

1. **Create branch** from `origin/main` with type prefix (feat/, fix/, etc.)
2. **Make changes** following code standards
3. **Test locally** with `nix flake check` and `nix run .#run-e2e-tests`
4. **Commit** using Conventional Commits format
5. **Push and create PR** with concise description
6. **Review and merge** after CI passes

## Code Standards

### General Principles

- **Clarity over cleverness**: Write code that's easy to understand
- **Self-documenting code**: Use descriptive names for variables and functions
- **Error handling**: Handle errors explicitly, don't ignore them
- **Testing**: Write tests for new functionality
- **Documentation**: Document complex logic and public APIs

### Naming Conventions

- Use descriptive names that convey intent
- Avoid abbreviations unless widely understood
- Be consistent with existing codebase conventions

### Comments

- Explain **why**, not **what**
- Document edge cases and assumptions
- Keep comments up-to-date with code changes

## Testing

See @agents/TESTING.md for complete details.

### Quick Reference

```bash
# Run all checks (formatting, linting, tests, build)
nix flake check

# Run end-to-end tests
nix run .#run-e2e-tests

# Run Rust unit tests only
cargo test
```

### Test Types

1. **Unit Tests**: Test individual functions/methods in isolation
2. **Integration Tests**: Test how components work together
3. **End-to-End Tests**: Test complete workflows through browser

### Testing Best Practices

- Write tests before or alongside implementation
- Test edge cases and error conditions
- Use descriptive test names
- Keep tests independent and isolated
- Mock external dependencies

## Nix & Build System

See @agents/NIX.md for complete details.

### Quick Reference

```bash
# Enter development shell
nix develop

# Build webapp (client + server WASM)
nix build .#webapp

# Run locally
nix run .#wrangler-dev

# Run all checks
nix flake check

# Format Nix files
nix fmt
```

## Deployment

See @agents/DEPLOYMENT.md for complete details.

- **Production**: Deployed automatically on push to `main`
- **PR Previews**: Each PR gets a unique preview URL
- **Platform**: CloudFlare Workers (edge serverless)
- **Monitoring**: CloudFlare dashboard + `wrangler tail`

## Experience Log

Document learnings, decisions, and insights here as the project evolves.

### [2025-10-21] High-Level Documentation Structure

- Created dedicated documentation guides:
  - agents/WORKFLOW.md - Development workflow
  - agents/NIX.md - Nix commands and builds
  - agents/TESTING.md - Testing at all levels
  - agents/DEPLOYMENT.md - CloudFlare deployment
  - agents/TOOLS.md - Claude Code tool usage
- Updated AGENTS.md to serve as navigation hub

______________________________________________________________________

## Tips for AI Agents

- **Read before writing**: Always read existing files before modifying them
- **Follow conventions**: Match the style and patterns already in the codebase
- **Test your changes**: Run `nix flake check` and `nix run .#run-e2e-tests` after making changes
- **Small commits**: Make focused commits with clear messages
- **Ask when unclear**: Use the AskUserQuestion tool if requirements are ambiguous
- **Document as you go**: Update docs when adding features
- **Use todo lists**: Track multi-step tasks with the TodoWrite tool
- **Check the guides**: Refer to agents/\*.md for detailed workflows and best practices
- **Use right tools**: See @agents/TOOLS.md for Claude Code tool usage guidelines
