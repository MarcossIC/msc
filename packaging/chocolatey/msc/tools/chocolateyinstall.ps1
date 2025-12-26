$ErrorActionPreference = 'Stop'

$packageName = $env:ChocolateyPackageName
$version = $env:ChocolateyPackageVersion
$url64 = "https://github.com/MarcossIC/msc/releases/download/v$version/msc-x86_64-pc-windows-msvc.msi"

# Checksum will be automatically updated by the release workflow
$checksum64 = 'CHECKSUM_PLACEHOLDER'

$packageArgs = @{
  packageName    = $packageName
  fileType       = 'MSI'
  url64bit       = $url64
  checksum64     = $checksum64
  checksumType64 = 'sha256'
  silentArgs     = "/qn /norestart /l*v `"$($env:TEMP)\$packageName.$version.MsiInstall.log`""
  validExitCodes = @(0, 3010, 1641)
  softwareName   = 'MSC*'
}

Install-ChocolateyPackage @packageArgs
