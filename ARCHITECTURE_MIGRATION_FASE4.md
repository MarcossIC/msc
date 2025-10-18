### FASE 4: Extraer UI/Prompts (BAJO RIESGO)
**Duración estimada**: 1 hora
**Riesgo**: Bajo

#### Paso 4.1: Extraer ui/prompts.rs
```rust
// src/ui/prompts.rs
use std::io::{self, Write};
use colored::Colorize;

/// Ask user for yes/no confirmation
pub fn confirm(message: &str) -> io::Result<bool> {
    print!("{} (y/n): ", message.white().bold());
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    let response = input.trim().to_lowercase();
    Ok(response == "y" || response == "yes")
}

/// Display a warning message
pub fn warn(message: &str) {
    println!("{}", format!("⚠️  Warning: {}", message).yellow().bold());
}

/// Display an info message
pub fn info(message: &str) {
    println!("{}", message.cyan());
}

/// Display a success message
pub fn success(message: &str) {
    println!("{}", message.green().bold());
}

/// Display an error message
pub fn error(message: &str) {
    println!("{}", message.red().bold());
}
```

#### Paso 4.2: Actualizar src/ui/mod.rs
```rust
// src/ui/mod.rs
pub mod formatters;
pub mod prompts;
pub mod progress;  // Para futuras barras de progreso

// Re-exports
pub use formatters::{format_size, format_time, format_permissions};
pub use prompts::{confirm, warn, info, success, error};
```

#### Paso 4.3: Actualizar imports en main.rs
**En src/main.rs**, usar los nuevos helpers de UI:

```rust
// Reemplazar con:
use msc::ui::{confirm, warn, info, success, error};
```

#### Paso 4.4: Compilar, validar y commit
```bash
cargo build
cargo test
```

**Validación**: 
- ✅ Debe compilar sin errores
- ✅ UI helpers funcionando correctamente

```bash
git add .
git commit -m "feat: phase 4 - extract UI prompts and formatters"
```
