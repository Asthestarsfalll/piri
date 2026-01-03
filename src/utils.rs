pub fn send_notification(summary: &str, body: &str) {
    let _ = std::process::Command::new("notify-send")
        .arg("-a")
        .arg("piri")
        .arg("-i")
        .arg("dialog-error")
        .arg(summary)
        .arg(body)
        .spawn();
}
