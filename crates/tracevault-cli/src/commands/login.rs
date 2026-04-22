use crate::api_client::ApiClient;
use crate::credentials::Credentials;

/// Decide whether we're probably in a headless environment where opening a
/// browser will fail. Errs on the side of *not* opening when we're unsure,
/// since printing the URL is always safe and opening a browser in a
/// Docker/CI/SSH session usually fails noisily.
fn is_headless() -> bool {
    // Explicit opt-out always wins.
    if std::env::var_os("TRACEVAULT_NO_BROWSER").is_some() {
        return true;
    }
    // Common CI indicators.
    if std::env::var_os("CI").is_some() || std::env::var_os("GITHUB_ACTIONS").is_some() {
        return true;
    }
    // Typical "running inside a container" hint. Not bulletproof — some
    // desktop containers do have a browser — but a strong signal.
    if std::path::Path::new("/.dockerenv").exists() {
        return true;
    }
    // On Linux/BSD, a graphical session needs one of these env vars. macOS
    // and Windows don't use them, so only apply this check on Unix-like
    // platforms that aren't macOS.
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if std::env::var_os("DISPLAY").is_none() && std::env::var_os("WAYLAND_DISPLAY").is_none() {
            return true;
        }
    }
    false
}

fn print_url_banner(url: &str) {
    // Make the URL impossible to miss. Plain ASCII so it renders the same
    // in every terminal, including dumb ones.
    let bar = "─".repeat(70);
    println!();
    println!("┌{bar}┐");
    println!(
        "│ Open this URL in a browser to finish logging in:{:>20} │",
        ""
    );
    println!("│{:^70}│", "");
    println!("│ {url:<68} │");
    println!("│{:^70}│", "");
    println!("└{bar}┘");
    println!();
}

pub async fn login(server_url: &str, no_browser: bool) -> Result<(), Box<dyn std::error::Error>> {
    let client = ApiClient::new(server_url, None);

    let device = client.device_start().await?;

    // Browser URL is always built from server_url. In production a reverse
    // proxy routes /api/* to the backend and everything else to the
    // SvelteKit frontend on the same domain.
    let full_url = format!(
        "{}/auth/device?token={}",
        server_url.trim_end_matches('/'),
        device.token
    );

    print_url_banner(&full_url);

    let skip_browser = no_browser || is_headless();
    if skip_browser {
        println!("Not attempting to auto-open a browser (headless environment detected or --no-browser set).");
    } else {
        println!("Attempting to open the URL in your default browser...");
        if let Err(e) = open::that(&full_url) {
            // Non-fatal: the URL is already visible above, the user can
            // just copy it.
            eprintln!("Could not open browser automatically: {e}");
            eprintln!("Copy the URL above into a browser manually.");
        }
    }

    // Poll for approval
    print!("Waiting for authentication in the browser");
    use std::io::Write;
    let _ = std::io::stdout().flush();

    let poll_interval = std::time::Duration::from_secs(2);
    let max_attempts = 150; // 5 minutes at 2s intervals

    for _ in 0..max_attempts {
        tokio::time::sleep(poll_interval).await;
        print!(".");
        let _ = std::io::stdout().flush();

        match client.device_status(&device.token).await {
            Ok(status) => {
                if status.status == "approved" {
                    let token = status
                        .token
                        .ok_or("Server approved but did not return a session token")?;
                    let email = status
                        .email
                        .ok_or("Server approved but did not return an email")?;

                    println!(" done!");
                    println!();
                    println!("Logged in as {email}");

                    let creds = Credentials {
                        server_url: server_url.to_string(),
                        token,
                        email,
                    };
                    creds.save()?;
                    println!("Credentials saved to {}", Credentials::path().display());
                    return Ok(());
                }
                // Still pending, continue polling
            }
            Err(e) => {
                eprintln!("\nError polling status: {e}");
                return Err(e);
            }
        }
    }

    Err("Authentication timed out after 5 minutes".into())
}

#[cfg(test)]
mod tests {
    use super::is_headless;

    #[test]
    fn tracevault_no_browser_env_forces_headless() {
        // SAFETY: test-scoped env mutation. serial_test is not available, but
        // this test only reads a variable name nothing else touches.
        unsafe {
            std::env::set_var("TRACEVAULT_NO_BROWSER", "1");
        }
        assert!(is_headless());
        unsafe {
            std::env::remove_var("TRACEVAULT_NO_BROWSER");
        }
    }
}
