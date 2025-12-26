El comando msc wget cookies extrae cookies de navegadores basados en Chromium (Chrome, Edge, Brave) accediendo directamente a su base de datos SQLite local. Este proceso involucra localización de archivos, manejo de bloqueos de base de datos, queries SQL específicas, y formateo de datos.

1. Arquitectura General

  Usuario ejecuta comando
      ↓
  Parseo de argumentos (URL, navegador, formato)
      ↓
  Localización de base de datos SQLite
      ↓
  Copia temporal de la BD (evitar bloqueos)
      ↓
  Manejo de archivos WAL/SHM (navegador abierto)
      ↓
  Query SQL a la base de datos
      ↓
  Extracción y estructuración de cookies
      ↓
  Formateo según tipo solicitado (wget/json/netscape)
      ↓
  Salida a archivo o stdout

1.2 Ubicación del Código

- Comando: src/commands/wget.rs → función execute_cookies()
- Lógica Core: src/core/wget/wget_cookies.rs


2. Localización de la Base de Datos de Chrome

  2.1 Función: find_browser_cookie_db()

  Ruta en Windows:
  PathBuf::from(LOCALAPPDATA)
      .join("Google\\Chrome\\User Data\\Default\\Network\\Cookies")

  Ruta completa:
  C:\Users\<usuario>\AppData\Local\Google\Chrome\User Data\Default\Network\Cookies

  Detalles Técnicos:
  1. Variable de entorno: LOCALAPPDATA
    - Se obtiene mediante env::var("LOCALAPPDATA")
    - Apunta típicamente a C:\Users\<usuario>\AppData\Local
  2. Estructura de carpetas de Chrome:
    - Google\Chrome\User Data\ → Directorio raíz de perfiles
    - Default\ → Perfil por defecto (puede haber otros: Profile 1, Profile 2, etc.)
    - Network\ → Subcarpeta que contiene datos de red
    - Cookies → Archivo de base de datos SQLite (sin extensión)
  3. Validación:
  if !cookie_path.exists() {
      return Err(anyhow!("No se encontró la base de datos..."))
  }

3. Manejo de Base de Datos SQLite

3.1 Problema: Bloqueo de Base de Datos

  Chrome mantiene la base de datos bloqueada cuando está en ejecución. Intentar leer directamente resulta en error.

3.2 Solución: Copia Temporal

  Ubicación: src/core/wget/wget_cookies.rs:218-243

  let temp_db = env::temp_dir()
      .join(format!("msc_cookies_temp_{}.db", std::process::id()));

  fs::copy(db_path, &temp_db)
      .context("No se pudo copiar la base de datos...")?;

  Detalles:

- Crea archivo temporal en C:\Users\<usuario>\AppData\Local\Temp\
- Nombre único: msc_cookies_temp_<PID>.db (evita colisiones)
- Permite lectura sin conflictos con Chrome

3.3 Manejo de Archivos WAL/SHM

  Ubicación: src/core/wget/wget_cookies.rs:225-242

  ¿Qué son?
- WAL (Write-Ahead Log): Cookies-wal
  - Contiene cambios recientes no consolidados en la BD principal
  - Usado por SQLite para mejorar rendimiento y concurrencia
- SHM (Shared Memory): Cookies-shm
  - Índice compartido para acceso rápido al WAL
  - Usado internamente por SQLite

  Implementación:

  let wal_path = db_path.with_extension("sqlite-wal");
  let shm_path = db_path.with_extension("sqlite-shm");

  if wal_path.exists() && fs::copy(&wal_path, &temp_wal).is_ok() {
      println!("⚠️ Navegador abierto detectado, leyendo archivo WAL...");
  }

  Importancia:

- Si Chrome está abierto, las cookies más recientes pueden estar solo en el WAL
- Copiar el WAL asegura obtener cookies actualizadas
- Si no se copia, podrían perderse cookies recientes

4. Librería SQLite: rusqlite

4.1 Dependencia

  Cargo.toml (implícito):
  [dependencies]
  rusqlite = "0.x.x"

  4.2 Uso en el Código

  use rusqlite::Connection;

  let conn = Connection::open(&temp_db)
      .context("No se pudo abrir la base de datos de cookies")?;

  rusqlite es el binding de SQLite para Rust:

