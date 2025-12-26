# Quick Start

This guide will walk you through the essential commands to get started with MSC CLI.

## System Information

View detailed information about your system:

```bash
msc sys info
```

This displays:
- CPU specifications
- GPU information
- Memory (RAM) details
- Storage devices
- Network interfaces
- Battery status (laptops)

## Real-Time Monitoring

Launch the interactive system monitor:

```bash
msc sys monitor
```

Use keyboard shortcuts:
- `q` - Quit
- Arrow keys - Navigate
- `r` - Refresh

## Video Downloads

Download videos from YouTube and 1000+ other platforms:

```bash
# Basic download
msc vget "https://youtube.com/watch?v=..."

# Custom filename
msc vget "https://youtube.com/watch?v=..." -o my_video

# Specific quality
msc vget "URL" -q 720p

# Audio only
msc vget "URL" --audio-only
```

## Video Editing

Compress and convert videos:

```bash
# Compress to 720p
msc vedit input.mp4 -q 720p

# Convert format
msc vedit input.mkv --format mp4

# Custom output name
msc vedit input.mp4 -o output.mp4
```

## Website Archiving

Mirror websites for offline viewing:

```bash
# Basic mirror
msc wget "https://example.com"

# Recursive with depth limit
msc wget "https://example.com" -r -l 2

# Convert links for offline viewing
msc wget "https://example.com" -k -p
```

## System Cleanup

Clean temporary files safely:

```bash
# Preview what would be deleted (dry run)
msc clean start --dry-run

# Clean files older than 7 days
msc clean start --days 7

# Clean specific directory
msc clean add /path/to/cache
msc clean start
```

## Workspace Management

Quickly navigate to project directories:

```bash
# Add current directory as workspace
msc work add myproject

# List workspaces
msc work list

# Jump to workspace
cd $(msc work map myproject)
```

## Global Aliases

Create command shortcuts:

```bash
# Create an alias
msc alias add dev "cd ~/projects && code ."

# List aliases
msc alias list

# Remove alias
msc alias remove dev
```

## Getting Help

Get help for any command:

```bash
# General help
msc --help

# Command-specific help
msc sys --help
msc vget --help
msc clean --help
```

## Next Steps

- Learn about [Configuration](./configuration.md)
- Explore detailed [Command Reference](./commands/sys.md)
- Set up [Shell Completions](./advanced/completions.md)
