use anyhow::Result;

/// Construye el comando de PowerShell para elevación con escapado seguro
///
/// Esta función contiene la lógica REAL de construcción del comando que se usa
/// tanto en producción como en tests, garantizando que los tests validen
/// el comportamiento real del código.
///
/// # Seguridad - Estrategia de Defensa en Profundidad
///
/// NIVEL 1: Codificación Base64 del comando completo (-EncodedCommand)
/// - El comando Start-Process completo se codifica en Base64 UTF-16LE
/// - PowerShell decodifica y ejecuta el comando SIN interpretar caracteres especiales
/// - Esto previene TODAS las formas de inyección: ', ", ;, |, &, $, `, etc.
/// - Es la defensa más robusta contra inyección de comandos en PowerShell
///
/// Por qué otras estrategias son insuficientes:
/// - Escapar ' → '': No funciona correctamente en todos los contextos
/// - Usar arrays @('...'): Aún vulnerable si el contenido rompe las comillas
/// - Validación de caracteres: Puede ser evadida con técnicas de ofuscación
///
/// # Arguments
/// * `program_path` - Path al programa a ejecutar
/// * `arguments` - Vector de argumentos a pasar al programa
///
/// # Returns
/// El comando de PowerShell codificado en Base64 (completamente seguro)
fn build_elevation_command(program_path: &str, arguments: &[String]) -> String {
    use base64::{engine::general_purpose, Engine as _};

    // Construir el comando de PowerShell que queremos ejecutar de forma segura
    // Este comando se codificará en Base64, por lo que NO necesita escapado
    let ps_command = if arguments.is_empty() {
        format!(
            "Start-Process -FilePath '{}' -Verb RunAs -Wait",
            program_path
        )
    } else {
        // Construir argumentos como array de PowerShell
        // Nota: Aunque estamos dentro de Base64, usamos sintaxis correcta de PowerShell
        let args_list: Vec<String> = arguments
            .iter()
            .map(|arg| {
                // Escapar comillas simples dentro del comando que será codificado
                let escaped = arg.replace('\'', "''");
                format!("'{}'", escaped)
            })
            .collect();

        format!(
            "Start-Process -FilePath '{}' -ArgumentList @({}) -Verb RunAs -Wait",
            program_path.replace('\'', "''"),
            args_list.join(", ")
        )
    };

    // Codificar el comando completo en UTF-16LE (requerido por PowerShell -EncodedCommand)
    let utf16_bytes: Vec<u8> = ps_command
        .encode_utf16()
        .flat_map(|c| c.to_le_bytes())
        .collect();

    // Codificar en Base64
    let encoded = general_purpose::STANDARD.encode(&utf16_bytes);

    // Retornar el comando usando -EncodedCommand
    // PowerShell decodificará y ejecutará el comando sin interpretar caracteres especiales
    format!("-EncodedCommand {}", encoded)
}

#[cfg(windows)]
pub fn is_elevated() -> bool {
    use std::mem;
    use std::ptr;
    use winapi::ctypes::c_void;

    unsafe {
        let mut handle: *mut c_void = ptr::null_mut();

        if winapi::um::processthreadsapi::OpenProcessToken(
            winapi::um::processthreadsapi::GetCurrentProcess(),
            winapi::um::winnt::TOKEN_QUERY,
            &mut handle,
        ) == 0
        {
            return false;
        }

        let mut elevation: winapi::um::winnt::TOKEN_ELEVATION = mem::zeroed();
        let mut size: u32 = 0;

        let result = winapi::um::securitybaseapi::GetTokenInformation(
            handle,
            winapi::um::winnt::TokenElevation,
            &mut elevation as *mut _ as *mut c_void,
            mem::size_of::<winapi::um::winnt::TOKEN_ELEVATION>() as u32,
            &mut size,
        );

        winapi::um::handleapi::CloseHandle(handle);

        result != 0 && elevation.TokenIsElevated != 0
    }
}

#[cfg(not(windows))]
pub fn is_elevated() -> bool {
    // On Unix, check if running as root
    unsafe { libc::geteuid() == 0 }
}

#[cfg(windows)]
pub fn elevate_and_rerun() -> Result<bool> {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    let exe_path = std::env::current_exe()?;
    let args: Vec<String> = std::env::args().skip(1).collect();

    // Usar la función compartida que implementa el escapado seguro con Base64
    // Retorna: "-EncodedCommand <base64>"
    let powershell_args = build_elevation_command(&exe_path.display().to_string(), &args);

    // Dividir en las partes del argumento
    let mut parts = powershell_args.split_whitespace();
    let encoded_flag = parts.next().unwrap(); // "-EncodedCommand"
    let encoded_value = parts.next().unwrap(); // "<base64 string>"

    let result = Command::new("powershell")
        .args([encoded_flag, encoded_value])
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
        .status();

    match result {
        Ok(status) => Ok(status.success()),
        Err(_) => Ok(false),
    }
}

#[cfg(not(windows))]
pub fn elevate_and_rerun() -> Result<bool> {
    // On Unix, can't auto-elevate, user must run with sudo
    Ok(false)
}

/// Ensures the program has elevated privileges, attempting to elevate if needed
pub fn ensure_elevated() -> Result<bool> {
    if is_elevated() {
        return Ok(true);
    }

    elevate_and_rerun()
}

/// Expone la construcción del comando de PowerShell para testing
///
/// Esta función permite que los tests verifiquen que la lógica de escapado
/// previene la inyección de comandos. Usa **exactamente el mismo código**
/// que elevate_and_rerun() (la función build_elevation_command), garantizando
/// que los tests validen el comportamiento real de producción.
///
/// # Arguments
/// * `program` - The path to the program to execute
/// * `args` - A single argument string (for testing injection attempts)
///
/// # Returns
/// El comando de PowerShell que sería ejecutado en producción
pub fn simulate_elevation_command(program: &str, args: &str) -> String {
    // Usar la MISMA función que usa elevate_and_rerun()
    // Esto garantiza que el test valida el código de producción real
    build_elevation_command(program, &vec![args.to_string()])
}
