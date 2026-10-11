use crate::core::yt_dlp_output::{
    classify_failure, redact_url, version_age_days, FailureKind, UrlRedactor, STALE_AFTER_DAYS,
};
use crate::core::{validation, Config, YtDlpManager};
use anyhow::{anyhow, Context, Result};
use colored::Colorize;
use dialoguer::Input;
use std::collections::VecDeque;
use std::fs;
use std::io::{self, BufRead, BufReader, ErrorKind, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::thread;

/// Máximo de líneas que se guardan de la salida de yt-dlp para diagnosticar
const OUTPUT_TAIL: usize = 50;

/// Configuration for video download
struct DownloadConfig<'a> {
    yt_dlp_path: &'a Path,
    url: &'a str,
    download_dir: &'a Path,
    output: Option<&'a str>,
    quality: Option<&'a str>,
    format: Option<&'a str>,
    audio_only: bool,
    no_playlist: bool,
    playlist: bool,
    no_continue: bool,
    cookies_browser: Option<&'a str>,
    cookies_file: Option<&'a str>,
}

pub fn execute(matches: &clap::ArgMatches) -> Result<()> {
    // 1. Extraer argumentos
    let get = |k| matches.get_one::<String>(k).map(|s| s.as_str());

    let url = get("url");
    let update = matches.get_flag("update");
    let output = get("output");
    let quality = get("quality");
    let format = get("format");
    let audio_only = matches.get_flag("audio-only");
    let no_playlist = matches.get_flag("no-playlist");
    let playlist = matches.get_flag("playlist");
    let no_continue = matches.get_flag("no-continue");
    let clean_parts = matches.get_flag("clean-parts");
    let cookies_browser = get("cb");
    let cookies_file = get("cookies");

    // 2. Validar URL con validación mejorada
    if let Some(url) = url {
        validation::validate_url(url).with_context(|| format!("URL inválida: {}", url))?;
    }

    // 3. Actualizar yt-dlp si se pidió (solo, o antes de descargar)
    let mut manager = YtDlpManager::new()?;
    if update {
        manager.update()?;
    }

    // clap exige la URL salvo con --update
    let Some(url) = url else {
        return Ok(());
    };

    // 4. Asegurar yt-dlp instalado
    let yt_dlp_path = manager.ensure_yt_dlp()?;

    // 5. Determinar directorio de destino
    let download_dir = get_download_directory()?;

    // 6. Limpiar archivos .part si se solicitó
    if clean_parts {
        clean_part_files(&download_dir)?;
    }

    // 7. Validar output path si se proporcionó
    if let Some(output_name) = output {
        validation::validate_output_path(output_name)
            .with_context(|| format!("Output path inválido: {}", output_name))?;
    }

    // 8. Construir y ejecutar comando
    let config = DownloadConfig {
        yt_dlp_path: &yt_dlp_path,
        url,
        download_dir: &download_dir,
        output,
        quality,
        format,
        audio_only,
        no_playlist,
        playlist,
        no_continue,
        cookies_browser,
        cookies_file,
    };
    execute_download(&config)?;

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
        let msg = format!("{} archivos .part", count);
        println!("{} {}", "✓ Eliminados".green().bold(), msg);
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
            let msg = format!(
                "⚠️  El directorio de videos configurado no existe: {}",
                video_path
            );
            println!("{}", msg.yellow());
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
            let msg = format!("⚠️  El directorio '{}' no existe.", input);
            println!("{}", msg.yellow());

            // Preguntar si quiere crearlo
            if dialoguer::Confirm::new()
                .with_prompt("¿Deseas crearlo?")
                .default(true)
                .interact()?
            {
                fs::create_dir_all(&path).context("No se pudo crear el directorio")?;
                let success_msg = format!("✓ Directorio creado: {}", path.display());
                println!("{}", success_msg.green());
                return Ok(path);
            }

            println!("{}", "Por favor, ingresa una ruta válida.".yellow());
            println!();
            continue;
        }

        // Verificar que es un directorio
        if !path.is_dir() {
            println!();
            let msg = format!("⚠️  '{}' no es un directorio válido.", input);
            println!("{}", msg.red());
            println!();
            continue;
        }

        return Ok(path);
    }
}

