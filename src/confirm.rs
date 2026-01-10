use anyhow::{bail, Result};
use std::io::{self, Write};

pub fn prompt_confirm(message: &str) -> Result<bool> {
    print!("{message} [y/N]: ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    let input = input.trim().to_lowercase();
    Ok(matches!(input.as_str(), "y" | "yes"))
}

pub fn prompt_string(message: &str, default: &str) -> Result<String> {
    if default.is_empty() {
        print!("{message}: ");
    } else {
        print!("{message} [{default}]: ");
    }
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    let input = input.trim();
    if input.is_empty() {
        Ok(default.to_string())
    } else {
        Ok(input.to_string())
    }
}

pub fn prompt_select(message: &str, options: &[&str], default: usize) -> Result<usize> {
    println!("{message}");
    for (i, option) in options.iter().enumerate() {
        let marker = if i == default { " (default)" } else { "" };
        println!("  {}. {}{}", i + 1, option, marker);
    }
    print!("Choice [{}]: ", default + 1);
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    let input = input.trim();
    if input.is_empty() {
        return Ok(default);
    }

    match input.parse::<usize>() {
        Ok(n) if n >= 1 && n <= options.len() => Ok(n - 1),
        _ => bail!("Invalid selection"),
    }
}

pub fn prompt_multi(message: &str, options: &[&str], defaults: &[usize]) -> Result<Vec<usize>> {
    println!("{message}");
    for (i, option) in options.iter().enumerate() {
        let marker = if defaults.contains(&i) { " *" } else { "" };
        println!("  {}. {}{}", i + 1, option, marker);
    }
    println!("Enter numbers separated by spaces, or press Enter for defaults (*)");
    print!("Choices: ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    let input = input.trim();
    if input.is_empty() {
        return Ok(defaults.to_vec());
    }

    let mut selected = Vec::new();
    for part in input.split_whitespace() {
        if let Ok(n) = part.parse::<usize>() {
            if n >= 1 && n <= options.len() {
                selected.push(n - 1);
            }
        }
    }
    Ok(selected)
}