- Interfaz segura y tipo-safe para SQLite
- Manejo automático de memoria
- Soporte para prepared statements


5. Esquema de Base de Datos de Chrome

5.1 Estructura de la Tabla cookies

Ubicación del query: src/core/wget/wget_cookies.rs:250

  Columnas Principales:

  | Columna     | Tipo    | Descripción                                                         |
  |-------------|---------|---------------------------------------------------------------------|
  | name        | TEXT    | Nombre de la cookie (ej: "session_id")                              |
  | value       | TEXT    | Valor de la cookie (generalmente encriptado en versiones recientes) |
  | host_key    | TEXT    | Dominio asociado (ej: ".github.com")                                |
  | path        | TEXT    | Ruta del dominio (ej: "/")                                          |
  | expires_utc | INTEGER | Timestamp de expiración (formato Chrome/Webkit)                     |
  | is_secure   | INTEGER | 1 si requiere HTTPS, 0 si no                                        |

  Nota Importante sobre value:
  En versiones recientes de Chrome (80+), los valores están encriptados usando DPAPI en Windows. El código actual NO desencripta estos valores, lo que puede causar que las cookies extraídas no
  funcionen.

6. Query SQL y Extracción

 6.1 Query Principal

  Ubicación: src/core/wget/wget_cookies.rs:250-280

  SELECT name, value, host_key, path, expires_utc, is_secure
  FROM cookies
  WHERE host_key LIKE ?1 OR host_key LIKE ?2 OR host_key = ?3

  6.2 Patrones de Búsqueda

  Ubicación: src/core/wget/wget_cookies.rs:257-261

  Para un dominio como github.com:

  let clean_domain = domain.strip_prefix("www.").unwrap_or(domain);
  // clean_domain = "github.com"

  let dot_domain_pattern = format!(".{}", clean_domain);
  // ".github.com" → matches subdomain cookies

  let exact_domain_pattern = clean_domain.to_string();
  // "github.com" → matches exact domain

  let www_domain_pattern = format!("www.{}", clean_domain);
  // "www.github.com" → matches www variant

  Propósito: Capturar cookies de:

- Dominio exacto: github.com
- Subdominio wildcard: .github.com (válido para api.github.com, gist.github.com, etc.)
- Variante www: <www.github.com>

  6.3 Mapeo de Resultados

  let cookie_iter = stmt.query_map(
      [&dot_domain_pattern, &exact_domain_pattern, &www_domain_pattern],
      |row| {
          Ok(Cookie {
              name: row.get(0)?,      // nombre
              value: row.get(1)?,     // valor
              domain: row.get(2)?,    // host_key
              path: row.get(3)?,      // path
              expires: row.get(4)?,   // expires_utc
              secure: row.get::<_, i64>(5)? != 0,  // is_secure (0 o 1)
          })
      },
  )?;

  rusqlite convierte automáticamente tipos SQLite a Rust:

- TEXT → String
- INTEGER → i64
- Manejo de NULL con Option<T>

  ---

  7. Estructura de Datos: Cookie

  Ubicación: src/core/wget/wget_cookies.rs:11-19

  #[derive(Debug, Clone)]
  pub struct Cookie {
      pub name: String,
      pub value: String,
      pub domain: String,
      pub path: String,
      pub expires: i64,
      pub secure: bool,
  }

  Detalles:

