# PowerShell script to generate shell completions for MSC CLI

Write-Host "Generating shell completions for MSC..." -ForegroundColor Blue

# Build the project first
Write-Host "Building msc..." -ForegroundColor Cyan
cargo build --release

if ($LASTEXITCODE -ne 0) {
    Write-Host "Build failed!" -ForegroundColor Red
    exit 1
}

# Create completions directory if it doesn't exist
New-Item -ItemType Directory -Force -Path "completions" | Out-Null

# Binary path
$MSC_BIN = ".\target\release\msc.exe"

# Generate completions for each shell
Write-Host "`nGenerating Bash completion..." -ForegroundColor Green
& $MSC_BIN completions bash | Out-File -FilePath "completions\msc.bash" -Encoding utf8

Write-Host "Generating Zsh completion..." -ForegroundColor Green
& $MSC_BIN completions zsh | Out-File -FilePath "completions\_msc" -Encoding utf8

Write-Host "Generating Fish completion..." -ForegroundColor Green
& $MSC_BIN completions fish | Out-File -FilePath "completions\msc.fish" -Encoding utf8

Write-Host "Generating PowerShell completion..." -ForegroundColor Green
& $MSC_BIN completions powershell | Out-File -FilePath "completions\_msc.ps1" -Encoding utf8

Write-Host "Generating Elvish completion..." -ForegroundColor Green
& $MSC_BIN completions elvish | Out-File -FilePath "completions\msc.elv" -Encoding utf8

Write-Host "`n✓ Completions generated successfully in .\completions\" -ForegroundColor Blue

Write-Host "`nTo install PowerShell completion:" -ForegroundColor Yellow
Write-Host "  1. Import-Module .\completions\_msc.ps1"
Write-Host "  2. Or add to your PowerShell profile:"
Write-Host "     . `$PSScriptRoot\completions\_msc.ps1"
