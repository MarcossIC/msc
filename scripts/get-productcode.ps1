# Script to extract ProductCode from MSI installer
# Usage: .\scripts\get-productcode.ps1 -Version "0.1.7"

param(
    [Parameter(Mandatory=$false)]
    [string]$Version,

    [Parameter(Mandatory=$false)]
    [string]$MsiPath
)

# If no version specified, get from Cargo.toml
if (-not $Version) {
    $cargoToml = Get-Content "Cargo.toml" -Raw
    if ($cargoToml -match 'version = "([^"]+)"') {
        $Version = $matches[1]
        Write-Host "📦 Version detected from Cargo.toml: $Version" -ForegroundColor Cyan
    } else {
        Write-Host "❌ Could not detect version from Cargo.toml" -ForegroundColor Red
        exit 1
    }
}

# Determine MSI path
if (-not $MsiPath) {
    $MsiPath = "msc-$Version-x86_64-pc-windows-msvc.msi"

    # Try to find in target/distrib
    $targetMsi = "target\distrib\$MsiPath"
    if (Test-Path $targetMsi) {
        $MsiPath = $targetMsi
        Write-Host "✅ Found MSI in target/distrib" -ForegroundColor Green
    } else {
        # Download from GitHub release
        $downloadUrl = "https://github.com/MarcossIC/msc/releases/download/v$Version/$MsiPath"
        Write-Host "📥 Downloading MSI from: $downloadUrl" -ForegroundColor Yellow

        try {
            Invoke-WebRequest -Uri $downloadUrl -OutFile $MsiPath -ErrorAction Stop
            Write-Host "✅ Downloaded successfully" -ForegroundColor Green
        } catch {
            Write-Host "❌ Failed to download MSI: $_" -ForegroundColor Red
            Write-Host "   URL: $downloadUrl" -ForegroundColor Red
            exit 1
        }
    }
}

if (-not (Test-Path $MsiPath)) {
    Write-Host "❌ MSI file not found: $MsiPath" -ForegroundColor Red
    exit 1
}

Write-Host "`n🔍 Extracting ProductCode from: $MsiPath" -ForegroundColor Cyan

try {
    # Create Windows Installer object
    $installer = New-Object -ComObject WindowsInstaller.Installer
    $database = $installer.GetType().InvokeMember(
        "OpenDatabase",
        "InvokeMethod",
        $null,
        $installer,
        @($MsiPath, 0)
    )

    # Query ProductCode
    $view = $database.GetType().InvokeMember(
        "OpenView",
        "InvokeMethod",
        $null,
        $database,
        ("SELECT Value FROM Property WHERE Property='ProductCode'")
    )

    $view.GetType().InvokeMember("Execute", "InvokeMethod", $null, $view, $null)
    $record = $view.GetType().InvokeMember("Fetch", "InvokeMethod", $null, $view, $null)
    $productCode = $record.GetType().InvokeMember("StringData", "GetProperty", $null, $record, 1)

    Write-Host "`n✅ ProductCode extracted:" -ForegroundColor Green
    Write-Host "   $productCode" -ForegroundColor White

    # Update winget manifest if exists
    $installerManifest = "packaging\winget\MSC.installer.yaml"
    if (Test-Path $installerManifest) {
        Write-Host "`n📝 Updating $installerManifest..." -ForegroundColor Cyan

        $content = Get-Content $installerManifest -Raw
        $content = $content -replace "ProductCode: '\{[^}]+\}'", "ProductCode: '$productCode'"
        $content = $content -replace "ProductCode: '\{REPLACE_WITH_PRODUCT_CODE\}'", "ProductCode: '$productCode'"
        $content | Set-Content $installerManifest -NoNewline

        Write-Host "✅ Manifest updated!" -ForegroundColor Green
        Write-Host "`n📋 Next steps:" -ForegroundColor Yellow
        Write-Host "   1. Verify the manifest: cd packaging\winget && wingetcreate validate ." -ForegroundColor White
        Write-Host "   2. Follow steps in docs\PACKAGE_MANAGERS.md to submit to winget" -ForegroundColor White
    } else {
        Write-Host "`n⚠️  Manifest not found at: $installerManifest" -ForegroundColor Yellow
        Write-Host "   Please update manually with ProductCode: $productCode" -ForegroundColor White
    }

    # Output for GitHub Actions
    if ($env:GITHUB_OUTPUT) {
        "product_code=$productCode" | Out-File -FilePath $env:GITHUB_OUTPUT -Append -Encoding utf8
        Write-Host "`n✅ ProductCode exported to GITHUB_OUTPUT" -ForegroundColor Green
    }

} catch {
    Write-Host "`n❌ Error extracting ProductCode: $_" -ForegroundColor Red
    exit 1
}
