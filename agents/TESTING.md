# Testing Guide

This project uses multiple levels of testing to ensure code quality and correctness.

## Testing Levels

### 1. Unit Tests (Rust)

Test individual functions and modules in isolation.

#### Running Unit Tests

```bash
# All tests via Nix
nix build .#checks.x86_64-linux.rust-test

# All tests via Cargo
cargo test

# Specific package
cargo test -p server
cargo test -p client

# Specific test
cargo test test_name

# With output
cargo test -- --nocapture
```

#### Writing Unit Tests

```rust
// In src/lib.rs or src/module.rs

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_example() {
        // Arrange
        let input = 42;

        // Act
        let result = my_function(input);

        // Assert
        assert_eq!(result, 84);
    }

    #[test]
    fn test_error_case() {
        let result = my_function_that_fails();
        assert!(result.is_err());
    }
}
```

#### Unit Test Best Practices

- **Test one thing** per test
- **Use descriptive names**: `test_addition_with_positive_numbers`
- **Follow AAA pattern**: Arrange, Act, Assert
- **Test edge cases**: Empty inputs, boundary values, error conditions
- **Mock external dependencies** (network, filesystem, etc.)

### 2. Integration Tests (Rust)

Test how components work together within a single package.

#### Location

- `server/tests/*.rs` - Server integration tests
- `client/tests/*.rs` - Client integration tests

#### Writing Integration Tests

```rust
// In server/tests/integration_test.rs

use server::{handler, Request};

#[test]
fn test_api_endpoint() {
    let request = Request::new("test input");
    let response = handler(request);
    assert_eq!(response.status, 200);
}
```

### 3. End-to-End Tests (Selenium)

Test complete workflows through the browser using Selenium WebDriver.

#### Running E2E Tests

```bash
# Via Nix (recommended)
nix run .#run-e2e-tests

# This will:
# 1. Build the webapp
# 2. Start geckodriver (Firefox WebDriver)
# 3. Start wrangler dev server
# 4. Run e2e_tests binary
# 5. Clean up services
```

#### E2E Test Structure

Located in `e2e_tests/src/main.rs`:

```rust
use anyhow::Result;
use thirtyfour::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    let webdriver_url = "http://localhost:4444";
    let webapp_url = "http://localhost:8787";

    let mut caps = DesiredCapabilities::firefox();
    caps.set_headless()?;
    let driver = WebDriver::new(webdriver_url, caps).await?;

    run(&driver, webapp_url).await?;

    driver.quit().await?;
    Ok(())
}

pub async fn run(driver: &WebDriver, webapp_url: &str) -> Result<()> {
    driver.goto(webapp_url).await?;

    // Wait for WASM to load
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    // Test counter functionality
    let counter_button = driver.find(By::Css(".counter-button")).await?;
    counter_button.click().await?;

    let counter_display = driver.find(By::Css(".counter-display")).await?;
    assert_eq!(counter_display.text().await?, "1");

    Ok(())
}
```

#### E2E Test Configuration

Environment variables (set by `nix run .#run-e2e-tests`):

- `E2E_BROWSER`: Browser to use (firefox, chrome, safari)
- `WEBDRIVER_PORT`: WebDriver server port (default: 4444)
- `WRANGLER_PORT`: Wrangler dev server port (default: 8787)

#### Adding New E2E Tests

1. Add test logic to `e2e_tests/src/main.rs::run()`
2. Use CSS selectors to find elements
3. Wait for async operations to complete
4. Assert expected behavior

Example:

```rust
// Test new feature
let button = driver.find(By::Css(".my-button")).await?;
button.click().await?;

// Wait for result
driver
    .query(By::Css(".result"))
    .wait(Duration::from_secs(5), Duration::from_millis(100))
    .first()
    .await?;

let result = driver.find(By::Css(".result")).await?.text().await?;
assert_eq!(result, "Expected Value");
```

### 4. CI Testing

All tests run automatically in CI on every push and PR.

#### CI Test Pipeline

From `.github/workflows/ci.yml`:

```yaml
jobs:
  nix-checks:
    - nix flake check  # Runs all checks including:
        - rust-fmt (formatting)
        - rust-clippy (linting)
        - rust-test (unit tests)
        - rust-build (compilation)
        - markdown-format
        - nix-format
        - shellcheck

  e2e-tests:
    - nix run .#run-e2e-tests  # End-to-end tests
```

## Testing Strategy

### When to Write Each Test Type

**Unit Tests:**

- **When**: Pure functions, business logic
- **Example**: Bayesian calculations, data transformations

**Integration Tests:**

- **When**: API endpoints, module interactions
- **Example**: HTTP handlers, database queries

**End-to-End Tests:**

- **When**: User workflows, UI interactions
- **Example**: Button click → API → Result display

### Test Pyramid

