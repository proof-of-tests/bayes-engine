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
    println!("Navigating to home page...");
    driver.goto(webapp_url).await?;
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    // Landing page should be hydrated and render hero title.
    let hero_title = driver.find(By::Css(".hero-title")).await?;
    let hero_text = hero_title.text().await?;
    assert!(
        !hero_text.trim().is_empty(),
        "Hero title should contain the total test estimate"
    );

    // If repositories are listed, the detail route should be navigable.
    if let Ok(detail_link) = driver.find(By::Css(".repo-link")).await {
        detail_link.click().await?;
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        let detail_title = driver.find(By::Css(".detail-title")).await?;
        let detail_text = detail_title.text().await?;
        assert!(
            !detail_text.trim().is_empty(),
            "Repository detail title should render after navigation"
        );
    }

    println!("All tests passed!");

    Ok(())
}