- #[derive(Debug, Clone)]: Auto-implementa traits para debugging y clonación
- Todos los campos públicos para fácil acceso
- expires es timestamp Unix (Chrome usa formato Webkit internamente, pero se lee como i64)

  ---

  8. Formateo de Cookies

  8.1 Formato WGET (por defecto)

  Ubicación: src/core/wget/wget_cookies.rs:356-364

  let cookie_pairs: Vec<String> = cookies
      .iter()
      .map(|c| format!("{}={}", c.name, c.value))
      .collect();
  Ok(cookie_pairs.join("; "))

  Salida:
  session_id=abc123; user_token=xyz789; preferences=dark_mode

  Uso:
  msc wget "<https://example.com>" --cookies 'session_id=abc123; user_token=xyz789'

  8.2 Formato Netscape

  Ubicación: src/core/wget/wget_cookies.rs:384-410

  for cookie in cookies {
      output.push_str(&format!(
          "{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
          cookie.domain,      // .github.com
          domain_flag,        // TRUE/FALSE (subdomain wildcard)
          cookie.path,        // /
          secure_flag,        // TRUE/FALSE (HTTPS only)
          cookie.expires,     // 1234567890
          cookie.name,        // session_id
          cookie.value        // abc123
      ));
  }

  Formato Netscape (7 campos separados por TAB):
  .github.com   TRUE    /       FALSE   1893456000      session_id      abc123

  8.3 Formato JSON

  Ubicación: src/core/wget/wget_cookies.rs:365-382

  serde_json::to_string_pretty(
      &cookies.iter().map(|c| {
          serde_json::json!({
              "name": c.name,
              "value": c.value,
              "domain": c.domain,
              "path": c.path,
              "expires": c.expires,
              "secure": c.secure,
          })
      }).collect::<Vec<_>>(),
  )?

  Salida:
  [
    {
      "name": "session_id",
      "value": "abc123",
      "domain": ".github.com",
      "path": "/",
      "expires": 1893456000,
      "secure": true
    }
  ]

  ---

  9. Limpieza de Recursos

  Ubicación: src/core/wget/wget_cookies.rs:346-349

  let _= fs::remove_file(&temp_db);
  let_ = fs::remove_file(&temp_wal);
  let _= fs::remove_file(&temp_shm);

  Nota:

- let _ = ... ignora errores de eliminación
- Los archivos temp se auto-eliminan al cerrar el sistema de todas formas
- Previene acumulación de archivos temporales

  ---

  10. Limitaciones y Áreas de Mejora

  10.1 Encriptación de Valores

  Problema: Chrome 80+ encripta valores de cookies usando:

- Windows: DPAPI (Data Protection API)
- macOS: Keychain
- Linux: libsecret o keyring

  Estado actual:

- El código lee el campo value directamente sin desencriptar
- Las cookies pueden tener valores encriptados inservibles

  Solución potencial:
  // Windows: Usar DPAPI
  use winapi::um::dpapi::CryptUnprotectData;

  fn decrypt_chrome_cookie_value(encrypted: &[u8]) -> Result<String> {
      // Implementar desencriptación DPAPI
      // El valor en BD está como BLOB, no como TEXT en versiones recientes
  }

  10.2 Soporte Multi-Perfil

  Problema: Solo lee el perfil Default

  Mejora:

- Detectar todos los perfiles (Profile 1, Profile 2, etc.)
- Permitir al usuario elegir perfil
- O buscar en todos los perfiles

  10.3 Manejo de Navegador Abierto

  Problema: Si Chrome está abierto, la copia puede fallar o dar datos incompletos

  Mejora actual:

- Copia archivos WAL/SHM
- Mensaje informativo al usuario

  Mejora potencial:

- Usar API de Chrome Remote Debugging Protocol
- Solicitar al usuario cerrar el navegador temporalmente

  ---

  11. Dependencias Clave

  [dependencies]
  rusqlite = "0.x.x"           # SQLite binding
  anyhow = "1.x.x"             # Error handling
  colored = "2.x.x"            # Terminal colors
  url = "2.x.x"                # URL parsing
  serde_json = "1.x.x"         # JSON serialization

  ---

  12. Ejemplo de Uso Completo

# 1. Extracción básica (Chrome, formato wget)

  msc wget cookies <https://github.com>

# Salida

# session_id=abc123; user_token=xyz789

# 2. Uso con wget

  msc wget "<https://github.com/user/private-repo>" \
      --cookies 'session_id=abc123; user_token=xyz789'

# 3. Extracción de Firefox en JSON

  msc wget cookies <https://github.com> --browser firefox --format json -o cookies.json

# 4. Debug de base de datos

  msc wget cookies <https://github.com> --debug
