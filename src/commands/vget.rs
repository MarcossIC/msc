use crate::core::{validation, Config, YtDlpManager};
use anyhow::{anyhow, Context, Result};
use colored::Colorize;
use dialoguer::Input;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

pub fn execute(matches: &clap::ArgMatches) -> Result<()> {
    // 1. Extraer argumentos
    let url = matches
        .get_one::<String>("url")
        .context("URL es requerida")?;

    let output = matches.get_one::<String>("output");
    let quality = matches.get_one::<String>("quality");
    let format = matches.get_one::<String>("format");
    let audio_only = matches.get_flag("audio-only");
    let no_playlist = matches.get_flag("no-playlist");
    let playlist = matches.get_flag("playlist");
    let no_continue = matches.get_flag("no-continue");
    let clean_parts = matches.get_flag("clean-parts");

    // 2. Validar URL con validación mejorada
    validation::validate_url(url).with_context(|| format!("URL inválida: {}", url))?;

    // 3. Asegurar yt-dlp instalado
    let mut manager = YtDlpManager::new()?;
    let yt_dlp_path = manager.ensure_yt_dlp()?;

    // 4. Determinar directorio de destino
    let download_dir = get_download_directory()?;

    // 5. Limpiar archivos .part si se solicitó
    if clean_parts {
        clean_part_files(&download_dir)?;
    }

    // 6. Validar output path si se proporcionó
    if let Some(output_name) = output {
        validation::validate_output_path(output_name)
            .with_context(|| format!("Output path inválido: {}", output_name))?;
    }

    // 7. Construir y ejecutar comando
    execute_download(
        &yt_dlp_path,
        url,
        &download_dir,
        output,
        quality,
        format,
        audio_only,
        no_playlist,
        playlist,
        no_continue,
    )?;

    Ok(())
}

/// Limpia archivos .part huérfanos del directorio de descarga
fn clean_part_files(download_dir: &PathBuf) -> Result<()> {
    println!("{}", "Buscando archivos .part huérfanos...".yellow());

    let entries =
        fs::read_dir(download_dir).context("No se pudo leer el directorio de descarga")?;

    let mut count = 0;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        if let Some(extension) = path.extension() {
            if extension == "part" {
                println!("  {} {}", "Eliminando:".red(), path.display());
                fs::remove_file(&path)?;
                count += 1;
            }
        }
    }

    if count > 0 {
        println!(
            "{} {}",
            "✓ Eliminados".green().bold(),
            format!("{} archivos .part", count)
        );
    } else {
        println!("{}", "No se encontraron archivos .part".dimmed());
    }

    println!();
    Ok(())
}

/// Obtiene el directorio de descarga (interactivo si no está configurado)
fn get_download_directory() -> Result<PathBuf> {
    let mut config = Config::load()?;

    // Verificar si ya existe video_path configurado
    if let Some(video_path) = config.get_video_path() {
        let path = PathBuf::from(video_path);

        if !path.exists() {
            println!(
                "{}",
                format!(
                    "⚠️  El directorio de videos configurado no existe: {}",
                    video_path
                )
                .yellow()
            );
            println!();
            // Pedir nueva ruta interactivamente
            let new_path = prompt_for_video_path()?;
            save_video_path(&mut config, &new_path)?;
            return Ok(new_path);
        }

        println!("{} {}", "✓ Directorio de descarga:".green(), video_path);
        return Ok(path);
    }

    // No hay video_path configurado, pedir interactivamente
    println!();
    println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".yellow());
    println!("{}", "  Configuración Inicial".yellow().bold());
    println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".yellow());
    println!();
    println!(
        "{}",
        "No tienes un directorio de videos configurado.".yellow()
    );
    println!(
        "{}",
        "Ingresa la ruta donde deseas guardar los videos descargados:".white()
    );
    println!();

    let video_path = prompt_for_video_path()?;
    save_video_path(&mut config, &video_path)?;

    println!();
    println!("{}", "✓ Configuración guardada exitosamente".green().bold());
    println!();

    Ok(video_path)
}

