# Tool Usage Cheatsheet

This document provides quick reference for Claude Code tool usage when working on this project.

## File Operations

### Reading Files

```
✅ DO: Use Read tool for reading files
❌ DON'T: Use Bash with cat, head, tail

# Correct
Read(file_path="/path/to/file.rs")

# Incorrect
Bash("cat /path/to/file.rs")
```

### Finding Files

```
✅ DO: Use Glob for pattern matching
❌ DON'T: Use Bash with find or ls

# Correct
Glob(pattern="**/*.rs")
Glob(pattern="**/test*.rs")

# Incorrect
Bash("find . -name '*.rs'")
Bash("ls -la **/*.rs")
```

### Searching Code

```
✅ DO: Use Grep for content search
❌ DON'T: Use Bash with grep or rg

# Correct
Grep(pattern="BayesianEngine", output_mode="files_with_matches")
Grep(pattern="fn main", output_mode="content", -n=true)

# Incorrect
Bash("grep -r 'BayesianEngine' .")
Bash("rg 'fn main'")
```

### Editing Files

```
✅ DO: Use Edit for modifications
❌ DON'T: Use Bash with sed or awk

# Correct
Edit(file_path="/path/to/file.rs", old_string="...", new_string="...")

# Incorrect
Bash("sed -i 's/old/new/' file.rs")
```

### Writing Files

```
✅ DO: Use Write for new files
❌ DON'T: Use Bash with echo or cat

# Correct
Write(file_path="/path/to/file.rs", content="...")

# Incorrect
Bash("echo 'content' > file.rs")
Bash("cat <<EOF > file.rs")
```

## Codebase Exploration

### Quick Needle Searches

For specific files, classes, or functions:

```
✅ DO: Use Glob or Grep directly
❌ DON'T: Use Task agent for simple searches

# Finding a specific file
Glob(pattern="**/BayesianEngine.rs")

# Finding a class definition
Grep(pattern="class BayesianEngine")
```

### Open-Ended Exploration

For understanding codebase structure or answering questions:

```
✅ DO: Use Task tool with Explore agent
❌ DON'T: Run multiple Glob/Grep directly

# Correct
Task(
  subagent_type="Explore",
  description="Find error handling code",
  prompt="Find and explain how errors from the client are handled in the codebase"
)

# When to use Explore agent:
- "Where are errors from the client handled?"
- "What is the codebase structure?"
- "How does authentication work?"
```

## Git Operations

### Viewing Changes

```
✅ DO: Use Bash for git commands
✅ DO: Run multiple independent git commands in parallel

# Correct - parallel git commands
Bash("git status")
Bash("git diff")
Bash("git log --oneline")

# Correct - sequential dependent commands
Bash("git add . && git commit -m 'message'")
```

### Committing Changes

See the Git Safety Protocol in [WORKFLOW.md](WORKFLOW.md). Key points:

```
✅ DO: Follow commit workflow
1. Bash("git status")
2. Bash("git diff")
3. Bash("git log --oneline")
4. Bash("git add <files> && git commit -m 'message'")
5. Bash("git status")

❌ DON'T:
- Include "Co-Authored-By: Claude" in commits
- Include "Generated with Claude Code" in PR descriptions
- Use --amend unless explicitly requested or adding pre-commit hook edits
- Use --force or --no-verify without user confirmation
```

### Creating Pull Requests

```
✅ DO: Use Bash with gh command
✅ DO: Check full commit history since branch diverged

# Correct workflow
1. Bash("git status")
2. Bash("git diff")
3. Bash("git log --oneline origin/main..HEAD")
4. Bash("git diff origin/main...HEAD")
5. Bash("git push -u origin <branch>")
6. Bash('gh pr create --title "..." --body "$(cat <<EOF\n...\nEOF\n)"')

❌ DON'T:
- Include "Test Plan" section (CI handles testing)
- Include AI attribution in PR descriptions
```

## Testing

### Running Tests

```
✅ DO: Use Bash for test commands

# Nix flake checks
Bash("nix flake check")

# End-to-end tests
Bash("nix run .#run-e2e-tests")

# Rust tests only
Bash("cargo test")
```

## Task Management

### Using TodoWrite

