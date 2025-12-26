use anyhow::Result;
use colored::Colorize;
use dialoguer::Confirm;

use crate::core::update::{detect_install_method, InstallMethod, UpdateManager};

pub fn execute(_matches: &clap::ArgMatches) -> Result<()> {
    // Banner de bienvenida
    println!();
    println!("{}", "━".repeat(50).cyan());
    println!("  {}", "MSC Update Utility".bold().cyan());
    println!("{}", "━".repeat(50).cyan());
    println!();

    // 1. Mostrar versión actual
    let current_version = UpdateManager::get_current_version();
    println!(
        "{} {}",
        "Current version:".dimmed(),
        current_version.yellow().bold()
    );

    // 2. Detectar método de instalación
    println!();
    println!("{}", "Checking installation method...".cyan());

    let install_method = detect_install_method()?;

    // 3. Si fue instalado por package manager, rechazar actualización
    match install_method {
        InstallMethod::Winget => {
            println!(
                "{}",
                "✗ Installed via Windows Package Manager (winget)".red()
            );
            println!();
            println!("{}", "To update MSC, please use:".yellow());
            println!("  {}", "winget upgrade Marco.MSC".green().bold());
            println!();
            println!(
                "{}",
                "This ensures proper integration with your system's package manager.".dimmed()
            );
            return Ok(());
        }
        InstallMethod::Chocolatey => {
            println!("{}", "✗ Installed via Chocolatey".red());
            println!();
            println!("{}", "To update MSC, please use:".yellow());
            println!("  {}", "choco upgrade msc".green().bold());
            println!();
            println!(
                "{}",
                "This ensures proper integration with your system's package manager.".dimmed()
            );
            return Ok(());
        }
        InstallMethod::Homebrew => {
            println!("{}", "✗ Installed via Homebrew".red());
            println!();
            println!("{}", "To update MSC, please use:".yellow());
            println!("  {}", "brew upgrade msc".green().bold());
            println!();
            println!(
                "{}",
                "This ensures proper integration with your system's package manager.".dimmed()
            );
            return Ok(());
        }
        InstallMethod::Cargo => {
            println!("{} {:?}", "✓ Installation method:".green(), install_method);
            println!();
            println!("{}", "Note: Installed via cargo.".dimmed());
            println!("{}", "Update will replace the binary directly.".dimmed());
        }
        InstallMethod::Manual => {
            println!("{} {:?}", "✓ Installation method:".green(), install_method);
        }
    }

    // 4. Buscar actualizaciones
    println!();
    println!("{}", "Checking for updates...".cyan());

    let manager = UpdateManager::new()?;

    let release = match manager.check_for_updates()? {
        Some(rel) => rel,
        None => {
            println!();
            println!(
                "{}",
                "✓ You're already running the latest version!"
                    .green()
                    .bold()
            );
            println!();
            return Ok(());
        }
    };

    let latest_version = release.version();
    println!(
        "{} {}",
        "✓ Latest version available:".green(),
        latest_version.yellow().bold()
    );

    // 5. Mostrar changelog si está disponible
    let changelog = release.changelog();
    if !changelog.is_empty() {
        println!();
        println!("{}:", "Release Notes".bold());
        println!("{}", "─".repeat(50).dimmed());

        // Mostrar las primeras 10 líneas del changelog
        let lines: Vec<&str> = changelog.lines().take(10).collect();
        for line in lines {
            println!("{}", line.dimmed());
        }

        if changelog.lines().count() > 10 {
            println!("{}", "... (see full changelog on GitHub)".dimmed());
        }

        println!("{}", "─".repeat(50).dimmed());
    }

    // 6. Calcular tamaño aproximado (basado en plataforma)
    #[cfg(windows)]
    let estimated_size = "~8.5 MB";
    #[cfg(not(windows))]
    let estimated_size = "~6.0 MB";

    println!();
    println!("{} {}", "Download size:".dimmed(), estimated_size);

    // 7. Confirmar con el usuario
    println!();
    let proceed = Confirm::new()
        .with_prompt("Proceed with update?")
        .default(true)
        .interact()?;

    if !proceed {
        println!();
        println!("{}", "Update cancelled.".yellow());
        return Ok(());
    }

    // 8. Ejecutar actualización
    println!();
    manager.perform_update(&release)?;

    // 9. Mensaje final
    println!();
    println!(
        "{}",
        "Please restart your terminal or run 'msc --version' to verify the update.".dimmed()
    );
    println!();

    Ok(())
}
