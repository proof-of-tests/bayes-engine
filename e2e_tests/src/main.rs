use anyhow::Result;
use thirtyfour::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    // Read configuration from environment variables (set by nix build)
    let browser = std::env::var("E2E_BROWSER").unwrap_or_else(|_| "safari".to_string());
    let webdriver_port = std::env::var("WEBDRIVER_PORT").unwrap_or_else(|_| "4444".to_string());
    let wrangler_port = std::env::var("WRANGLER_PORT").unwrap_or_else(|_| "8787".to_string());

    let webdriver_url = format!("http://localhost:{}", webdriver_port);
    let webapp_url = format!("http://localhost:{}", wrangler_port);

    // Configure browser capabilities and connect based on environment
    println!("Connecting to {} via {}", browser, webdriver_url);
    let driver = match browser.as_str() {
        "safari" => {
            // Safari doesn't support headless mode via standard WebDriver
            // but safaridriver handles this automatically
            let caps = DesiredCapabilities::safari();
            WebDriver::new(&webdriver_url, caps).await?
        }
        "firefox" => {
            let mut caps = DesiredCapabilities::firefox();
            caps.set_headless()?;
            WebDriver::new(&webdriver_url, caps).await?
        }
        "chrome" => {
            let mut caps = DesiredCapabilities::chrome();
            caps.set_headless()?;
            // Disable GPU for headless Chrome (avoids some rendering issues)
            caps.add_arg("--disable-gpu")?;
            // Run in no-sandbox mode (required for some CI environments)
            caps.add_arg("--no-sandbox")?;
            // Set Chrome binary path (required when using Nix-installed Chrome)
            if let Ok(chrome_path) = std::env::var("CHROME_PATH") {
                caps.set_binary(&chrome_path)?;
            }
            WebDriver::new(&webdriver_url, caps).await?
        }
        _ => {
            anyhow::bail!(
                "Unsupported browser: {}. Use 'safari', 'firefox', or 'chrome'",
                browser
            );
        }
    };

    run(&driver, &webapp_url).await?;

    driver.quit().await?;
    Ok(())
}

pub async fn run(driver: &WebDriver, webapp_url: &str) -> Result<()> {
    // Test 1: Direct route loading
    // First test that we can directly navigate to /tests route
    println!("Testing direct navigation to /tests...");
    driver.goto(&format!("{}/tests", webapp_url)).await?;

    // Wait for the WASM to load and hydrate the page
    println!("Waiting for page to load and hydrate...");
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    // Verify we're on the tests page by checking for test-specific elements
    let counter_display = driver.find(By::Css(".counter-display")).await?;
    println!("Successfully loaded /tests route directly");

    // Now test navigation via link (existing test)
    println!("Navigating to home page...");
    driver.goto(webapp_url).await?;
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    println!("Navigating to tests page via link...");
    let tests_link = driver.find(By::Css("a[href='/tests']")).await?;
    tests_link.click().await?;

    // Wait for navigation and re-render
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    // Test 2: Counter functionality
    println!("Testing counter button...");
    // Find counter_display again since we navigated
    let counter_display = driver.find(By::Css(".counter-display")).await?;
    let initial_count = counter_display.text().await?;
    assert_eq!(initial_count, "0", "Initial count should be 0");

    let counter_button = driver.find(By::Css(".counter-button")).await?;
    counter_button.click().await?;

    // Wait a moment for the UI to update
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let new_count = counter_display.text().await?;
    assert_eq!(new_count, "1", "Count should be 1 after clicking");

    // Test 3: Uppercase API functionality
    println!("Testing uppercase API...");
    let text_input = driver.find(By::Css(".text-input")).await?;
    text_input.send_keys("hello world").await?;

    let uppercase_button = driver.find(By::Css(".uppercase-button")).await?;
    uppercase_button.click().await?;

    // Wait for the API call to complete
    // The button text changes from "Convert to Uppercase" to "Converting..." and back
    // Wait for the result to appear
    driver
        .query(By::Css(".result-text"))
        .wait(
            std::time::Duration::from_secs(5),
            std::time::Duration::from_millis(100),
        )
        .first()
        .await?;

    let result_text = driver.find(By::Css(".result-text")).await?.text().await?;
    assert_eq!(result_text, "HELLO WORLD", "Result should be uppercase");

    // Test 3: WASM Executor functionality
    println!("Testing WASM executor...");
    if let Ok(wasm_path) = std::env::var("SIMPLE_WASM_MODULE") {
        // Find the file input element
        let file_input = driver
            .find(By::Css("input[type='file'][accept='.wasm']"))
            .await?;

        // Upload the WASM file
        file_input.send_keys(&wasm_path).await?;

        // Wait for the file to be uploaded and processed
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // Find and click the execute button
        let execute_button = driver.find(By::Css(".execute-button")).await?;
        execute_button.click().await?;

        // Wait for execution to complete
        driver
            .query(By::Css(".output-display"))
            .wait(
                std::time::Duration::from_secs(5),
                std::time::Duration::from_millis(100),
            )
            .first()
            .await?;

        // Check the output
        let output = driver
            .find(By::Css(".output-display"))
            .await?
            .text()
            .await?;
        assert!(
            output.contains("add(10, 32) = 42"),
            "WASM execution should produce correct result, got: {}",
            output
        );

        println!("WASM executor test passed!");
    } else {
        println!("Skipping WASM executor test (SIMPLE_WASM_MODULE not set)");
    }

    println!("All tests passed!");

    Ok(())
}
