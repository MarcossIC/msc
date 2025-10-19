# MSC CLI

A modular command-line interface tool for managing workspaces and system utilities.

## Features

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

#### Temporary File Cleaning
```bash
# Dry run (preview what would be deleted)
msc clean-temp --dry-run

# Actually clean temporary files (requires admin/sudo)
msc clean-temp
```

## Architecture

The project follows a clean, modular architecture with clear separation of concerns:

```
src/
├── main.rs          # Entry point (~80 lines)
├── lib.rs           # Public API for reusability
├── error.rs         # Custom error types
├── commands/        # CLI command handlers
├── core/            # Business logic
├── ui/              # User interface components
├── platform/        # OS-specific code
├── git/             # Git integration
└── utils/           # Shared utilities
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

