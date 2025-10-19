# Agent Development Guide

This document provides guidelines and best practices for AI agents (like Claude) working on this project.

## Project Structure

```
.
├── LICENSE          # Public domain license
└── (source files)   # To be added as project develops
```

## Development Workflow

### 1. Create a Feature Branch

**IMPORTANT**: Always create feature branches from the most recent `origin/main`:

```bash
git fetch origin
git checkout -b <type>/<brief-description> origin/main
```

Branch types:
- `feat/` - New features
- `fix/` - Bug fixes
- `docs/` - Documentation changes
- `refactor/` - Code refactoring
- `test/` - Test additions or modifications
- `chore/` - Maintenance tasks

### 2. Make Changes

Follow the code standards and testing guidelines in this document.

### 3. Commit Changes

Use [Conventional Commits](https://www.conventionalcommits.org/) format:

```
<type>(<scope>): <description>

[optional body]

[optional footer]
```

**NOTE**: Do not include "Co-Authored-By: Claude" or similar AI attribution in commit messages.

Examples:
```bash
git commit -m "feat: add Bayesian inference engine"
git commit -m "fix(parser): handle edge case in probability parsing"
git commit -m "docs: update README with installation instructions"
```

### 4. Push and Create PR

```bash
git push -u origin <branch-name>
gh pr create --title "Title" --body "Description"
```

**NOTE**: Do not include "Generated with Claude Code" or similar AI attribution in PR descriptions.

### 5. Review and Merge

Address review comments, then merge when approved.

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

### Test Types

1. **Unit Tests**: Test individual functions/methods in isolation
2. **Integration Tests**: Test how components work together
3. **End-to-End Tests**: Test complete workflows

### Testing Best Practices

- Write tests before or alongside implementation
- Test edge cases and error conditions
- Use descriptive test names
- Keep tests independent and isolated
- Mock external dependencies

## Common Commands

### Git Operations

```bash
# Check status
git status

# View changes
git diff

# View commit history
git log --oneline --graph

# Create and switch to new branch
git checkout -b <branch-name>

# Push branch to remote
git push -u origin <branch-name>

# Update from main
git fetch origin
git rebase origin/main
```

### GitHub CLI

```bash
# Create PR
gh pr create

# View PR status
gh pr status

# List PRs
gh pr list

# View PR in browser
gh pr view --web

# Check CI status
gh pr checks
```

## Implementation Patterns

### Testing Pattern

Structure tests clearly:

```
# Arrange: Set up test data
# Act: Execute the code being tested
# Assert: Verify the results
```

## Experience Log

Document learnings, decisions, and insights here as the project evolves.

### [YYYY-MM-DD] Initial Setup

- Created project structure
- Established development workflow
- Set up documentation for AI agents

---

## Tips for AI Agents

- **Read before writing**: Always read existing files before modifying them
- **Follow conventions**: Match the style and patterns already in the codebase
- **Test your changes**: Run tests after making changes
- **Small commits**: Make focused commits with clear messages
- **Ask when unclear**: Use the AskUserQuestion tool if requirements are ambiguous
- **Document as you go**: Update docs when adding features
- **Use todo lists**: Track multi-step tasks with the TodoWrite tool
