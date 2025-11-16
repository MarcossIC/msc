# MSC CLI

A modular command-line interface tool for managing workspaces and system utilities.

## Quick Start

### Prerequisites
- [Rust](https://www.rust-lang.org/tools/install) (1.70 or higher)

En caso de tener problemas con dlltool
- [MSY](https://www.msys2.org)

Ejecutar en MSYS2 MinGW

```bash
pacman -Syu
pacman -S mingw-w64-x86_64-toolchain
```bash

### Build and Run

```bash
# Clone the repository
git clone https://github.com/marco/msc.git
cd msc

# Build the project
cargo build

# Run locally commands (development)
cargo run -- --help
cargo run -- list
cargo run -- work map

# Build optimized release version
cargo build --release

# Install locally
cargo install --path .

# After installation, use directly
msc --help
```

## Features

- **Global Alias System**: Create global command aliases accessible from anywhere
- **Workspace Management**: Configure and manage your work directories
- **File Listing**: Advanced file scanning with Git integration and icons
- **Temporary File Cleaning**: Clean system temporary directories safely
- **Git Integration**: Color-coded file status based on Git state
- **Cross-platform**: Works on Windows and Unix-like systems

## Installation

```bash
cargo install --path .
```

## Usage

### General Help

```bash
msc --help
```

### Commands

#### Hello Command
```bash
msc hello --name Marco
```

#### Version Information
```bash
msc version
```

#### Configuration Management
```bash
# Set workspace directory
msc set work /path/to/workspace

# Get current workspace
msc get work
```

#### Workspace Operations
```bash
# Map workspace structure
msc work map

# List workspace contents
msc work list
```

#### File Listing
```bash
# List files in current directory
msc list

# List with details
msc list --long
```

#### Global Alias System
```bash
# Initialize the alias system (add to PATH)
msc alias init

# Create a new alias
msc alias add pyh "python3 -m http.server 5000"

# Create an alias with description
msc alias add gp "git push" -d "Quick git push"

# List all aliases
msc alias list

# Remove an alias
msc alias remove pyh

# After creating aliases, use them directly:
pyh              # Runs: python3 -m http.server 5000
gp               # Runs: git push
```

#### Temporary File Cleaning
```bash
# List all paths that will be cleaned
msc clean list

# Dry run (preview what would be deleted)
msc clean start --dry-run

# Clean temporary files
msc clean start

# Clean with specific age threshold
msc clean start --min-age 48

# Include Recycle Bin
msc clean start --IR

# Clean work cache (target, dist, node_modules)
msc clean start --work-cache
```

## Alias System

The alias system allows you to create global command shortcuts that work from anywhere in your terminal.

### How It Works

1. **Windows**: Creates lightweight executable shims (~369KB each) that forward commands
2. **Unix**: Creates shell scripts that execute your commands
3. **Configuration**: Stored in `~/.config/msc/aliases/aliases.json` (cross-platform)
4. **Executables**: Stored in `~/.config/msc/aliases/bin/`

### Quick Start

```bash
# 1. Initialize (adds bin directory to PATH)
msc alias init

# 2. Create your first alias
msc alias add gs "git status"

# 3. Restart your terminal or source your shell config

# 4. Use it!
gs    # Runs: git status
```

### Common Use Cases

```bash
# Development shortcuts
msc alias add cb "cargo build --release"
msc alias add ct "cargo test"

# Git shortcuts
msc alias add gp "git push"
msc alias add gl "git log --oneline --graph"
msc alias add gs "git status"

# Python development
msc alias add pyh "python3 -m http.server 5000"
msc alias add venv "python3 -m venv venv"

# System utilities
msc alias add update "sudo apt update && sudo apt upgrade"
msc alias add ports "netstat -tuln"
```

### Platform-Specific Details

#### Windows
- Adds `%APPDATA%\msc\aliases\bin` to user PATH via registry
- May require terminal restart
- Supports command arguments: `pyh --help` works as expected

#### Unix (Linux/macOS)
- Adds `~/.config/msc/aliases/bin` to shell config (.bashrc, .zshrc, or config.fish)
- Run `source ~/.bashrc` (or equivalent) to apply immediately
- Supports all shell features

## Architecture

The project follows a clean, modular architecture with clear separation of concerns:

```
msc/
├── msc-shim/            # Lightweight shim executable for aliases
│   └── src/
│       └── main.rs      # Shim implementation (~140 lines)
├── src/
│   ├── main.rs          # Entry point (~80 lines)
│   ├── lib.rs           # Public API for reusability
│   ├── error.rs         # Custom error types
│   ├── commands/        # CLI command handlers
│   │   └── alias.rs     # Alias command implementation
│   ├── core/            # Business logic
│   │   ├── alias.rs     # Alias data model
│   │   ├── alias_generator.rs  # Platform-specific generators
│   │   └── path_manager.rs     # PATH management
│   ├── ui/              # User interface components
│   ├── platform/        # OS-specific code
│   ├── git/             # Git integration
│   └── utils/           # Shared utilities
└── tests/               # Comprehensive test suite
```

### Module Responsibilities

- **`commands/`** - CLI command handlers (application layer)
- **`core/`** - Pure business logic, independent of UI
- **`ui/`** - Presentation layer (formatting, progress bars, prompts)
- **`platform/`** - OS-specific interactions (Windows/Unix abstraction)
- **`git/`** - Git repository integration
- **`utils/`** - Shared utilities and helpers

## Development

### Building

```bash
cargo build
```

### Running Tests

```bash
cargo test
```

### Running Locally

```bash
cargo run -- list
cargo run -- work map
```

### Code Quality

```bash
# Run linter
cargo clippy --all-targets --all-features

# Format code
cargo fmt --all

# Generate documentation
cargo doc --open
```

### Build Release Version

```bash
cargo build --release
./target/release/msc --version
```

## Testing

The project includes comprehensive testing infrastructure:

- **Unit Tests**: In each module with `#[cfg(test)]`
- **Integration Tests**: In `tests/integration/`
- **Test Fixtures**: Sample data in `tests/fixtures/`

## Dependencies

Key dependencies:

- **clap**: Command-line argument parsing
- **serde**: Serialization/deserialization
- **git2**: Git integration
- **colored**: Terminal colors
- **anyhow/thiserror**: Error handling
- **log/env_logger**: Logging infrastructure

## Project Status

This project has undergone a complete architectural migration from a monolithic `main.rs` (~850 lines) to a modular, well-organized structure with 20+ modules.

### Migration Metrics

- **Code Reduction**: main.rs reduced from ~850 lines to ~80 lines (90% reduction)
- **Modules Created**: 20+ well-organized files
- **Test Coverage**: Comprehensive test infrastructure
- **Maintainability**: High - easy to understand and extend
- **Scalability**: Excellent - simple to add new commands

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Run tests and linting
5. Submit a pull request

## License

MIT

