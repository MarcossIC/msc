# MSC CLI

[![Release](https://img.shields.io/github/v/release/MarcossIC/msc?style=flat-square)](https://github.com/MarcossIC/msc/releases)
[![Downloads](https://img.shields.io/github/downloads/MarcossIC/msc/total?style=flat-square)](https://github.com/MarcossIC/msc/releases)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg?style=flat-square)](https://opensource.org/licenses/MIT)
[![CI](https://img.shields.io/github/actions/workflow/status/MarcossIC/msc/release.yml?style=flat-square&label=CI)](https://github.com/MarcossIC/msc/actions)
[![Rust](https://img.shields.io/badge/rust-stable-orange?style=flat-square)](https://www.rust-lang.org)

Multi-purpose command-line interface tool for system monitoring, media management, and productivity.

[Installation](#installation) | [Features](#features) | [Commands](#all-commands) | [Documentation](docs/) | [Contributing](#contributing)

## Features

- 🖥️ **Hardware Information** — Deep system report: CPU (microarch from CPUID, microcode, stepping, temperature), GPU, RAM, motherboard + BIOS/UEFI, storage, monitors (EDID), network, OS, battery
- 💽 **Drive Health** — NVMe SMART via native IOCTL: wear level, TBW, power-on hours, temperature — no admin required
- 🔐 **Security Posture** — TPM version (TBS), Secure Boot state, HVCI, virtualization support
- 📊 **JSON Output** — `sys info --json` for scripting and dashboards
- 📈 **Real-Time Monitoring** — TUI dashboard with CPU, GPU, memory, network, disk, and top processes
- 📹 **Video Downloading** — 1000+ platforms via yt-dlp (YouTube, Vimeo, TikTok, Twitch…), with browser cookie import
- ✂️ **Video Compression** — FFmpeg presets with before/after stats
- 🌐 **Website Archiving** — Mirror sites for offline viewing with regex URL filtering and link rewriting
- 🧹 **System Cleanup** — Age-based temp removal with dry-run, path validation, and work-cache mode
- ⚡ **Global Aliases** — Command shortcuts available anywhere in your shell *(Windows)*
- 📁 **Workspace Management** — Register and map project directories

### Platform support

| Area | Windows | Linux / macOS |
| ---- | ------- | ------------- |
| `sys info` | Full detail (WMI, registry, IOCTL, TBS, EDID) | Reduced — core CPU/RAM/OS/disk only |
| `sys monitor` | ✅ | ✅ |
| `vget` / `vedit` / `wget` | ✅ | ✅ |
| `clean` / `work` / `set` / `get` / `list` | ✅ | ✅ |
| `alias` | ✅ | ❌ (shim is a Windows executable) |

## Installation

### Windows

**MSI Installer** — download the latest `.msi` from [releases](https://github.com/MarcossIC/msc/releases) and run it. It adds `msc` to your PATH automatically.

**PowerShell**
```powershell
irm https://github.com/MarcossIC/msc/releases/latest/download/msc-installer.ps1 | iex
```

### macOS / Linux

```bash
curl -sSL https://github.com/MarcossIC/msc/releases/latest/download/msc-installer.sh | sh
```

**Manual (Linux)**
```bash
wget https://github.com/MarcossIC/msc/releases/latest/download/msc-x86_64-unknown-linux-gnu.tar.xz
tar -xf msc-x86_64-unknown-linux-gnu.tar.xz
sudo mv msc /usr/local/bin/
```

### From Source

Requires a recent stable Rust toolchain ([rustup.rs](https://rustup.rs)).

```bash
git clone https://github.com/MarcossIC/msc.git
cd msc
cargo build --release
# Binary at: target/release/msc (msc.exe on Windows)
```

### Updating

```bash
msc update
```

Downloads, verifies, and installs the latest GitHub release in place. On Windows, run the terminal as Administrator if MSC lives in `Program Files`.

### System Requirements

- **Windows** 10/11 (x64) · **macOS** 11+ (Intel & Apple Silicon) · **Linux** (x64 / ARM64)
- **Disk**: ~20–30 MB
- **Optional**: FFmpeg (`vedit`), yt-dlp (`vget`), wget (`wget`) — yt-dlp and FFmpeg are auto-installed on first use

## Quick Start

### System Information

```bash
msc sys info                    # Full hardware report
msc sys info --cpu --gpu        # Filter: combine flags freely
msc sys info --ram --mbo
msc sys info --os --energy --network

msc sys info --json             # Machine-readable output
msc sys info --json --compact   # Single-line JSON

msc sys info --wan              # Opt-in: public IP + internet latency (~370ms)
msc sys info --profile          # Per-section timing breakdown
msc sys info --no-cache         # Bypass the disk cache
msc sys info --clear-cache      # Wipe the cache, then run fresh
```

`--wan` is off by default because it costs a network round-trip. Skipped fields are omitted, never reported as failures.

### Real-Time Monitor

```bash
msc sys monitor                     # Full TUI dashboard
msc sys monitor -i 500              # 500ms refresh interval
msc sys monitor --cpu-only          # Or --gpu-only / --memory-only
msc sys monitor --network --disks   # Force these panels on
msc sys monitor -p 20               # Top 20 processes
msc sys monitor --json              # Non-interactive JSON stream
```

### Video Downloading

```bash
msc vget "https://youtube.com/watch?v=..."
msc vget "URL" -q 1080p                  # 2160p|1080p|720p|480p|360p|best
msc vget "URL" -f mkv                    # mp4|mkv|webm|avi
msc vget "URL" -o my_video               # Custom filename
msc vget "URL" --audio-only
msc vget "URL" --playlist                # Or --no-playlist to force single
msc vget "URL" --clean-parts             # Remove orphaned .part files first
msc vget "URL" --no-continue             # Restart from scratch

# Authenticated content
msc vget "URL" --cb                      # Chrome cookies (default)
msc vget "URL" --cb firefox              # firefox|chrome:Default|edge|safari|brave
msc vget "URL" --cookies cookies.txt     # Netscape-format cookie file
```

`--cb` reads cookies from an installed browser. `--cookies` loads them from a file — they are not interchangeable.

### Website Archiving

```bash
msc wget "https://example.com"                    # Single page + resources
msc wget "https://example.com" my-site            # Into a named folder
msc wget "https://example.com" --all              # Mirror the whole site

# Filtering the crawl
msc wget "https://blog.com" --all --pattern '/posts/.*'
msc wget "https://blog.com" --all --exclude '/feed/'
msc wget "https://blog.com" --all --pattern '/posts/.*' --exclude '#comment'
msc wget "https://blog.com" --all --limit 150     # Cap total pages

# Cookies
msc wget "URL" --cookies 'session=abc123; age_verified=1'
msc wget cookies https://example.com --browser chrome --format json
msc wget cookies https://instagram.com --cdp      # Chrome 127+ App-Bound Encryption

# Re-run link rewriting without re-downloading
msc wget postprocessing ./my-site -u https://example.com
```

### System Cleanup

```bash
msc clean start --dry-run          # Always preview first
msc clean start                    # Delete temp files older than 24h
msc clean start --min-age 48       # Custom age threshold
msc clean start --include-recycle  # Or --IR
msc clean start --work-cache       # Or -WC: node_modules, target, dist in workspaces
msc clean start --include-recent   # ⚠️ Ignores age filter entirely

msc clean list                     # Show every configured path
msc clean add C:\MyTempFolder      # Add a custom path (validated)
msc clean remove                   # Interactive removal
msc clean reset                    # Back to defaults

msc clean ignore list              # Folders skipped by --work-cache
msc clean ignore add my-project
msc clean ignore remove my-project
```

### Global Aliases *(Windows)*

```bash
msc alias init                                  # Register the alias dir in PATH
msc alias add gs "git status"
msc alias add cb "cargo build --release" -d "Release build"
msc alias list
msc alias remove gs
msc alias nuke                                  # ⚠️ Remove the entire alias system

# Then use them directly:
gs
cb
```

Each alias is a standalone shim executable, so it works from any shell — cmd, PowerShell, or Git Bash.

### Files & Workspaces

```bash
msc list                    # List current directory (git-aware colors)
msc list -a                 # Include hidden files
msc list -l                 # Long/table format
msc list -d --depth 3       # Recursive, max depth 3

msc work map                # Register project folders as workspaces
msc work list
```

## Configuration

Config lives at `<config-dir>/msc/`:

| OS | Path |
| -- | ---- |
| Windows | `%APPDATA%\msc\` |
| Linux | `~/.config/msc/` |
| macOS | `~/Library/Application Support/msc/` |

Aliases go in the `aliases/` subdirectory.

```bash
msc set work   C:\Users\You\Projects    # Workspace root
msc set video  C:\Users\You\Videos      # vget destination
msc set web    C:\Users\You\Web         # wget destination

msc get work
msc get video
msc get web
```

## All Commands

```
msc <COMMAND>

Commands:
  sys       System information and monitoring (info, monitor)
  vget      Download videos from 1000+ platforms
  vedit     Compress and convert videos
  wget      Download websites for offline viewing
  clean     Cleanup temporary files and caches
  alias     Global alias management (Windows)
  work      Workspace management
  list      List files and directories
  set       Set configuration values
  get       Get configuration values
  update    Update MSC to the latest version
  version   Show version information
  hello     Say hello
  help      Print help for any command
```

Shell completions are also available (hidden from help):

```bash
msc completions bash        # bash|zsh|fish|powershell|elvish
```

## Video Compression

```bash
msc vedit comp low    video.mp4    # CRF 28, fast preset, 96k audio
msc vedit comp medium video.mp4    # CRF 23, medium preset, 128k audio
msc vedit comp high   video.mp4    # CRF 18, slow preset, 192k audio
```

Output is written alongside the source with `_compress` appended. Supported: mp4, mkv, webm, avi, mov, wmv, flv, m4v.

## Architecture

```
msc/
├── src/
│   ├── commands/            # CLI handlers (one per subcommand)
│   ├── core/                # Business logic
│   │   ├── system_info/     # Hardware collection (parallel, cached, profiled)
│   │   ├── system_monitor/  # Real-time metrics
│   │   ├── wget/            # Crawler + link post-processing
│   │   ├── update/          # Self-update
│   │   ├── alias.rs         # Alias management
│   │   └── cleaner.rs       # Cleanup engine
│   ├── platform/            # OS-specific code
│   │   ├── system/windows/  # WMI, registry, IOCTL, TBS, EDID readers
│   │   └── gpu/             # NVIDIA (NVML) and AMD backends
│   ├── ui/                  # Rendering and TUI components
│   └── git/                 # Git status integration
└── msc-shim/                # Lightweight alias executable
```

Design rule in `system_info`: **pure parsers are unit-tested against golden bytes; `unsafe` FFI fetchers are isolated and thin.** See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Development

```bash
cargo build                                  # Debug
cargo build --release                        # Optimized
cargo test                                   # Test suite
cargo test -- --ignored                      # Hardware dumps (real device required)
cargo run -- sys info                        # Run locally

cargo clippy --all-targets --all-features    # Lint
cargo fmt --all                              # Format
cargo doc --open                             # Docs
```

### Feature flags

| Flag | Default | Purpose |
| ---- | ------- | ------- |
| `nvml` | ✅ | NVIDIA GPU metrics via NVML |
| `rocm` | ❌ | AMD GPU metrics via ROCm SMI (Linux) |

## Troubleshooting

**`msc` is not recognized (Windows)**
MSC isn't in your PATH. Reinstall via the MSI, or add the directory containing `msc.exe` manually: Win + X → System → Advanced system settings → Environment Variables → Path.

**"Access Denied" during cleanup**
Some system directories need elevation. MSC runs user directories first, then prompts for admin. Run the terminal as Administrator for a full system pass.

**Video download fails**
MSC auto-installs yt-dlp, but you can install it yourself: `winget install yt-dlp`.

**FFmpeg not found**
`winget install ffmpeg`, or grab it from [ffmpeg.org](https://ffmpeg.org/download.html).

**`sys info` shows fewer fields on Linux/macOS**
Expected. The deep readers (WMI, registry, NVMe IOCTL, TPM, EDID) are Windows-only. Non-Windows falls back to `sysinfo` for core data.

**Cookie extraction fails on Chrome 127+**
Chrome added App-Bound Encryption. Use `msc wget cookies URL --cdp` with Chrome running as `chrome.exe --remote-debugging-port=9222`, or add `--auto-launch`.

### Getting Help

```bash
msc --help
msc sys --help
msc sys info --help
msc clean start --help
```

Every subcommand ships a detailed `--help` with examples.

## Security & Safety

- **Path validation** — protected system directories cannot be added to clean paths
- **Age-based filtering** — 24-hour minimum by default; `--include-recent` is opt-in and warned about
- **Dry-run mode** — preview every deletion before it happens
- **Two-phase cleanup** — user directories first, system directories only after explicit elevation
- **Ignore lists** — configurable exclusions for work-cache cleanup
- **No admin for diagnostics** — SMART, TPM, and EDID readers run unprivileged by design

See [docs/security.md](docs/security.md).

## Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Make your changes
4. Run `cargo test` and `cargo clippy --all-targets`
5. Commit using [conventional commits](https://www.conventionalcommits.org/) (`git commit -m 'feat: add amazing feature'`)
6. Push and open a Pull Request

## License

MIT — see [LICENSE](LICENSE).

## Acknowledgments

Built with [Rust](https://www.rust-lang.org/) · TUI by [ratatui](https://github.com/ratatui-org/ratatui) · CLI by [clap](https://github.com/clap-rs/clap) · base metrics from [sysinfo](https://github.com/GuillaumeGomez/sysinfo) · downloads via [yt-dlp](https://github.com/yt-dlp/yt-dlp) and [FFmpeg](https://ffmpeg.org/)

## Support

- 🐛 [Report bugs](https://github.com/MarcossIC/msc/issues)
- 💡 [Request features](https://github.com/MarcossIC/msc/issues)
- 📖 [Documentation](docs/)

---

**Made with ❤️ by Marcos**