```
     E2E Tests (Few)
         /\
        /  \
       /    \
      /------\
     / Integration (Some)
    /----------\
   /   Unit     \
  /--------------\
 /  (Many)       \
------------------
```

- **Many unit tests**: Fast, focused, easy to maintain
- **Some integration tests**: Test component interactions
- **Few e2e tests**: Cover critical user workflows

## Running All Tests

### Local Development

```bash
# All checks (fast)
nix flake check

# With e2e tests (slower)
nix flake check && nix run .#run-e2e-tests
```

### Before Committing

```bash
# Run all checks
nix flake check

# Run e2e tests
nix run .#run-e2e-tests

# If all pass, commit
git add .
git commit -m "feat: add new feature"
```

## Test-Driven Development (TDD)

### TDD Workflow

1. **Write failing test**

   ```rust
   #[test]
   fn test_new_feature() {
       let result = new_feature(42);
       assert_eq!(result, 84);
   }
   ```

2. **Run test (it fails)**

   ```bash
   cargo test test_new_feature
   # Test fails because new_feature doesn't exist
   ```

3. **Implement minimum code to pass**

   ```rust
   fn new_feature(x: i32) -> i32 {
       x * 2
   }
   ```

4. **Run test (it passes)**

   ```bash
   cargo test test_new_feature
   # Test passes
   ```

5. **Refactor** (optional)

   - Improve code quality
   - Tests should still pass

6. **Repeat** for next feature

## Debugging Tests

### Unit/Integration Tests

```bash
# Run with output
cargo test -- --nocapture

# Run specific test
cargo test test_name -- --nocapture

# Run with backtrace
RUST_BACKTRACE=1 cargo test
```

### E2E Tests

```bash
# Check logs
tail -f wrangler.log
tail -f geckodriver.log

# Run with visible browser (remove headless)
# Edit e2e_tests/src/main.rs:
# caps.set_headless()?;  // Comment this line

# Use different browser
E2E_BROWSER=chrome nix run .#run-e2e-tests
```

## Common Testing Patterns

### Testing Async Code

```rust
#[tokio::test]
async fn test_async_function() {
    let result = async_function().await;
    assert_eq!(result, expected_value);
}
```

### Testing Error Cases

```rust
#[test]
fn test_error_handling() {
    let result = function_that_fails();
    assert!(result.is_err());

    match result {
        Err(e) => assert_eq!(e.to_string(), "Expected error message"),
        Ok(_) => panic!("Expected error"),
    }
}
```

### Testing with Mock Data

```rust
#[test]
fn test_with_mock() {
    let mock_data = vec![1, 2, 3];
    let result = process_data(mock_data);
    assert_eq!(result, vec![2, 4, 6]);
}
```

### Parameterized Tests

```rust
#[test]
fn test_multiple_cases() {
    let cases = vec![
        (1, 2),
        (2, 4),
        (3, 6),
    ];

    for (input, expected) in cases {
        assert_eq!(double(input), expected);
    }
}
```

## Test Coverage

### Viewing Coverage

```bash
# Install tarpaulin
cargo install cargo-tarpaulin

# Generate coverage report
cargo tarpaulin --out Html

# Open report
open tarpaulin-report.html
```

### Coverage Goals

- **Unit tests**: Aim for >80% coverage of business logic
- **Integration tests**: Cover main API endpoints
- **E2E tests**: Cover critical user journeys

## Troubleshooting

### Tests Pass Locally, Fail in CI

- Ensure deterministic behavior (no random values, fixed timestamps)
- Check for environment-specific assumptions
- Verify CI has all required dependencies

### E2E Tests Timing Out

- Increase wait timeouts
- Add more explicit waits for async operations
- Check if WASM is loading correctly

### Flaky Tests

- Add explicit waits instead of fixed sleeps
- Use `driver.query().wait()` for element appearance
- Ensure tests are independent (no shared state)

### Tests are Slow

- Run unit tests in parallel (Cargo does this by default)
- Mock expensive operations
- Use smaller test datasets
- Consider splitting long-running tests

## Best Practices

01. **Write tests first** or alongside implementation
02. **Test edge cases**: null, empty, boundary values
03. **Use descriptive test names**: `test_handles_empty_input`
04. **Keep tests independent**: No shared state between tests
05. **Mock external dependencies**: Network, filesystem, time
06. **Make tests deterministic**: Same input = same output
07. **Run tests frequently**: After every change
08. **Keep tests fast**: Unit tests \<1ms, integration \<100ms
09. **Document complex test setups**: Use comments
10. **Clean up test data**: Don't leave artifacts

## Resources

- [Rust Testing Documentation](https://doc.rust-lang.org/book/ch11-00-testing.html)
- [Thirtyfour (Selenium for Rust)](https://github.com/stevepryde/thirtyfour)
- [Tokio Testing](https://tokio.rs/tokio/topics/testing)
- [Test-Driven Development](https://en.wikipedia.org/wiki/Test-driven_development)
