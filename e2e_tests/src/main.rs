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
        _ => {
            anyhow::bail!(
                "Unsupported browser: {}. Use 'safari' or 'firefox'",
                browser
            );
        }
    };

    run(&driver, &webapp_url).await?;

    driver.quit().await?;
    Ok(())
}

pub async fn run(driver: &WebDriver, webapp_url: &str) -> Result<()> {
    // Navigate to the app running on the configured port
    driver.goto(webapp_url).await?;

    // Wait for the WASM to load and hydrate the page
    println!("Waiting for page to load and hydrate...");
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    // Test 1: Counter functionality
    println!("Testing counter button...");
    let counter_display = driver.find(By::Css(".counter-display")).await?;
    let initial_count = counter_display.text().await?;
    assert_eq!(initial_count, "0", "Initial count should be 0");

    let counter_button = driver.find(By::Css(".counter-button")).await?;
    counter_button.click().await?;

    // Wait a moment for the UI to update
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let new_count = counter_display.text().await?;
    assert_eq!(new_count, "1", "Count should be 1 after clicking");

    // Test 2: Uppercase API functionality
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

    println!("All tests passed!");

    Ok(())
}