/// Guarda la ruta de video en la configuración
fn save_video_path(config: &mut Config, path: &Path) -> Result<()> {
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
fn execute_download(config: &DownloadConfig) -> Result<()> {
    println!();
    println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".cyan());
    println!("{}", "  Descargando Video".cyan().bold());
    println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".cyan());
    println!();

    let mut cmd = Command::new(config.yt_dlp_path);

    // Configurar directorio de salida
    let output_template = if let Some(name) = config.output {
        config.download_dir.join(format!("{}.%(ext)s", name))
    } else {
        config.download_dir.join("%(title)s.%(ext)s")
    };

    cmd.arg("-o").arg(output_template);

    // Configurar continuación de descarga
    // Por defecto, yt-dlp continúa descargas interrumpidas (-c está implícito)
    if config.no_continue {
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
    if let Some(q) = config.quality {
        cmd.arg("-f").arg(format!(
            "bestvideo[height<={}]+bestaudio/best[height<={}]",
            q, q
        ));
    } else if config.audio_only {
        cmd.arg("-f").arg("bestaudio");
        cmd.arg("-x"); // Extract audio
    }

    // Configurar formato
    if let Some(fmt) = config.format {
        cmd.arg("--merge-output-format").arg(fmt);
    } else if !config.audio_only {
        cmd.arg("--merge-output-format").arg("mp4");
    }

    // Configurar playlist
    if config.no_playlist {
        cmd.arg("--no-playlist");
    } else if config.playlist {
        cmd.arg("--yes-playlist");
    }

    // Configurar cookies del navegador
    if let Some(browser) = config.cookies_browser {
        cmd.arg("--cookies-from-browser").arg(browser);
        println!("{} {}", "🍪 Usando cookies de:".cyan(), browser);
    }

    // Configurar archivo de cookies
    if let Some(file) = config.cookies_file {
        // Prevenir que yt-dlp intente leer cookies del navegador automáticamente
        cmd.arg("--no-cookies-from-browser");
        cmd.arg("--cookies").arg(file);
        println!("{} {}", "🍪 Usando archivo de cookies:".cyan(), file);
    }

    // Por pipe yt-dlp apaga los colores: los forzamos solo si msc escribe en
    // una terminal real, así la salida se ve igual que antes
    if io::stdout().is_terminal() {
        cmd.arg("--color").arg("always");
    }

    // Agregar URL
    cmd.arg(config.url);

    // Por pipe Python usa la codepage de Windows; forzamos UTF-8 para los títulos
    cmd.env("PYTHONIOENCODING", "utf-8");

    println!(
        "{} {}",
        "Ejecutando:".dimmed(),
        redacted_command_line(&cmd, config.url)
    );
    println!();

    // Ejecutar comando
    let redactor = UrlRedactor::new(config.url);
    let (status, stderr_tail) = run_redacted(&mut cmd, &redactor)?;

    println!();

    if status.success() {
        println!("{}", "✓ Descarga completada exitosamente".green().bold());
        return Ok(());
    }

    let kind = classify_failure(&stderr_tail);
    report_failure(kind, config.yt_dlp_path);

    let reason = match kind {
        FailureKind::Unknown => format!("{} ({})", kind.label(), status),
        _ => kind.label().to_string(),
    };
    Err(anyhow!(
        "La descarga de {} falló: {}",
        redact_url(config.url),
        reason
    ))
}

/// Como el `{:?}` de Command, pero con la URL reducida a su dominio
fn redacted_command_line(cmd: &Command, url: &str) -> String {
    std::iter::once(cmd.get_program())
        .chain(cmd.get_args())
        .map(|arg| {
            let arg = arg.to_string_lossy();
            if arg == url {
                format!("{:?}", redact_url(url))
            } else {
                format!("{:?}", arg)
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Corre yt-dlp reenviando su stdout/stderr a la terminal con la URL oculta.
/// Devuelve el exit status y las últimas líneas de stderr para diagnosticar.
fn run_redacted(cmd: &mut Command, redactor: &UrlRedactor) -> Result<(ExitStatus, Vec<String>)> {
    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("Error al ejecutar yt-dlp")?;

    let stdout = child.stdout.take().context("stdout de yt-dlp no disponible")?;
    let stderr = child.stderr.take().context("stderr de yt-dlp no disponible")?;

    // Ambos pipes se drenan en paralelo: si uno se llena, yt-dlp se bloquea
    let (status, stderr_tail) = thread::scope(|s| {
        let out = s.spawn(|| relay(stdout, io::stdout(), redactor));
        let err = s.spawn(|| relay(stderr, io::stderr(), redactor));
        let status = child.wait();
        // Un panic en el relay no debe tapar el resultado de la descarga
        let _ = out.join();
        (status, err.join().unwrap_or_default())
    });

    Ok((status.context("Error esperando a yt-dlp")?, stderr_tail))
}

/// Copia un stream de yt-dlp al nuestro ocultando la URL. Corta en '\n' y '\r'
/// para que la barra de progreso (que se redibuja con '\r') siga funcionando.
/// Devuelve las últimas líneas no vacías.
fn relay(src: impl Read, mut dst: impl Write, redactor: &UrlRedactor) -> Vec<String> {
    let mut reader = BufReader::new(src);
    let mut pending: Vec<u8> = Vec::new();
    let mut tail: VecDeque<String> = VecDeque::new();

    loop {
        let chunk = match reader.fill_buf() {
            Ok([]) => break,
            Ok(chunk) => chunk,
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(_) => break,
        };
        let read = chunk.len();
        pending.extend_from_slice(chunk);
        reader.consume(read);

        // Todo hasta el último separador son líneas completas
        if let Some(last) = pending.iter().rposition(|&b| b == b'\n' || b == b'\r') {
            let rest = pending.split_off(last + 1);
            emit(&pending, &mut dst, redactor, &mut tail);
            pending = rest;
        }

        // yt-dlp escribe el progreso como "\r[download]  45%..." sin salto final:
        // se muestra ya para no atrasar la barra. Esas líneas no llevan URL ni ID
        if pending.starts_with(b"[download]") && !pending.windows(3).any(|w| w == b"://") {
            emit(&pending, &mut dst, redactor, &mut tail);
            pending.clear();
        }

        let _ = dst.flush();
    }

    if !pending.is_empty() {
        emit(&pending, &mut dst, redactor, &mut tail);
        let _ = dst.flush();
    }

    tail.into()
}

fn emit(bytes: &[u8], dst: &mut impl Write, redactor: &UrlRedactor, tail: &mut VecDeque<String>) {
    let text = redactor.redact(&String::from_utf8_lossy(bytes));
    let _ = dst.write_all(text.as_bytes());

    for line in text
        .split(|c: char| c == '\n' || c == '\r')
        .map(str::trim)
        .filter(|l| !l.is_empty())
    {
        if tail.len() == OUTPUT_TAIL {
            tail.pop_front();
        }
        tail.push_back(line.to_string());
    }
}

/// Explica la causa probable del fallo y, si yt-dlp está viejo, cómo actualizarlo
fn report_failure(kind: FailureKind, yt_dlp_path: &Path) {
    if let Some(meaning) = kind.meaning() {
        println!("{} {}", format!("✗ {}:", kind.label()).red().bold(), meaning);
        println!("  {}", "Causas probables:".yellow());
        for (i, cause) in kind.causes().iter().enumerate() {
            println!("    {}. {}", i + 1, cause);
        }
        println!();
    }

    if !kind.update_may_help() {
        return;
    }

    let Some(version) = YtDlpManager::version_of(yt_dlp_path) else {
        return;
    };
    let today = chrono::Local::now().date_naive();
    if let Some(age) = version_age_days(&version, today).filter(|&d| d > STALE_AFTER_DAYS) {
        println!(
            "{} Tu yt-dlp ({}) tiene {} días. Actualizalo con:",
            "⚠️ ".yellow(),
            version.yellow(),
            age
        );
        println!("     {}", "msc vget --update".green().bold());
        println!(
            "   {}",
            "o sumá --update al mismo comando para actualizar y reintentar de una".dimmed()
        );
        println!();
    }
}
