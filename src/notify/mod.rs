use anyhow::Result;

#[cfg(target_os = "macos")]
use anyhow::Context;
#[cfg(target_os = "macos")]
use std::process::Command;

#[cfg(target_os = "macos")]
pub fn send_notification(title: &str, body: &str) -> Result<()> {
    let title = escape_applescript(title);
    let body = escape_applescript(body);
    let script = format!("display notification \"{body}\" with title \"{title}\"");
    let status = Command::new("osascript")
        .args(["-e", &script])
        .status()
        .context("Failed to run osascript for notification")?;

    if !status.success() {
        anyhow::bail!("osascript notification failed");
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn escape_applescript(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(not(target_os = "macos"))]
pub fn send_notification(_title: &str, _body: &str) -> Result<()> {
    Ok(())
}
