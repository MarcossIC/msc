Estrategia de Distribución y Actualización para CLI en Rust
Este documento detalla cómo empaquetar un binario de Rust para distribución profesional, asegurando que se agregue al PATH del sistema (especialmente en Windows) e implementando un mecanismo de self-update.

1. Empaquetado e Instalación (Resolviendo el PATH)
Para que el usuario final pueda ejecutar tu herramienta desde cualquier terminal, el instalador debe modificar las variables de entorno del sistema. En lugar de hacerlo manualmente en tu código (lo cual es riesgoso y complejo), se recomienda delegar esto a un instalador.

Opción A: La Solución "Estándar de Oro" (Recomendada)
Herramienta: cargo-dist

Es la herramienta más moderna y automatizada del ecosistema Rust (creada por el equipo de Axo).

¿Qué hace?: Se integra con CI/CD (GitHub Actions). Cuando creas un "Release" en GitHub, automáticamente compila tu binario para Windows, Linux y Mac.

Instalación en Windows: Genera un instalador MSI o un script de PowerShell.

Gestión del PATH: El instalador MSI generado por cargo-dist tiene la opción nativa para agregar tu binario al PATH del usuario automáticamente.

Ventaja: Configuración mínima (cargo dist init) y resultado profesional.

Opción B: Control Total del Instalador (Windows Only)
Herramienta: cargo-wix (basado en WiX Toolset)

Si prefieres generar un instalador .msi manualmente sin depender de pipelines en la nube.

¿Qué hace?: Crea un instalador de Windows Installer (MSI) directamente desde tu proyecto cargo.

Gestión del PATH: Debes configurar el archivo wix/main.wxs (XML) para incluir la modificación del entorno.

```xml
<Environment Id="PATH" Name="PATH" Value="[INSTALLFOLDER]" Permanent="no" Part="last" Action="set" System="yes" />
```

Opción C: Instalador Clásico .exe
Herramienta: Inno Setup

Es un estándar en Windows para crear instaladores .exe (no MSI). Es muy amigable para el usuario final.

Workflow: Compilas tu binario Rust (cargo build --release) y usas un script .iss de Inno Setup para empaquetarlo.

Gestión del PATH: Inno Setup tiene directivas simples para esto:

```ini
[Setup]
ChangesEnvironment=yes

[Registry]
Root: HKLM; Subkey: "SYSTEM\CurrentControlSet\Control\Session Manager\Environment"; \
    ValueType: expandsz; ValueName: "Path"; ValueData: "{olddata};{app}"; \
    Check: NeedsAddPath('{app}')
```

2. Implementando el comando update (Self-Update)
Actualizar un ejecutable en Windows mientras está corriendo es difícil porque el sistema operativo bloquea el archivo. Sin embargo, existen librerías en Rust que manejan la técnica de "renombrar y reemplazar" necesaria para lograrlo.

Librería Recomendada: self_update
Es la crate más utilizada para este propósito. Se conecta a tus GitHub Releases, verifica la versión y descarga el nuevo binario.

Características:

Soporta GitHub y GitLab.

Verifica versiones (SemVer).

Maneja el reemplazo de binarios en Windows (renombra el ejecutable actual a .tmp para permitir la descarga del nuevo, y luego elimina el viejo).

Soporta verificación de firmas (opcional pero recomendado).

Implementación en Código
Agrega esto a tu Cargo.toml:

```ini
[dependencies]
self_update = "0.41" # Verifica la última versión
semver = "1.0"
```

Ejemplo de función de actualización:

```ini
use std::error::Error;

fn update_cli() -> Result<(), Box<dyn Error>> {
    let status = self_update::backends::github::Update::configure()
        .repo_owner("tu_usuario")
        .repo_name("nombre_de_tu_repo")
        .bin_name("tu_cli_name") // Nombre del ejecutable
        .show_download_progress(true)
        .current_version(env!("CARGO_PKG_VERSION")) // Versión actual del Cargo.toml
        .build()?
        .update()?; // ¡Esto hace toda la magia!

    println!("Actualización exitosa: versión {} instalada.", status.version());
    Ok(())
}
```

3. Flujo de Trabajo Recomendado (Resumen)
Para obtener un resultado profesional con el menor esfuerzo de mantenimiento, esta es la arquitectura sugerida:

Hospedaje: Código fuente en GitHub.

Infraestructura de Release: Usa cargo-dist.

Ejecuta cargo dist init.

Esto creará un workflow de GitHub Actions.

Cada vez que pongas un tag (ej. v1.0.0), cargo-dist compilará los binarios y creará un instalador MSI para Windows.

Instalación del Usuario:

El usuario descarga el .msi o corre el script de instalación.

El instalador coloca el binario en Archivos de Programa y actualiza el PATH.

Actualización:

Dentro de tu código Rust, usas la crate self_update.

Cuando el usuario ejecuta tu-cli update, la herramienta consulta GitHub, descarga el nuevo binario generado por cargo-dist (que está en los Releases) y se reemplaza a sí misma.

Nota sobre permisos en Windows
Si instalas el CLI en una carpeta de sistema (como Program Files), el comando update requerirá que el usuario ejecute la terminal como Administrador.

Alternativa sin Admin: Configurar el instalador para que instale en %LOCALAPPDATA%\TuApp. Esto permite que el usuario actualice la herramienta sin necesidad de permisos de administrador y simplifica la gestión del PATH a nivel de usuario (User Path), no de sistema.