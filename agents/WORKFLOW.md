# Development Workflow

This document outlines the standard workflow for making changes to the project.

## Quick Reference

```bash
# 1. Create feature branch
git fetch origin
git checkout -b <type>/<brief-description> origin/main

# 2. Make changes and test
nix flake check
nix run .#run-e2e-tests

# 3. Commit changes
git add <files>
git commit -m "<type>(<scope>): <description>"

# 4. Push and create PR
git push -u origin <branch-name>
gh pr create --title "Title" --body "Description"
```

## Detailed Workflow

### 1. Create a Feature Branch

**IMPORTANT**: Always create feature branches from the most recent `origin/main`:

```bash
git fetch origin
git checkout -b <type>/<brief-description> origin/main
```

#### Branch Types

- `feat/` - New features
- `fix/` - Bug fixes
- `docs/` - Documentation changes
- `refactor/` - Code refactoring
- `test/` - Test additions or modifications
- `chore/` - Maintenance tasks

#### Branch Naming Examples

- `feat/bayesian-inference` - Adding Bayesian inference
- `fix/parser-edge-case` - Fixing parser edge case
- `docs/api-documentation` - Adding API docs
- `refactor/error-handling` - Refactoring error handling
- `test/integration-tests` - Adding integration tests

### 2. Make Changes

Follow the code standards and testing guidelines in [AGENTS.md](../AGENTS.md).

#### Development Commands

```bash
# Enter Nix development shell
nix develop

# Build the project
nix build

# Run locally
nix run .#wrangler-dev

# Run all checks
nix flake check

# Run e2e tests
nix run .#run-e2e-tests
```

### 3. Commit Changes

Use [Conventional Commits](https://www.conventionalcommits.org/) format:

```
<type>(<scope>): <description>

[optional body]

[optional footer]
```

#### Commit Types

- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation changes
- `refactor`: Code refactoring
- `test`: Test changes
- `chore`: Build process, tooling, or dependency updates
- `perf`: Performance improvements
- `style`: Code style changes (formatting, etc.)

#### Commit Examples

```bash
# Feature addition
git commit -m "feat: add Bayesian inference engine"

# Bug fix with scope
git commit -m "fix(parser): handle edge case in probability parsing"

# Documentation
git commit -m "docs: update README with installation instructions"

# Breaking change
git commit -m "feat!: change API response format

BREAKING CHANGE: API now returns JSON instead of XML"
```

#### Important Notes

- **DO NOT** include "Co-Authored-By: Claude" or similar AI attribution in commit messages
- Keep commit messages concise and descriptive
- Focus on **what** and **why**, not **how**

### 4. Push and Create PR

```bash
# Push branch to remote
git push -u origin <branch-name>

# Create pull request
gh pr create --title "Title" --body "Description"
```

#### PR Guidelines

- **Title**: Use same format as commit messages (e.g., `feat: add Bayesian inference engine`)
- **Description**: Short and concise explanation of changes
- **DO NOT** include "Test Plan" section - PRs are automatically tested in CI
- **DO NOT** include "Generated with Claude Code" or similar AI attribution

#### PR Description Example

```markdown
## Summary

Adds Bayesian inference engine with support for:

- Prior probability calculations
- Posterior updates
- Multiple evidence sources

## Changes

- Added `BayesianEngine` struct
- Implemented prior/posterior calculations
- Added unit tests and integration tests
```

### 5. Review and Merge

1. **CI Validation**: Wait for CI to pass
   - `nix flake check` runs all checks
   - `nix run .#run-e2e-tests` runs end-to-end tests
2. **Code Review**: Address review comments if needed
3. **Merge**: Merge when approved and CI passes
4. **Deployment**: Changes to `main` are automatically deployed to CloudFlare Workers

## Common Tasks

### Updating Dependencies

#### Rust Dependencies

```bash
# Update a specific dependency
cargo update -p <package-name>

# Update all dependencies
cargo update

# Commit changes
git add Cargo.lock
git commit -m "chore(deps): update Rust dependencies"
```

#### Nix Dependencies

```bash
# Update all flake inputs
nix flake update

# Update specific input
nix flake lock --update-input <input-name>

# Commit changes
git add flake.lock
git commit -m "chore(deps): update Nix flake inputs"
```

### Rebasing on Main

```bash
# Fetch latest changes
git fetch origin

# Rebase your branch
git rebase origin/main

# If conflicts occur, resolve them and continue
git add <resolved-files>
git rebase --continue

# Force push (if already pushed)
git push --force-with-lease
```

### Fixing CI Failures

If CI fails:

1. **Check CI logs** for specific failures
2. **Run checks locally**:
   ```bash
   nix flake check
   nix run .#run-e2e-tests
   ```
3. **Fix issues** and commit
4. **Push fixes** - CI will re-run automatically

### Interactive Development

```bash
# Start local development server
nix run .#wrangler-dev

# In another terminal, run tests
nix run .#run-e2e-tests
```

## Git Tips

### View Changes

```bash
# View unstaged changes
git diff

# View staged changes
git diff --cached

# View changes in specific file
git diff <file-path>

# View commit history
git log --oneline --graph
```

### Stashing Changes

```bash
# Stash current changes
git stash

# View stashed changes
git stash list

# Apply most recent stash
git stash pop

# Apply specific stash
git stash apply stash@{n}
```

### Undoing Changes

```bash
# Unstage a file
git reset HEAD <file>

# Discard changes in working directory
git checkout -- <file>

# Undo last commit (keep changes)
git reset --soft HEAD~1

# Undo last commit (discard changes)
git reset --hard HEAD~1
```

## Best Practices

1. **Keep branches short-lived** - Aim to merge within 1-3 days
2. **Make small, focused commits** - Easier to review and revert
3. **Test before committing** - Run `nix flake check` locally
4. **Write clear commit messages** - Help reviewers understand changes
5. **Rebase frequently** - Keep your branch up-to-date with main
6. **Use draft PRs** for work in progress
7. **Request reviews early** - Don't wait until everything is perfect

## Troubleshooting

### Branch is behind main

```bash
git fetch origin
git rebase origin/main
```

### Need to change last commit message

```bash
# If not yet pushed
git commit --amend

# If already pushed
git commit --amend
git push --force-with-lease
```

### Accidentally committed to main

```bash
# Create new branch with current changes
git branch <new-branch-name>

# Reset main to origin
git reset --hard origin/main

# Switch to new branch
git checkout <new-branch-name>
```
