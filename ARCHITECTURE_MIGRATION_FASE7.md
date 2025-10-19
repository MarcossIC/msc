### FASE 7: Refactorizar main.rs (CRÍTICO)
**Duración estimada**: 2-3 horas
**Riesgo**: Crítico

#### Paso 7.1: Nuevo main.rs simplificado
```rust
// src/main.rs
use msc::commands;
use msc::error::Result;
use clap::{Arg, Command};

fn main() -> Result<()> {
    // Initialize logger
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();
    
    log::info!("Starting MSC CLI v{}", env!("CARGO_PKG_VERSION"));
    
    let matches = build_cli().get_matches();

    // Handle version flag
    if matches.get_flag("version") {
        commands::version::execute()?;
        return Ok(());
    }

    // Dispatch commands
    match matches.subcommand() {
        Some(("hello", sub_matches)) => commands::hello::execute(sub_matches),
        Some(("version", _)) => commands::version::execute(),
        Some(("set", sub_matches)) => commands::config::handle_set(sub_matches),
        Some(("get", sub_matches)) => commands::config::handle_get(sub_matches),
        Some(("work", sub_matches)) => commands::workspace::execute(sub_matches),
        Some(("clean-temp", sub_matches)) => commands::clean_temp::execute(sub_matches),
        Some(("list", sub_matches)) => commands::list::execute(sub_matches),
        _ => {
            println!("Welcome to MSC CLI!");
            println!("Use 'msc --help' for more information.");
            Ok(())
        }
    }
}

fn build_cli() -> Command {
    Command::new("msc")
        .version(env!("CARGO_PKG_VERSION"))
        .author("Marco")
        .about("A modular command-line interface tool")
        .disable_version_flag(true)
        .arg(
            Arg::new("version")
                .short('v')
                .short_alias('V')
                .long("version")
                .help("Print version information")
                .action(clap::ArgAction::SetTrue)
        )
        .subcommand(
            Command::new("hello")
                .about("Says hello")
                .arg(
                    Arg::new("name")
                        .short('n')
                        .long("name")
                        .value_name("NAME")
                        .help("Name to greet")
                        .default_value("World")
                )
        )
        .subcommand(
            Command::new("version")
                .about("Shows version information")
        )
        .subcommand(
            Command::new("list")
                .about("List files and directories")
                .arg(
                    Arg::new("path")
                        .help("Path to list")
                        .default_value(".")
                        .index(1)
                )
                .arg(
                    Arg::new("all")
                        .short('a')
                        .long("all")
                        .help("Show hidden files")
                        .action(clap::ArgAction::SetTrue)
                )
                .arg(
                    Arg::new("long")
                        .short('l')
                        .long("long")
                        .help("Show detailed information")
                        .action(clap::ArgAction::SetTrue)
                )
        )
        .subcommand(
            Command::new("set")
                .about("Set configuration values")
                .subcommand_required(true)
                .arg_required_else_help(true)
                .subcommand(
                    Command::new("work")
                        .about("Set work directory path")
                        .arg(
                            Arg::new("path")
                                .help("Path to the work directory")
                                .required(true)
                                .index(1)
                        )
                )
        )
        .subcommand(
            Command::new("get")
                .about("Get configuration values")
                .subcommand_required(true)
                .arg_required_else_help(true)
                .subcommand(
                    Command::new("work")
                        .about("Get work directory path")
                )
        )
        .subcommand(
            Command::new("work")
                .about("Manage workspaces")
                .subcommand_required(true)
                .arg_required_else_help(true)
                .subcommand(
                    Command::new("map")
                        .about("Map project folders as workspaces")
                )
                .subcommand(
                    Command::new("list")
                        .about("List all registered workspaces")
                )
        )
        .subcommand(
            Command::new("clean-temp")
                .about("Clean temporary files from the system")
                .arg(
                    Arg::new("dry-run")
                        .long("dry-run")
                        .help("Show what would be deleted without actually deleting")
                        .action(clap::ArgAction::SetTrue)
                )
        )
}
```

#### Paso 7.2: Compilar, validar y commit
```bash
cargo build --release
cargo test
cargo clippy -- -D warnings
```

**Validación completa**: 
- ✅ Debe compilar sin errores
- ✅ Sin warnings
- ✅ Todos los comandos funcionando
- ✅ main.rs reducido de ~850 líneas a ~80 líneas

**Probar todos los comandos**:
```bash
cargo run -- hello
cargo run -- version
cargo run -- list
cargo run -- list -al
cargo run -- set work /tmp
cargo run -- get work
cargo run -- work map
cargo run -- work list
cargo run -- clean-temp --dry-run
```

```bash
git add .
git commit -m "feat: phase 7 - refactor main.rs to use modular architecture"
```
