use anyhow::Result;
use std::io::{self, Write};

pub fn prompt_confirm(message: &str) -> Result<bool> {
    print!("{message} [y/N]: ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    let input = input.trim().to_lowercase();
    Ok(matches!(input.as_str(), "y" | "yes"))
}
