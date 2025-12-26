use anyhow::Result;
use clap::{ArgMatches, Command};
use clap_complete::{generate, Shell};
use std::io;

/// Generate shell completions for the specified shell
pub fn execute(matches: &ArgMatches, cli: &mut Command) -> Result<()> {
    if let Some(shell_str) = matches.get_one::<String>("shell") {
        let shell = match shell_str.to_lowercase().as_str() {
            "bash" => Shell::Bash,
            "zsh" => Shell::Zsh,
            "fish" => Shell::Fish,
            "powershell" => Shell::PowerShell,
            "elvish" => Shell::Elvish,
            _ => {
                eprintln!("Unsupported shell: {}", shell_str);
                eprintln!("Supported shells: bash, zsh, fish, powershell, elvish");
                std::process::exit(1);
            }
        };

        generate(shell, cli, "msc", &mut io::stdout());
        Ok(())
    } else {
        eprintln!("Error: shell argument is required");
        eprintln!("Usage: msc completions <SHELL>");
        eprintln!("Supported shells: bash, zsh, fish, powershell, elvish");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_variants() {
        // Just verify we can reference the shells
        let _bash = Shell::Bash;
        let _zsh = Shell::Zsh;
        let _fish = Shell::Fish;
        let _powershell = Shell::PowerShell;
        let _elvish = Shell::Elvish;
    }
}
