# Introduction

Welcome to the **MSC CLI** documentation!

MSC is a multi-purpose command-line interface tool that combines system monitoring, media management, website archiving, and productivity utilities into a single, powerful application.

## What is MSC?

MSC CLI is designed to be your Swiss Army knife for common system administration, content creation, and development tasks. Instead of installing and managing multiple CLI tools, MSC provides a unified interface for:

- **System Monitoring**: Real-time hardware metrics and resource usage
- **Media Management**: Video downloading and editing
- **Web Archiving**: Website mirroring for offline viewing
- **System Cleanup**: Safe temporary file removal
- **Productivity**: Workspace management and global aliases

## Key Features

### 🖥️ System Monitoring
Monitor your system in real-time with a beautiful TUI dashboard showing CPU, GPU, memory, network, and disk metrics.

### 📹 Video Management
Download videos from 1000+ platforms (YouTube, Vimeo, TikTok) and edit them with integrated FFmpeg support.

### 🌐 Web Archiving
Mirror entire websites for offline viewing with intelligent link conversion and resource optimization.

### 🧹 System Cleanup
Safely clean temporary files, build artifacts, and caches with age-based filtering and safety validations.

### ⚡ Global Aliases
Create command shortcuts that work from anywhere in your system.

## Why MSC?

- **All-in-One**: Multiple tools unified under one interface
- **Cross-Platform**: Works on Windows, macOS, and Linux
- **Fast**: Written in Rust for maximum performance
- **Safe**: Built-in security validations and safety checks
- **Modern**: Beautiful TUI interfaces and intuitive commands

## Quick Example

```bash
# Monitor system resources in real-time
msc sys monitor

# Download a video
msc vget "https://youtube.com/watch?v=..."

# Clean temporary files older than 7 days
msc clean start --days 7 --dry-run

# Create a global alias
msc alias add dev "cd ~/projects && code ."
```

## Getting Started

Ready to dive in? Head over to the [Installation](./installation.md) guide to get MSC up and running on your system.

## Community and Support

- **GitHub Repository**: [github.com/marco/msc](https://github.com/marco/msc)
- **Issue Tracker**: [Report bugs or request features](https://github.com/marco/msc/issues)
- **Discussions**: [Ask questions and share ideas](https://github.com/marco/msc/discussions)

## License

MSC is open source software licensed under the [MIT License](https://opensource.org/licenses/MIT).
