# MSC CLI

[![Release](https://img.shields.io/github/v/release/marco/msc?style=flat-square)](https://github.com/marco/msc/releases)
[![Downloads](https://img.shields.io/github/downloads/marco/msc/total?style=flat-square)](https://github.com/marco/msc/releases)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg?style=flat-square)](https://opensource.org/licenses/MIT)
[![CI](https://img.shields.io/github/actions/workflow/status/marco/msc/release.yml?style=flat-square&label=CI)](https://github.com/marco/msc/actions)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange?style=flat-square)](https://www.rust-lang.org)

Multi-purpose command-line interface tool for system monitoring, media management, and productivity.

[Installation](#installation) | [Features](#features) | [Documentation](docs/) | [Contributing](#contributing)

## Features

- 🖥️ **System Monitoring** - Real-time TUI dashboard with CPU, GPU, memory, network, and disk metrics
- 📊 **Hardware Information** - Detailed system specifications (CPU, GPU, RAM, motherboard, storage, battery)
- 📹 **Video Downloading** - Download videos from 1000+ platforms (YouTube, Vimeo, TikTok, Twitch, etc.)
- ✂️ **Video Editing** - Compress and convert videos with FFmpeg integration
- 🌐 **Website Archiving** - Mirror websites for offline viewing with link conversion
- 🧹 **System Cleanup** - Safe temporary file removal with age-based filtering and safety validations
- ⚡ **Global Aliases** - Create command shortcuts accessible anywhere (~369KB per alias)
- 📁 **Workspace Management** - Configure and manage project directories
- 🎨 **Git Integration** - Color-coded file status and gitignore support

## Installation

### Windows

#### Via winget (Recommended - Coming Soon)
```powershell
winget install Marco.MSC
```

#### Via MSI Installer
Download the latest `.msi` installer from [releases](https://github.com/marco/msc/releases) and run it. The installer will automatically add `msc` to your PATH.

#### Via PowerShell Script
```powershell
irm https://github.com/marco/msc/releases/latest/download/msc-installer.ps1 | iex
```

### macOS

#### Via Homebrew (Coming Soon)
```bash
brew tap marco/msc
brew install msc
```

#### Via Install Script
```bash
curl -sSL https://github.com/marco/msc/releases/latest/download/msc-installer.sh | sh
```

### Linux

#### Arch Linux (AUR - Coming Soon)
```bash
yay -S msc-bin
# or
paru -S msc-bin
```

#### Via Install Script (Universal)
```bash
curl -sSL https://github.com/marco/msc/releases/latest/download/msc-installer.sh | sh
```

#### Manual Installation
```bash
# Download the appropriate tarball for your architecture
# x86_64:
wget https://github.com/marco/msc/releases/latest/download/msc-x86_64-unknown-linux-gnu.tar.xz

# Extract and install
tar -xf msc-x86_64-unknown-linux-gnu.tar.xz
sudo mv msc /usr/local/bin/
```

### From Source

Requires Rust toolchain (install from [rustup.rs](https://rustup.rs))

```bash
git clone https://github.com/marco/msc.git
cd msc
cargo build --release

# Binary will be at: target/release/msc (or msc.exe on Windows)
```

### Updating

MSC includes a built-in self-update feature:

```bash
msc update
```

**Note:** On Windows, you may need to run your terminal as Administrator to update if MSC was installed to `Program Files`.

### System Requirements

- **Windows**: Windows 10/11 (x64)
- **macOS**: macOS 11+ (Intel and Apple Silicon)
- **Linux**: Any modern distribution (x64 or ARM64)
- **Disk Space**: ~20-30 MB
- **Optional Dependencies**:
  - FFmpeg - For video editing features
  - yt-dlp - For video downloading features (auto-downloaded by MSC if needed)
  - wget - For website archiving features (auto-downloaded by MSC if needed)

## Quick Start

### System Monitoring

```bash
# Show detailed hardware information
msc sys info

# Filter specific components
msc sys info --cpu
msc sys info --gpu
msc sys info --ram
msc sys info --energy

# Real-time monitoring dashboard (TUI)
msc sys monitor
```

### Video Downloading

```bash
# Download a video
msc vget "https://www.youtube.com/watch?v=..."

# Download with quality selection
msc vget "URL" --quality 1080p

# Download playlist
msc vget "playlist-url" --playlist

# Download with browser cookies (for private content)
msc vget "URL" --cookies chrome
```

### Website Archiving

```bash
# Download a website for offline viewing
msc wget "https://example.com"

# Download recursively (mirror entire site)
msc wget "https://example.com" -r

# Download with custom depth
msc wget "https://example.com" -r --depth 3
```

### System Cleanup

```bash
# Preview what would be cleaned (dry run)
msc clean start --dry-run

# Clean temporary files (default: 24 hours old)
msc clean start

# Clean with specific age threshold (48 hours)
msc clean start --min-age 48

# Clean work cache (node_modules, target, dist)
msc clean start --work-cache

# List all paths that will be cleaned
msc clean list
```

### Global Alias System

```bash
# Initialize the alias system
msc alias init

# Create a new alias
msc alias add gs "git status"
msc alias add cb "cargo build --release"
msc alias add pyh "python -m http.server 5000"

# List all aliases
msc alias list

# Remove an alias
msc alias remove gs

# After creating aliases, use them directly:
gs      # Runs: git status
cb      # Runs: cargo build --release
pyh     # Runs: python -m http.server 5000
```

## Updating

MSC includes a built-in self-update feature (coming soon):

```bash
msc update
```

This will check for the latest version and update automatically.

**Windows Note:** You may need to run your terminal as Administrator to update if MSC is installed in Program Files.

Alternatively, download the latest installer from [releases](https://github.com/marco/msc/releases).

## Configuration

MSC stores configuration in:
- **Windows**: `%APPDATA%\msc\`
- **Aliases**: `%APPDATA%\msc\aliases\`

### Setting Directories

```bash
# Set workspace directory
msc set work C:\Users\YourName\Projects

# Set video downloads directory
msc set video C:\Users\YourName\Videos

# Set web downloads directory
msc set web C:\Users\YourName\Downloads\Web
```

## All Commands

```
msc <COMMAND>

Commands:
  hello       Say hello
  version     Show version information
  list        List files and directories
  set         Set configuration values
  get         Get configuration values
  work        Workspace management
  alias       Global alias management
  clean       Cleanup temporary files
  vget        Download videos from online platforms
  vedit       Edit and compress videos
  wget        Download websites for offline viewing
  sys         System information and monitoring
  help        Print this message or the help of the given subcommand(s)
```

## Advanced Usage

### Video Editing

```bash
# Compress video with quality preset
msc vedit comp high video.mp4
msc vedit comp medium video.mp4
msc vedit comp low video.mp4

# Supported formats: mp4, mkv, webm, avi, mov, wmv, flv, m4v
```

### Workspace Management

```bash
# Map workspace structure
msc work map

# List workspace contents
msc work list
```

### Browser Cookie Extraction

```bash
# Extract cookies for wget/vget (for authenticated downloads)
# Supports: Chrome, Edge, Firefox, Brave, LibreWolf
msc wget cookies --browser chrome
```

## Architecture

The project follows a modular architecture:

```
msc/
├── src/
│   ├── commands/        # CLI command handlers
│   ├── core/           # Business logic
│   │   ├── system_info/      # Hardware information collection
│   │   ├── system_monitor/   # Real-time monitoring
│   │   ├── wget/             # Website downloading
│   │   ├── alias.rs          # Alias management
│   │   ├── cleaner.rs        # Cleanup engine
│   │   └── ...
│   ├── ui/             # User interface components
│   ├── platform/       # OS-specific code
│   └── ...
└── msc-shim/          # Lightweight alias executables
```

## Development

### Building from Source

```bash
# Clone repository
git clone https://github.com/marco/msc.git
cd msc

# Build debug version
cargo build

# Build optimized release version
cargo build --release

# Run tests
cargo test

# Run locally
cargo run -- --help
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

## Troubleshooting

### Windows Issues

**"msc is not recognized as an internal or external command"**
- MSC is not in your PATH. Reinstall using the MSI installer, or add manually:
  1. Press Win + X → System → Advanced system settings
  2. Environment Variables → Path → Edit
  3. Add the directory containing msc.exe

**"Access Denied" during cleanup**
- Some system directories require Administrator privileges
- Run terminal as Administrator for system-wide cleanup

**Video download fails**
- Ensure yt-dlp is installed: `winget install yt-dlp` or download from https://github.com/yt-dlp/yt-dlp

**FFmpeg not found**
- Install FFmpeg: `winget install ffmpeg` or download from https://ffmpeg.org/download.html

### Getting Help

```bash
# General help
msc --help

# Command-specific help
msc sys --help
msc vget --help
msc clean --help
```

## Security & Safety

- **Path Validation**: Cleanup operations validate paths to prevent dangerous deletions
- **Age-Based Filtering**: Default 24-hour minimum age for file deletion
- **Dry-Run Mode**: Preview changes before executing
- **Ignore Lists**: Configurable exclusion patterns
- **Admin Escalation**: Prompts when elevated privileges needed

## Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Make your changes
4. Run tests (`cargo test`)
5. Commit your changes (`git commit -m 'Add amazing feature'`)
6. Push to the branch (`git push origin feature/amazing-feature`)
7. Open a Pull Request

## License

MIT License - see [LICENSE](LICENSE) file for details.

## Acknowledgments

- Uses [yt-dlp](https://github.com/yt-dlp/yt-dlp) for video downloading
- Uses [FFmpeg](https://ffmpeg.org/) for video processing
- Built with [Rust](https://www.rust-lang.org/)
- TUI powered by [ratatui](https://github.com/ratatui-org/ratatui)
- System information via [sysinfo](https://github.com/GuillaumeGomez/sysinfo)

## Support

- 🐛 [Report bugs](https://github.com/marco/msc/issues)
- 💡 [Request features](https://github.com/marco/msc/issues)
- 📖 [View documentation](https://github.com/marco/msc/blob/main/README.md)

---

**Made with ❤️ by Marco**