```
✅ DO: Use for complex multi-step tasks (3+ steps)
✅ DO: Update status in real-time
✅ DO: Mark exactly ONE task in_progress at a time
✅ DO: Mark completed immediately after finishing

❌ DON'T: Use for single, trivial tasks
❌ DON'T: Batch multiple completions
❌ DON'T: Mark as completed if tests fail or errors occur

# Example
TodoWrite(todos=[
  {content: "Run tests", status: "in_progress", activeForm: "Running tests"},
  {content: "Fix type errors", status: "pending", activeForm: "Fixing type errors"}
])
```

## Parallel vs Sequential Operations

### Parallel (Independent)

```
✅ DO: Call multiple tools in single message when independent

# Correct - parallel independent operations
Bash("git status")
Bash("git diff")
Bash("git log --oneline")
```

### Sequential (Dependent)

```
✅ DO: Use && for dependent commands
❌ DON'T: Call tools in parallel when one depends on another

# Correct - sequential dependent operations
Bash("git add . && git commit -m 'message' && git push")

# When to use sequential:
- mkdir before cp
- Write file before git add
- git add before git commit
```

## Communication

### Outputting Information

```
✅ DO: Output text directly to user
❌ DON'T: Use Bash echo or comments

# Correct
"I found 10 type errors and will fix them now."

# Incorrect
Bash("echo 'Found 10 type errors'")
# Don't use code comments to communicate with user
```

## Best Practices

### 1. Choose the Right Tool

**File Operations:**

- **Read file**: Use `Read` tool (NOT `cat`)
- **Find files**: Use `Glob` tool (NOT `find` or `ls`)
- **Search code**: Use `Grep` tool (NOT `grep` or `rg`)
- **Edit file**: Use `Edit` tool (NOT `sed` or `awk`)
- **Write file**: Use `Write` tool (NOT `echo >` or `cat <<EOF`)

**Codebase Exploration:**

- **Explore codebase**: Use `Task(Explore)` (NOT multiple `Grep` calls)

**Terminal Operations:**

- **Git operations**: Use `Bash` tool
- **Terminal commands**: Use `Bash` tool

### 2. Efficiency

- Run independent operations in parallel
- Use specialized tools over bash alternatives
- Use Task(Explore) for complex searches

### 3. Safety

- Always read files before editing/writing
- Never use --force without confirmation
- Test changes before committing

### 4. Clarity

- Output text directly (not via echo)
- Use descriptive task descriptions
- Keep todo lists up-to-date

## Common Patterns

### Feature Implementation

```
1. TodoWrite - Plan tasks
2. Glob/Read - Explore existing code
3. Write/Edit - Implement changes
4. Bash - Run tests
5. Bash - Commit changes
6. TodoWrite - Update progress
```

### Bug Fix

```
1. Grep - Find relevant code
2. Read - Understand context
3. Edit - Fix issue
4. Bash - Test fix
5. Bash - Commit
```

### Code Exploration

```
1. Task(Explore) - Understand structure
2. Read - Dive into specific files
3. Communicate findings
```

## Anti-Patterns

### ❌ Using Bash for File Operations

```
# DON'T
Bash("cat file.rs")
Bash("grep 'pattern' .")
Bash("find . -name '*.rs'")

# DO
Read("file.rs")
Grep(pattern="pattern")
Glob(pattern="**/*.rs")
```

### ❌ Manual Exploration Instead of Task(Explore)

```
# DON'T (for open-ended questions)
Glob("**/error*")
Grep("error handling")
Read("src/errors.rs")
Read("src/lib.rs")
...

# DO
Task(
  subagent_type="Explore",
  prompt="Explain how error handling works in this codebase"
)
```

### ❌ Using echo for Communication

```
# DON'T
Bash("echo 'Starting to fix errors'")

# DO
"Starting to fix errors now."
```

### ❌ Premature Completion

```
# DON'T
TodoWrite(todos=[
  {content: "Run tests", status: "completed", ...}
])
# But tests actually failed!

# DO
TodoWrite(todos=[
  {content: "Run tests", status: "in_progress", ...}
])
# Test output shows failures
TodoWrite(todos=[
  {content: "Run tests", status: "in_progress", ...},
  {content: "Fix failing test: test_example", status: "pending", ...}
])
```