/// Solicita al usuario la ruta del directorio de videos
fn prompt_for_video_path() -> Result<PathBuf> {
    loop {
        let input: String = Input::new()
            .with_prompt("Ruta del directorio de videos")
            .with_initial_text(
                dirs::video_dir()
                    .or_else(|| dirs::home_dir().map(|p| p.join("Videos")))
                    .and_then(|p| p.to_str().map(String::from))
                    .unwrap_or_default(),
            )
            .interact_text()?;

        let path = PathBuf::from(&input);

        // Verificar si el directorio existe
        if !path.exists() {
            println!();
            println!(
                "{}",
                format!("⚠️  El directorio '{}' no existe.", input).yellow()
            );

            // Preguntar si quiere crearlo
            if dialoguer::Confirm::new()
                .with_prompt("¿Deseas crearlo?")
                .default(true)
                .interact()?
            {
                fs::create_dir_all(&path).context("No se pudo crear el directorio")?;
                println!(
                    "{}",
                    format!("✓ Directorio creado: {}", path.display()).green()
                );
                return Ok(path);
            }

            println!("{}", "Por favor, ingresa una ruta válida.".yellow());
            println!();
            continue;
        }

        // Verificar que es un directorio
        if !path.is_dir() {
            println!();
            println!(
                "{}",
                format!("⚠️  '{}' no es un directorio válido.", input).red()
            );
            println!();
            continue;
        }

        return Ok(path);
    }
}

/// Guarda la ruta de video en la configuración
fn save_video_path(config: &mut Config, path: &PathBuf) -> Result<()> {
    let canonical_path = path
        .canonicalize()
        .map_err(|e| anyhow::anyhow!("Failed to resolve path: {}", e))?
        .to_string_lossy()
        .to_string();

    config.set_video_path(canonical_path.clone());
    config.save()?;

    Ok(())
}

/// Ejecuta la descarga del video
fn execute_download(
    yt_dlp_path: &PathBuf,
    url: &str,
    download_dir: &PathBuf,
    output: Option<&String>,
    quality: Option<&String>,
    format: Option<&String>,
    audio_only: bool,
    no_playlist: bool,
    playlist: bool,
    no_continue: bool,
) -> Result<()> {
    println!();
    println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".cyan());
    println!("{}", "  Descargando Video".cyan().bold());
    println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".cyan());
    println!();

    let mut cmd = Command::new(yt_dlp_path);

    // Configurar directorio de salida
    let output_template = if let Some(name) = output {
        download_dir.join(format!("{}.%(ext)s", name))
    } else {
        download_dir.join("%(title)s.%(ext)s")
    };

    cmd.arg("-o").arg(output_template);

    // Configurar continuación de descarga
    // Por defecto, yt-dlp continúa descargas interrumpidas (-c está implícito)
    if no_continue {
        // Forzar descarga desde cero (--no-continue)
        cmd.arg("--no-continue");
        println!(
            "{}",
            "⚠️  Modo: Descarga desde cero (ignorando archivos .part)".yellow()
        );
    } else {
        // Modo por defecto: continuar si existe .part
        cmd.arg("--continue");
        println!(
            "{}",
            "✓ Modo: Resumir descargas interrumpidas (si existen .part)".green()
        );
    }

    println!();

    // Configurar calidad
    if let Some(q) = quality {
        cmd.arg("-f").arg(format!(
            "bestvideo[height<={}]+bestaudio/best[height<={}]",
            q, q
        ));
    } else if audio_only {
        cmd.arg("-f").arg("bestaudio");
        cmd.arg("-x"); // Extract audio
    }

    // Configurar formato
    if let Some(fmt) = format {
        cmd.arg("--merge-output-format").arg(fmt);
    } else if !audio_only {
        cmd.arg("--merge-output-format").arg("mp4");
    }

    // Configurar playlist
    if no_playlist {
        cmd.arg("--no-playlist");
    } else if playlist {
        cmd.arg("--yes-playlist");
    }

    // Agregar URL
    cmd.arg(url);

    println!("{} {:?}", "Ejecutando:".dimmed(), cmd);
    println!();

    // Ejecutar comando
    let status = cmd.status().context("Error al ejecutar yt-dlp")?;

    println!();

    if status.success() {
        println!("{}", "✓ Descarga completada exitosamente".green().bold());
    } else {
        return Err(anyhow!("La descarga falló con código: {}", status));
    }

    Ok(())
}
