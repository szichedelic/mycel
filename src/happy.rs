use anyhow::{bail, Result};

use crate::config::ResolvedBackend;

pub fn apply_happy_wrapper(backend: &mut ResolvedBackend) -> Result<()> {
    if !is_happy_available() {
        bail!(
            "Happy CLI not found. Install with: npm install -g happy-coder\n\
             Learn more: https://github.com/slopus/happy-cli"
        );
    }

    let original_command = std::mem::replace(&mut backend.command, "happy".to_string());
    let original_args = std::mem::take(&mut backend.args);

    backend.args = vec![original_command];
    backend.args.extend(original_args);

    Ok(())
}

pub fn is_happy_available() -> bool {
    std::process::Command::new("which")
        .arg("happy")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
