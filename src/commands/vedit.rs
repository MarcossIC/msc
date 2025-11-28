use crate::core::FFmpegManager;
use anyhow::{anyhow, Context, Result};
use colored::Colorize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Execute the vedit command to edit videos
pub fn execute(matches: &clap::ArgMatches) -> Result<()> {
    match matches.subcommand() {
        Some(("comp", sub_matches)) | Some(("compress", sub_matches)) => {
            execute_compress(sub_matches)
        }
        _ => Err(anyhow!(
            "Subcomando no reconocido. Usa 'msc vedit comp <quality> <video>'"
        )),
    }
}

/// Execute video compression
fn execute_compress(matches: &clap::ArgMatches) -> Result<()> {
    // 1. Extraer argumentos
    let quality = matches
        .get_one::<String>("quality")
        .context("Nivel de calidad es requerido")?;

    let video_path = matches
        .get_one::<String>("video")
        .context("Archivo de video es requerido")?;

    // 2. Validar que el archivo existe
    let input_path = PathBuf::from(video_path);
    if !input_path.exists() {
        return Err(anyhow!("El archivo '{}' no existe", video_path));
    }

    // 3. Validar que es un archivo de video
    validate_video_file(&input_path)?;

    // 4. Asegurar que FFmpeg está disponible
    let mut manager = FFmpegManager::new()?;
    let ffmpeg_path = manager.ensure_ffmpeg()?;

    // 5. Construir nombre de salida
    let output_path = build_output_path(&input_path)?;

    // 6. Ejecutar compresión
    execute_compression(&ffmpeg_path, &input_path, &output_path, quality)?;

    Ok(())
}

/// Valida que el archivo tenga una extensión de video válida
fn validate_video_file(path: &Path) -> Result<()> {
    let valid_extensions = ["mp4", "avi", "mkv", "mov", "wmv", "flv", "webm", "m4v"];

    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .ok_or_else(|| anyhow!("El archivo no tiene extensión"))?
        .to_lowercase();

    if !valid_extensions.contains(&extension.as_str()) {
        return Err(anyhow!(
            "Extensión '{}' no soportada. Extensiones válidas: {}",
            extension,
            valid_extensions.join(", ")
        ));
    }

    Ok(())
}

/// Construye el path de salida agregando "_compress" antes de la extensión
fn build_output_path(input_path: &Path) -> Result<PathBuf> {
    let parent = input_path
        .parent()
        .ok_or_else(|| anyhow!("No se pudo obtener el directorio padre"))?;

    let stem = input_path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| anyhow!("No se pudo obtener el nombre del archivo"))?;

    let extension = input_path
        .extension()
        .and_then(|e| e.to_str())
        .ok_or_else(|| anyhow!("No se pudo obtener la extensión"))?;

    let output_name = format!("{}_compress.{}", stem, extension);
    let output_path = parent.join(output_name);

    // Verificar si el archivo de salida ya existe
    if output_path.exists() {
        println!(
            "{}",
            format!(
                "⚠ El archivo '{}' ya existe y será sobrescrito",
                output_path.display()
            )
            .yellow()
        );
    }

    Ok(output_path)
}

/// Ejecuta la compresión con FFmpeg
fn execute_compression(
    ffmpeg_path: &PathBuf,
    input_path: &PathBuf,
    output_path: &PathBuf,
    quality: &str,
) -> Result<()> {
    // Obtener parámetros según calidad
    let (crf, preset, audio_bitrate) = match quality {
        "low" => ("28", "fast", "96k"),
        "medium" => ("23", "medium", "128k"),
        "high" => ("18", "slow", "192k"),
        _ => {
            return Err(anyhow!(
                "Calidad inválida: '{}'. Usa: low, medium o high",
                quality
            ))
        }
    };

    println!();
    println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".cyan());
    println!("{}", "  Compresión de Video".cyan().bold());
    println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".cyan());
    println!();
    println!("{} {}", "📹 Entrada:".cyan(), input_path.display());
    println!("{} {}", "💾 Salida:".cyan(), output_path.display());
    println!("{} {}", "🎚️  Calidad:".cyan(), quality);
    println!(
        "{} CRF={}, Preset={}, Audio={}",
        "⚙️  Parámetros:".cyan(),
        crf,
        preset,
        audio_bitrate
    );
    println!();

    // Construir comando
    let mut cmd = Command::new(ffmpeg_path);
    cmd.arg("-i")
        .arg(input_path)
        .arg("-c:v")
        .arg("libx264")
        .arg("-crf")
        .arg(crf)
        .arg("-preset")
        .arg(preset)
        .arg("-c:a")
        .arg("aac")
        .arg("-b:a")
        .arg(audio_bitrate)
        .arg("-y") // Sobrescribir sin preguntar
        .arg(output_path);

    println!("{} {:?}", "Ejecutando:".dimmed(), cmd);
    println!();

    // Ejecutar comando
    let status = cmd.status().context("Error al ejecutar ffmpeg")?;

    println!();

    if status.success() {
        // Mostrar tamaños para comparación
        let original_size = fs::metadata(input_path)?.len();
        let compressed_size = fs::metadata(output_path)?.len();
        let reduction = 100.0 - (compressed_size as f64 / original_size as f64 * 100.0);

        println!("{}", "✓ Compresión completada exitosamente".green().bold());
        println!();
        println!(
            "{} {} MB",
            "📦 Tamaño original:".cyan(),
            original_size / 1_048_576
        );
        println!(
            "{} {} MB",
            "📦 Tamaño comprimido:".cyan(),
            compressed_size / 1_048_576
        );
        println!("{} {:.1}%", "📉 Reducción:".cyan(), reduction);
        println!();
        println!(
            "{} {}",
            "💾 Archivo guardado:".green().bold(),
            output_path.display()
        );
    } else {
        return Err(anyhow!(
            "La compresión falló con código de salida: {}",
            status
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_video_file() {
        assert!(validate_video_file(&PathBuf::from("video.mp4")).is_ok());
        assert!(validate_video_file(&PathBuf::from("video.avi")).is_ok());
        assert!(validate_video_file(&PathBuf::from("video.mkv")).is_ok());
        assert!(validate_video_file(&PathBuf::from("video.txt")).is_err());
        assert!(validate_video_file(&PathBuf::from("no_extension")).is_err());
    }

    #[test]
    fn test_build_output_path() {
        let input = PathBuf::from("C:\\videos\\test.mp4");
        let output = build_output_path(&input).unwrap();
        assert_eq!(output, PathBuf::from("C:\\videos\\test_compress.mp4"));

        let input2 = PathBuf::from("/home/user/video.avi");
        let output2 = build_output_path(&input2).unwrap();
        assert_eq!(output2, PathBuf::from("/home/user/video_compress.avi"));
    }
}
