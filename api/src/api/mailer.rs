/// `theme` mirrors the frontend's AccentProvider ("blue" | "green") so the
/// email matches whatever accent the user had active when they triggered the
/// send. Any other value is treated as "blue" by the mailer worker.
pub async fn send_code(email: &str, code: &str, theme: &str) -> Result<(), String> {
    let mut req = reqwest::Client::new()
        .post("https://stellar.mailer.primelendrow.com")
        .json(&serde_json::json!({ "email": email, "code": code, "theme": theme }));

    // Attach shared secret so the worker rejects calls not from this backend
    if let Ok(secret) = std::env::var("WORKER_SECRET")
        && !secret.is_empty()
    {
        req = req.header("X-Worker-Secret", secret);
    }

    let res = req.send().await.map_err(|e| e.to_string())?;
    let status = res.status();
    if status.is_success() {
        Ok(())
    } else {
        let body = res.text().await.unwrap_or_default();
        Err(format!("Worker {status}: {body}"))
    }
}
