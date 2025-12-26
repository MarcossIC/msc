# Installation

MSC CLI can be installed on Windows, macOS, and Linux through various methods.

## Choose Your Platform

- [Windows Installation](./installation/windows.md)
- [macOS Installation](./installation/macos.md)
- [Linux Installation](./installation/linux.md)
- [Build from Source](./installation/source.md)

## Quick Install

### Windows

```powershell
# Via winget (recommended)
winget install Marco.MSC

# Or download MSI installer from releases
```

### macOS / Linux

```bash
# Via Homebrew
brew tap marco/msc
brew install msc

# Or via install script
curl -sSL https://github.com/MarcossIC/msc/releases/latest/download/msc-installer.sh | sh
```

## Verifying Installation

After installation, verify MSC is working:

```bash
msc --version
```

You should see output like:
```
msc 0.1.0
```

## Updating

MSC includes built-in self-update functionality:

```bash
msc update
```

On Windows, you may need to run your terminal as Administrator to update.

## Next Steps

Once installed, check out the [Quick Start](./quickstart.md) guide to learn the basics.
