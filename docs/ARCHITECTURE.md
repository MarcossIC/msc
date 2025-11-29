# MSC - Arquitectura del Proyecto

## Tabla de Contenidos

1. [Visión General](#visión-general)
2. [Principios Arquitectónicos](#principios-arquitectónicos)
3. [Estructura de Directorios](#estructura-de-directorios)
4. [Capas de la Aplicación](#capas-de-la-aplicación)
5. [Convenciones de Nomenclatura](#convenciones-de-nomenclatura)
6. [Código Autoexplicativo y Comentarios](#código-autoexplicativo-y-comentarios)
7. [Gestión de Módulos](#gestión-de-módulos)
8. [Manejo de Errores](#manejo-de-errores)
9. [Testing](#testing)
10. [Configuración](#configuración)
11. [Dependencias](#dependencias)
12. [Guías de Desarrollo](#guías-de-desarrollo)
13. [Patrones y Buenas Prácticas](#patrones-y-buenas-prácticas)

---

## Visión General

**MSC** es una herramienta CLI modular construida en Rust que sigue una arquitectura en capas con separación clara de responsabilidades. El proyecto está diseñado para ser:

- **Modular**: Cada componente tiene una responsabilidad específica
- **Extensible**: Fácil agregar nuevos comandos y funcionalidades
- **Mantenible**: Código organizado y bien documentado
- **Multiplataforma**: Soporte para Windows, Linux y macOS
- **Testeable**: Arquitectura que facilita las pruebas unitarias e integración

### Tecnologías Principales

- **Lenguaje**: Rust (Edition 2021)
- **CLI Framework**: Clap v4.4
- **Manejo de Errores**: thiserror + anyhow
- **Serialización**: serde + bincode
- **Git Integration**: git2
- **Logging**: log + env_logger

---

## Principios Arquitectónicos

### 1. Separation of Concerns (SoC)

Cada módulo tiene una responsabilidad única y bien definida:

- **Commands**: Únicamente parseo de argumentos y orquestación
- **Core**: Lógica de negocio pura, sin dependencias de UI o CLI
- **UI**: Presentación y formateo de salida
- **Platform**: Código específico de sistema operativo
- **Git**: Integración con repositorios Git
- **Utils**: Utilidades compartidas

### 2. Dependency Inversion

- Los módulos de alto nivel (commands) no dependen de módulos de bajo nivel (platform)
- Ambos dependen de abstracciones definidas en `core`
- Se usan traits para definir contratos e interfaces

### 3. Don't Repeat Yourself (DRY)

- Código común extraído a módulos reutilizables
- Re-exportación de tipos comunes desde `mod.rs`
- Utilidades compartidas en el módulo `utils`

### 4. Explicit Error Handling

- Uso consistente de `Result<T, E>` para operaciones fallibles
- Errores personalizados con contexto significativo
- No usar `unwrap()` o `expect()` en código de producción (solo en tests)

### 5. Composición sobre Herencia

- Rust no tiene herencia clásica
- Usar composición y traits para compartir comportamiento
- Preferir structs pequeños y componibles

---

## Estructura de Directorios

```
msc/
├── src/
│   ├── main.rs              # Punto de entrada del binario
│   ├── lib.rs               # Punto de entrada de la biblioteca
│   ├── error.rs             # Tipos de error personalizados
│   │
│   ├── commands/            # CAPA: Handlers de comandos CLI
│   │   ├── mod.rs           # Re-exportaciones públicas
│   │   ├── hello.rs         # Comando de ejemplo
│   │   ├── version.rs       # Información de versión
│   │   ├── list.rs          # Listado de directorios
│   │   ├── config.rs        # Gestión de configuración
│   │   ├── workspace.rs     # Gestión de workspaces
│   │   └── clean_temp.rs    # Limpieza de temporales
│   │
│   ├── core/                # CAPA: Lógica de negocio
│   │   ├── mod.rs           # Re-exportaciones públicas
│   │   ├── config.rs        # Modelo de configuración
│   │   ├── workspace.rs     # Gestión de workspaces
│   │   ├── file_scanner.rs  # Escaneo de archivos
│   │   └── cleaner.rs       # Lógica de limpieza
│   │
│   ├── git/                 # CAPA: Integración Git
│   │   ├── mod.rs           # Re-exportaciones públicas
│   │   ├── status.rs        # Estado de Git
│   │   ├── ignore.rs        # Manejo de .gitignore
│   │   └── colors.rs        # Colores por estado Git
│   │
│   ├── platform/            # CAPA: Específico de OS
│   │   ├── mod.rs           # Re-exportaciones públicas
│   │   ├── fs.rs            # Operaciones de sistema de archivos
│   │   ├── elevation.rs     # Gestión de permisos/elevación
│   │   └── temp_dirs.rs     # Directorios temporales por OS
│   │
│   ├── ui/                  # CAPA: Interfaz de usuario
│   │   ├── mod.rs           # Re-exportaciones públicas
│   │   ├── formatters.rs    # Formateo de datos
│   │   ├── progress.rs      # Barras de progreso
│   │   └── prompts.rs       # Prompts interactivos
│   │
│   └── utils/               # CAPA: Utilidades
│       ├── mod.rs           # Re-exportaciones públicas
│       └── icons.rs         # Iconos para tipos de archivo
│
├── tests/                   # Tests de integración
│   ├── integration.rs       # Módulo raíz de tests
│   ├── integration/
│   │   ├── commands_test.rs
│   │   └── config_test.rs
│   └── fixtures/            # Datos de prueba
│       └── test_workspace/
│
├── Cargo.toml               # Configuración del proyecto
├── Cargo.lock               # Lock de dependencias
└── README.md                # Documentación de usuario
```

### Reglas de Organización de Archivos

1. **Un módulo por archivo**: Cada archivo representa un módulo o submódulo
2. **`mod.rs`**: Archivo especial que define el módulo público del directorio
3. **Límite de líneas**: Archivos no deben exceder ~600 líneas (considerar refactorizar)
4. **Cohesión**: Código relacionado debe estar en el mismo módulo
5. **Acoplamiento**: Minimizar dependencias entre módulos

---

## Capas de la Aplicación

### 1. Commands Layer (`src/commands/`)

**Responsabilidad**: Interfaz entre CLI y lógica de negocio

#### ¿Qué va aquí?

- Funciones `execute()` que reciben `ArgMatches` de clap
- Validación de argumentos CLI
- Llamadas orquestadas a la capa Core
- Impresión de mensajes al usuario (usando UI Layer)
- Conversión de errores de negocio a mensajes amigables

#### ¿Qué NO va aquí?

- Lógica de negocio compleja
- Acceso directo al sistema de archivos
- Operaciones de base de datos o configuración
- Cálculos o transformaciones de datos

#### Ejemplo de estructura:

```rust
// src/commands/example.rs
use anyhow::Result;
use clap::ArgMatches;
use crate::core::SomeService;
use crate::ui::format_result;

pub fn execute(matches: &ArgMatches) -> Result<()> {
    // 1. Extraer argumentos
    let input = matches.get_one::<String>("input")
        .ok_or_else(|| anyhow::anyhow!("Input required"))?;
    
    // 2. Llamar a Core layer
    let service = SomeService::new();
    let result = service.process(input)?;
    
    // 3. Formatear y mostrar resultado
    let formatted = format_result(&result);
    println!("{}", formatted);
    
    Ok(())
}
```

### 2. Core Layer (`src/core/`)

**Responsabilidad**: Lógica de negocio pura y modelos de dominio

#### ¿Qué va aquí?

- Structs y enums del dominio
- Lógica de negocio compleja
- Reglas de validación
- Servicios y managers
- Operaciones de configuración
- Transformaciones de datos

#### ¿Qué NO va aquí?

- Código de presentación (println!, colored, etc.)
- Dependencias de clap o CLI
- Llamadas directas a winapi o libc
- Lógica específica de UI

#### Características:

- **Testeable**: Sin dependencias de I/O o UI
- **Reutilizable**: Puede ser usado desde CLI, API, o GUI
- **Puro**: Funciones deterministas cuando sea posible

#### Ejemplo:

```rust
// src/core/service.rs
use crate::error::{MscError, Result};

pub struct Service {
    config: Config,
}

impl Service {
    pub fn new(config: Config) -> Self {
        Self { config }
    }
    
    /// Process business logic
    pub fn process(&self, input: &str) -> Result<ProcessedData> {
        // Validación
        if input.is_empty() {
            return Err(MscError::invalid_path("Input cannot be empty"));
        }
        
        // Lógica de negocio
        let data = self.transform(input)?;
        
        Ok(data)
    }
    
    fn transform(&self, input: &str) -> Result<ProcessedData> {
        // Transformación compleja
        todo!()
    }
}
```

### 3. UI Layer (`src/ui/`)

**Responsabilidad**: Presentación y formateo de salida

#### ¿Qué va aquí?

- Funciones de formateo de datos
- Generación de tablas y listas
- Aplicación de colores y estilos
- Barras de progreso
- Prompts interactivos
- Cálculo de anchos de columna

#### ¿Qué NO va aquí?

- Lógica de negocio
- Acceso a archivos o configuración
- Validación de datos de negocio

#### Ejemplo:

```rust
// src/ui/formatters.rs
use colored::*;

pub fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    
    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

pub fn format_time(time: SystemTime) -> String {
    // Formateo consistente de fechas
    todo!()
}
```

### 4. Git Layer (`src/git/`)

**Responsabilidad**: Integración con Git

#### ¿Qué va aquí?

- Operaciones con repositorios Git (usando git2)
- Lectura de estado de archivos
- Parseo de .gitignore
- Determinación de colores por estado
- Verificación de cambios

#### Ejemplo:

```rust
// src/git/status.rs
use git2::Repository;
use std::path::Path;
use std::collections::HashMap;

pub fn load_git_status(path: &Path) -> Option<HashMap<PathBuf, Status>> {
    let repo = Repository::discover(path).ok()?;
    // Cargar y procesar estado
    Some(status_map)
}
```

### 5. Platform Layer (`src/platform/`)

**Responsabilidad**: Código específico del sistema operativo

#### ¿Qué va aquí?

- Funciones que usan `#[cfg(target_os)]`
- Operaciones con winapi (Windows)
- Operaciones con libc (Unix)
- Gestión de permisos y elevación
- Paths de sistema específicos de OS

#### Estructura:

```rust
// src/platform/elevation.rs
#[cfg(windows)]
pub fn is_elevated() -> bool {
    // Implementación Windows con winapi
}

#[cfg(unix)]
pub fn is_elevated() -> bool {
    // Implementación Unix con libc
}
```

### 6. Utils Layer (`src/utils/`)

**Responsabilidad**: Utilidades compartidas

#### ¿Qué va aquí?

- Funciones helper que no pertenecen a ninguna capa específica
- Constantes compartidas
- Pequeñas utilidades sin estado
- Mapeos y conversiones simples

---

## Convenciones de Nomenclatura

### Archivos y Directorios

```
snake_case          # Para archivos .rs
lowercase           # Para directorios
```

#### Ejemplos:
- ✅ `file_scanner.rs`
- ✅ `clean_temp.rs`
- ✅ `commands/`
- ❌ `FileScanner.rs`
- ❌ `cleanTemp.rs`

### Variables y Funciones

```rust
snake_case          # Variables, funciones, métodos
```

```rust
// ✅ Correcto
let user_name = "John";
let file_path = Path::new("./test");
fn calculate_total() -> u32 { }
fn get_config_path() -> Result<PathBuf> { }

// ❌ Incorrecto
let userName = "John";
let FilePath = Path::new("./test");
fn CalculateTotal() -> u32 { }
```

### Tipos (Structs, Enums, Traits)

```rust
PascalCase          # Structs, Enums, Traits, Type Aliases
```

```rust
// ✅ Correcto
struct UserConfig { }
enum FileType { }
trait Processor { }
type Result<T> = std::result::Result<T, MscError>;

// ❌ Incorrecto
struct user_config { }
enum file_type { }
trait processor { }
```

### Constantes

```rust
SCREAMING_SNAKE_CASE    # Constantes y statics
```

### Módulos

```rust
snake_case          # Nombres de módulos
```

### Acrónimos y Abreviaciones

- En tipos: Mantener en PascalCase: `HttpClient`, `JsonParser`
- En variables: Mantener en snake_case: `http_client`, `json_parser`

```rust
// ✅ Correcto
struct HttpClient { }
let http_client = HttpClient::new();

struct GitStatus { }
let git_status = GitStatus::default();

// ❌ Incorrecto
struct HTTPClient { }
struct gitStatus { }
```

---

## Código Autoexplicativo y Comentarios

### Filosofía: El Código Debe Hablar por Sí Mismo

Un código bien escrito es **autoexplicativo**. Si necesitas muchos comentarios para explicar qué hace tu código, es señal de que tu código no es suficientemente claro.

### Cuándo SÍ Usar Comentarios

✅ **Explicar el "por qué", no el "qué"**
```rust
// CORRECTO: Explica una decisión de diseño no obvia
// Usamos Base64 en lugar de escapado porque PowerShell tiene bugs
// con comillas anidadas en ciertos contextos de seguridad
let encoded = encode_base64(command);
```

✅ **Documentar APIs públicas con doc comments**
```rust
/// See docs/security.md for security considerations
pub fn validate_input(input: &str) -> Result<String> { ... }
```

✅ **Advertir sobre comportamientos peligrosos**
```rust
// WARNING: This deletes files permanently without confirmation
fn delete_all_files(path: &Path) -> Result<()> { ... }
```

### Cuándo NO Usar Comentarios

❌ **Comentarios que repiten el código**
```rust
// BAD: El código ya dice esto
// Create a new config
let config = Config::new();

// GOOD: Sin comentario innecesario
let config = Config::new();
```

❌ **Doc comments obvios**
```rust
// BAD: El nombre de la función ya lo dice
/// Returns true if the file exists
pub fn file_exists(&self) -> bool { ... }

// GOOD: Sin doc comment
pub fn file_exists(&self) -> bool { ... }
```

❌ **Comentarios inline en arrays/structs**
```rust
// BAD: Descripciones redundantes
const DANGEROUS_CHARS: &[(&str, &str)] = &[
    (";", "semicolon - command separator"),
    ("|", "pipe - command chaining"),
];

// GOOD: El contexto es suficiente
const DANGEROUS_CHARS: &[&str] = &[";", "|", "&", "$"];
```

❌ **Bloques masivos de comentarios de seguridad**
```rust
// BAD: 30 líneas de comentarios explicando seguridad
/// This function prevents command injection attacks by...
/// [30 líneas más]
pub fn validate() -> Result<()> { ... }

// GOOD: Referencia a documentación externa
/// See docs/security.md for security considerations
pub fn validate() -> Result<()> { ... }
```

### Preferir Nombres Claros Sobre Comentarios

**En lugar de:**
```rust
// Validate the URL to prevent injection
fn check(s: &str) -> bool { ... }
```

**Hacer:**
```rust
fn validate_url_against_injection(url: &str) -> Result<()> { ... }
```

### Regla de Oro

Si tu comentario puede ser reemplazado por un mejor nombre de variable/función, **elimina el comentario y mejora el nombre**.

### Documentación de Seguridad

Para funciones críticas de seguridad, la documentación detallada debe ir en archivos externos:
- `docs/security.md` - Consideraciones de seguridad
- `docs/architecture.md` - Decisiones arquitectónicas
- `docs/ffmpeg_implementation.md` - Implementación específica

---

## Gestión de Módulos

### Estructura de `mod.rs`

Cada directorio con submódulos debe tener un `mod.rs` que:

1. Declara los submódulos
2. Re-exporta los tipos públicos principales
3. Documenta el propósito del módulo

### Imports

#### Orden de imports:

```rust
// 1. Standard library
use std::fs;
use std::path::{Path, PathBuf};
use std::collections::HashMap;

// 2. External crates (en orden alfabético)
use anyhow::{Context, Result};
use colored::Colorize;
use serde::{Deserialize, Serialize};

// 3. Crate interno (absolute paths desde crate root)
use crate::core::Config;
use crate::error::{MscError, Result};
use crate::ui::format_size;

// 4. Módulo actual (relative paths)
use super::helper_function;
```

#### Reglas de imports:

1. **Usar paths absolutos desde crate root**: `use crate::core::Config`
2. **Agrupar imports por procedencia** (std, external, internal)
3. **Una línea por import** (para mejor legibilidad en diffs)
4. **Evitar glob imports** (`use module::*`) excepto en preludes
5. **Importar traits cuando sean necesarios**: `use std::io::Write;`

```rust
// ✅ Correcto
use std::fs;
use std::path::Path;
use crate::core::Config;
use crate::ui::format_size;

// ❌ Incorrecto
use crate::core::*;
use std::*;
use super::super::ui::format_size;  // Preferir absolute path
```

### Visibilidad (pub vs private)

```rust
// Público a todo el proyecto
pub struct Config { }

// Público solo dentro del crate
pub(crate) fn internal_helper() { }

// Público solo dentro del módulo padre
pub(super) fn parent_helper() { }

// Privado (default)
fn private_function() { }
```

#### Guía de visibilidad:

- **`pub`**: API pública, expuesta a usuarios del crate
- **`pub(crate)`**: Compartido entre módulos internos, no expuesto
- **`pub(super)`**: Compartido con módulo padre
- **private**: Implementación interna del módulo

---

## Manejo de Errores

### Arquitectura de Errores

El proyecto usa una combinación de `thiserror` y `anyhow`:

```rust
// src/error.rs
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MscError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    
    #[error("Configuration error: {0}")]
    Config(String),
    
    #[error("Git error: {0}")]
    Git(#[from] git2::Error),
    
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
    
    #[error("Invalid path: {0}")]
    InvalidPath(String),
    
    #[error("Workspace error: {0}")]
    Workspace(String),
    
    #[error("Elevation required: {0}")]
    ElevationRequired(String),
    
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, MscError>;
```

### Cuándo usar qué tipo de Result

#### `crate::error::Result<T>` (MscError)

Usar en **Core layer** y **Platform layer**:

```rust
// src/core/config.rs
use crate::error::{MscError, Result};

impl Config {
    pub fn load() -> Result<Self> {
        // Puede retornar MscError específico
    }
    
    pub fn save(&self) -> Result<()> {
        // Control preciso del tipo de error
    }
}
```

#### `anyhow::Result<T>`

Usar en **Commands layer** y funciones de alto nivel:

```rust
// src/commands/config.rs
use anyhow::{Context, Result};

pub fn execute(matches: &ArgMatches) -> Result<()> {
    let config = Config::load()
        .context("Failed to load configuration")?;
    
    // anyhow permite agregar contexto fácilmente
    config.save()
        .context("Failed to save configuration")?;
    
    Ok(())
}
```

### Patrón de Constructores de Error

```rust
impl MscError {
    /// Create a config error
    pub fn config<S: Into<String>>(msg: S) -> Self {
        MscError::Config(msg.into())
    }
    
    /// Create a workspace error
    pub fn workspace<S: Into<String>>(msg: S) -> Self {
        MscError::Workspace(msg.into())
    }
}

// Uso:
return Err(MscError::config("Invalid configuration format"));
return Err(MscError::workspace("Workspace not found"));
```

### Manejo de Errores en Código

```rust
// ✅ Correcto: Propagar con contexto
fn load_file(path: &Path) -> anyhow::Result<String> {
    fs::read_to_string(path)
        .context(format!("Failed to read file: {:?}", path))
}

// ✅ Correcto: Convertir a error custom
fn validate_path(path: &str) -> Result<PathBuf> {
    let path_buf = PathBuf::from(path);
    if !path_buf.exists() {
        return Err(MscError::invalid_path(
            format!("Path does not exist: {}", path)
        ));
    }
    Ok(path_buf)
}

// ❌ Incorrecto: unwrap en código de producción
fn bad_function(path: &str) -> String {
    fs::read_to_string(path).unwrap()  // Nunca hacer esto
}

// ❌ Incorrecto: Ignorar errores
fn also_bad() {
    let _ = some_operation();  // Error ignorado
}
```

### Logging de Errores

```rust
use log::{error, warn, info, debug};

// Log al manejar errores
pub fn execute(matches: &ArgMatches) -> Result<()> {
    match dangerous_operation() {
        Ok(result) => {
            info!("Operation completed successfully");
            Ok(())
        }
        Err(e) => {
            error!("Operation failed: {:#}", e);  // {:#} para pretty print
            Err(e)
        }
    }
}
```

---

## Testing

### Estructura de Tests

```
tests/
├── integration.rs           # Entry point, declara módulos
├── integration/
│   ├── commands_test.rs    # Tests de comandos
│   ├── config_test.rs      # Tests de configuración
│   └── workspace_test.rs   # Tests de workspace
└── fixtures/                # Datos de prueba
    ├── test_workspace/
    └── sample_config.bin
```

### Tests Unitarios

Los tests unitarios van al final del archivo del módulo:

```rust
// src/core/config.rs

impl Config {
    pub fn load() -> Result<Self> {
        // implementación
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    
    #[test]
    fn test_load_creates_default_when_missing() {
        let config = Config::load().unwrap();
        assert!(config.work_path.is_none());
    }
    
    #[test]
    fn test_save_creates_directory() {
        let dir = tempdir().unwrap();
        // Test implementation
    }
}
```

### Tests de Integración

```rust
// tests/integration/commands_test.rs

use msc::commands;
use std::path::PathBuf;
use tempfile::tempdir;

#[test]
fn test_list_command_lists_files() {
    let temp = tempdir().unwrap();
    let test_file = temp.path().join("test.txt");
    std::fs::write(&test_file, "test content").unwrap();
    
    // Test del comando
    let result = commands::list::execute_simple(temp.path());
    assert!(result.is_ok());
}

#[test]
fn test_list_handles_nonexistent_directory() {
    let result = commands::list::execute_simple(Path::new("/nonexistent"));
    assert!(result.is_ok());  // Maneja gracefully
}
```

### Convenciones de Testing

1. **Nombre de tests**: `test_<function>_<scenario>`
   ```rust
   #[test]
   fn test_parse_config_with_valid_input() { }
   
   #[test]
   fn test_parse_config_with_empty_file() { }
   
   #[test]
   fn test_parse_config_returns_error_on_invalid_format() { }
   ```

2. **Usar tempfile para archivos temporales**:
   ```rust
   use tempfile::{tempdir, NamedTempFile};
   
   #[test]
   fn test_with_temp_directory() {
       let dir = tempdir().unwrap();
       let file_path = dir.path().join("test.txt");
       // Test usa file_path
       // dir se limpia automáticamente al final
   }
   ```

3. **Tests deben ser independientes**: No depender del orden de ejecución

4. **Tests deben ser deterministas**: Mismo input, mismo output

5. **Usar fixtures para datos complejos**:
   ```rust
   fn fixture_sample_config() -> Config {
       Config {
           work_path: Some("/test/path".to_string()),
           workspaces: HashMap::new(),
       }
   }
   ```

6. **Tests de error**:
   ```rust
   #[test]
   fn test_load_fails_with_corrupted_file() {
       let result = Config::load_from_bytes(&[0xFF, 0xFF]);
       assert!(result.is_err());
       
       // O más específico:
       match result {
           Err(MscError::Serialization(_)) => { /* OK */ }
           _ => panic!("Expected Serialization error"),
       }
   }
   ```

### Cuándo Escribir Tests

#### ✅ Siempre escribir tests para:

- Lógica de negocio en Core layer
- Parsing y validación de datos
- Manejo de casos edge
- Funciones de transformación
- Operaciones de configuración

#### ⚠️ Tests opcionales para:

- Código de UI (formateo, colores)
- Funciones triviales (getters/setters)
- Código de infraestructura simple

#### ❌ No escribir tests para:

- Código generado
- Código que solo delega a otras funciones

---

## Configuración

### Gestión de Configuración

La configuración se maneja mediante serialización binaria con `bincode`:

```rust
// src/core/config.rs
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Config {
    pub work_path: Option<String>,
    pub workspaces: HashMap<String, String>,
}

impl Config {
    pub fn load() -> Result<Self> {
        let config_path = Self::get_config_path()?;
        
        if !config_path.exists() {
            return Ok(Config::default());
        }
        
        let data = fs::read(&config_path)?;
        let config = bincode::deserialize(&data)
            .unwrap_or_default();  // Fallback a default si corrupto
        
        Ok(config)
    }
    
    pub fn save(&self) -> Result<()> {
        let config_path = Self::get_config_path()?;
        
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }
        
        let data = bincode::serialize(self)?;
        fs::write(&config_path, data)?;
        
        Ok(())
    }
    
    fn get_config_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| MscError::config("Could not determine config directory"))?;
        
        Ok(config_dir.join("msc").join("config.bin"))
    }
}
```

### Ubicación de Archivos de Configuración

Usar el crate `dirs` para paths multiplataforma:

```rust
// Linux/macOS: ~/.config/msc/config.bin
// Windows: C:\Users\<user>\AppData\Roaming\msc\config.bin
let config_dir = dirs::config_dir()?;

// Linux/macOS: ~/.cache/msc/
// Windows: C:\Users\<user>\AppData\Local\msc\
let cache_dir = dirs::cache_dir()?;

// Home directory
let home_dir = dirs::home_dir()?;
```

### Agregar Nuevas Opciones de Configuración

1. Agregar campo al struct `Config`:
   ```rust
   #[derive(Debug, Serialize, Deserialize, Default)]
   pub struct Config {
       pub work_path: Option<String>,
       pub workspaces: HashMap<String, String>,
       #[serde(default)]  // Importante para compatibilidad
       pub new_option: bool,
   }
   ```

2. Agregar método getter/setter:
   ```rust
   impl Config {
       pub fn set_new_option(&mut self, value: bool) {
           self.new_option = value;
       }
       
       pub fn get_new_option(&self) -> bool {
           self.new_option
       }
   }
   ```

3. Agregar comando CLI en `main.rs` si es necesario

4. Escribir tests

---

## Dependencias

### Agregar Nueva Dependencia

#### Proceso:

1. **Evaluar necesidad**: ¿Realmente necesitamos esta dependencia?
2. **Verificar alternativas**: ¿Hay algo en stdlib que sirva?
3. **Verificar mantenimiento**: ¿El crate está activamente mantenido?
4. **Verificar licencia**: ¿Es compatible con MIT?
5. **Agregar con versión específica**: No usar `*`

### Dependency Features

Habilitar solo features necesarios:

```toml
# ✅ Correcto
serde = { version = "1.0", features = ["derive"] }
chrono = { version = "0.4", features = ["serde"] }

# ❌ Incorrecto (muchos features innecesarios)
tokio = { version = "1.0", features = ["full"] }
```

---

## Guías de Desarrollo

### Agregar un Nuevo Comando

#### Checklist:

1. ✅ Crear archivo en `src/commands/new_command.rs`
2. ✅ Implementar función `execute(matches: &ArgMatches) -> Result<()>`
3. ✅ Declarar módulo en `src/commands/mod.rs`
4. ✅ Agregar subcomando en `main.rs::build_cli()`
5. ✅ Agregar caso en `main.rs::match` dispatcher
6. ✅ Implementar lógica en Core layer si es complejo
7. ✅ Escribir tests de integración
8. ✅ Documentar en README.md

#### Ejemplo:

```rust
// 1. src/commands/new_command.rs
use anyhow::Result;
use clap::ArgMatches;
use crate::core::NewService;

/// Execute the new command
pub fn execute(matches: &ArgMatches) -> Result<()> {
    let input = matches.get_one::<String>("input")
        .expect("Input is required");
    
    let service = NewService::new();
    let result = service.process(input)?;
    
    println!("Result: {}", result);
    Ok(())
}

// 2. src/commands/mod.rs
pub mod new_command;

// 3. main.rs - build_cli()
.subcommand(
    Command::new("new-command")
        .about("Description of the new command")
        .arg(
            Arg::new("input")
                .help("Input parameter")
                .required(true)
                .index(1),
        ),
)

// 4. main.rs - match dispatcher
match matches.subcommand() {
    Some(("new-command", sub_matches)) => {
        commands::new_command::execute(sub_matches)
    }
    // ... otros comandos
}

// 5. tests/integration/new_command_test.rs
#[test]
fn test_new_command_executes() {
    // Test implementation
}
```

### Agregar Nueva Funcionalidad a Core

#### Checklist:

1. ✅ Decidir ubicación: ¿Nuevo archivo o archivo existente?
2. ✅ Crear struct/enum con documentación
3. ✅ Implementar métodos con Result<T>
4. ✅ Agregar a `mod.rs` si es nuevo archivo
5. ✅ Re-exportar desde `mod.rs` si es API pública
6. ✅ Escribir tests unitarios
7. ✅ Usar desde Commands layer

#### Ejemplo:

```rust
// src/core/new_service.rs

use crate::error::{MscError, Result};

/// Service for doing something important
pub struct NewService {
    config: Config,
}

impl NewService {
    /// Create a new instance
    pub fn new() -> Self {
        Self {
            config: Config::default(),
        }
    }
    
    /// Process input and return result
    pub fn process(&self, input: &str) -> Result<String> {
        // Validación
        if input.is_empty() {
            return Err(MscError::other("Input cannot be empty"));
        }
        
        // Procesamiento
        let result = format!("Processed: {}", input);
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_process_returns_result() {
        let service = NewService::new();
        let result = service.process("test").unwrap();
        assert_eq!(result, "Processed: test");
    }
    
    #[test]
    fn test_process_fails_with_empty_input() {
        let service = NewService::new();
        assert!(service.process("").is_err());
    }
}

// src/core/mod.rs
pub mod new_service;
pub use new_service::NewService;
```

### Agregar Funcionalidad Específica de Plataforma

```rust
// src/platform/new_feature.rs

#[cfg(windows)]
pub fn platform_specific_function() -> bool {
    use winapi::um::...;
    // Implementación Windows
    true
}

#[cfg(unix)]
pub fn platform_specific_function() -> bool {
    use libc::...;
    // Implementación Unix
    true
}

#[cfg(not(any(windows, unix)))]
pub fn platform_specific_function() -> bool {
    // Fallback para otras plataformas
    false
}
```

### Cuándo Crear un Nuevo Archivo vs Ampliar Existente

#### Crear NUEVO archivo cuando:

- ✅ La funcionalidad es conceptualmente diferente
- ✅ El archivo actual tiene >500 líneas
- ✅ Es un nuevo servicio o manager
- ✅ Necesita su propio conjunto de tests

#### Ampliar archivo EXISTENTE cuando:

- ✅ Es una extensión natural de funcionalidad existente
- ✅ Comparte estado con el código existente
- ✅ El archivo tiene <300 líneas
- ✅ Es un helper o método adicional

---

## Patrones y Buenas Prácticas

### 1. Builder Pattern para Structs Complejos

```rust
pub struct FileScanner {
    max_depth: u32,
    show_hidden: bool,
    follow_symlinks: bool,
}

impl FileScanner {
    pub fn builder() -> FileScannerBuilder {
        FileScannerBuilder::default()
    }
}

#[derive(Default)]
pub struct FileScannerBuilder {
    max_depth: Option<u32>,
    show_hidden: bool,
    follow_symlinks: bool,
}

impl FileScannerBuilder {
    pub fn max_depth(mut self, depth: u32) -> Self {
        self.max_depth = Some(depth);
        self
    }
    
    pub fn show_hidden(mut self, show: bool) -> Self {
        self.show_hidden = show;
        self
    }
    
    pub fn build(self) -> FileScanner {
        FileScanner {
            max_depth: self.max_depth.unwrap_or(1),
            show_hidden: self.show_hidden,
            follow_symlinks: self.follow_symlinks,
        }
    }
}

// Uso:
let scanner = FileScanner::builder()
    .max_depth(3)
    .show_hidden(true)
    .build();
```

### 2. Newtype Pattern para Type Safety

```rust
// En lugar de usar String en todas partes
pub struct WorkspaceName(String);
pub struct WorkspacePath(PathBuf);

impl WorkspaceName {
    pub fn new(name: String) -> Result<Self> {
        if name.is_empty() {
            return Err(MscError::workspace("Name cannot be empty"));
        }
        Ok(Self(name))
    }
    
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
```

### 3. Extension Traits para Funcionalidad Adicional

```rust
// src/utils/path_ext.rs
use std::path::Path;

pub trait PathExt {
    fn is_hidden(&self) -> bool;
    fn size(&self) -> Result<u64>;
}

impl PathExt for Path {
    fn is_hidden(&self) -> bool {
        self.file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.starts_with('.'))
            .unwrap_or(false)
    }
    
    fn size(&self) -> Result<u64> {
        Ok(fs::metadata(self)?.len())
    }
}

// Uso:
use crate::utils::PathExt;
if path.is_hidden() { ... }
```

### 4. Early Returns para Validación

```rust
// ✅ Correcto
pub fn process(path: &Path) -> Result<()> {
    if !path.exists() {
        return Err(MscError::invalid_path("Path does not exist"));
    }
    
    if !path.is_dir() {
        return Err(MscError::invalid_path("Path is not a directory"));
    }
    
    // Lógica principal
    perform_processing(path)?;
    Ok(())
}

// ❌ Incorrecto (muchos niveles de anidación)
pub fn process(path: &Path) -> Result<()> {
    if path.exists() {
        if path.is_dir() {
            perform_processing(path)?;
            Ok(())
        } else {
            Err(MscError::invalid_path("Path is not a directory"))
        }
    } else {
        Err(MscError::invalid_path("Path does not exist"))
    }
}
```

### 5. Usar `impl Trait` para Flexibilidad

```rust
// Acepta cualquier cosa que se pueda convertir a String
pub fn set_name(name: impl Into<String>) {
    self.name = name.into();
}

// Acepta cualquier Path-like
pub fn process_path(path: impl AsRef<Path>) -> Result<()> {
    let path = path.as_ref();
    // ...
}

// Retorna un iterador sin especificar el tipo exacto
pub fn get_files(&self) -> impl Iterator<Item = &PathBuf> {
    self.files.iter()
}
```

### 6. Logging Apropiado

```rust
use log::{trace, debug, info, warn, error};

pub fn process_files(&self) -> Result<()> {
    info!("Starting file processing");
    debug!("Processing {} files", self.files.len());
    
    for file in &self.files {
        trace!("Processing file: {:?}", file);
        
        match self.process_single(file) {
            Ok(_) => debug!("Successfully processed: {:?}", file),
            Err(e) => {
                warn!("Failed to process file {:?}: {}", file, e);
                continue;
            }
        }
    }
    
    info!("File processing completed");
    Ok(())
}
```

#### Niveles de log:

- **error**: Errores críticos que requieren atención inmediata
- **warn**: Situaciones anormales pero manejables
- **info**: Información general sobre el flujo de la aplicación
- **debug**: Información útil para debugging
- **trace**: Información muy detallada (cada iteración, etc.)

### 8. Const para Valores Mágicos

```rust
// ❌ Incorrecto
if depth > 10 {
    return Err(...);
}

// ✅ Correcto
const MAX_RECURSION_DEPTH: u32 = 10;

if depth > MAX_RECURSION_DEPTH {
    return Err(...);
}
```

### 9. Type Aliases para Complejidad

```rust
// En lugar de repetir este tipo complejo
pub fn get_status() -> std::result::Result<HashMap<PathBuf, FileStatus>, MscError> {
    // ...
}

// Usar type alias
pub type StatusMap = HashMap<PathBuf, FileStatus>;
pub type Result<T> = std::result::Result<T, MscError>;

pub fn get_status() -> Result<StatusMap> {
    // ...
}
```

### 10. Evitar Clones Innecesarios

```rust
// ❌ Incorrecto (clone innecesario)
pub fn process(&self, data: Vec<String>) -> Result<()> {
    for item in data.clone().iter() {  // Clone innecesario
        // ...
    }
}

// ✅ Correcto (borrow)
pub fn process(&self, data: &[String]) -> Result<()> {
    for item in data {
        // ...
    }
}

// ✅ Correcto (consume ownership si es necesario)
pub fn process(self, data: Vec<String>) -> Result<()> {
    for item in data {  // data se mueve
        // ...
    }
}
```

---

## Checklist de Pre-Commit

Antes de hacer commit, verificar:

- [ ] `cargo fmt` ejecutado (código formateado)
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` sin warnings
- [ ] `cargo test` todos los tests pasan
- [ ] `cargo build --release` compila sin errores
- [ ] No hay `println!` de debug olvidados
- [ ] No hay `unwrap()` o `expect()` en código de producción
- [ ] Código nuevo tiene tests
- [ ] Código nuevo tiene documentación
- [ ] Imports organizados correctamente
- [ ] No hay código comentado (eliminar o justificar)
- [ ] Variables y funciones tienen nombres descriptivos

---

### Herramientas

```bash
# Formatting
cargo fmt

# Linting
cargo clippy

# Testing
cargo test
cargo test -- --nocapture  # Ver println en tests

# Documentation
cargo doc --open

# Check sin compilar
cargo check

# Build optimizado
cargo build --release
```
