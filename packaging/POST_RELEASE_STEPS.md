# Post-Release Steps for Package Managers

This document explains how to complete the package manager integration **AFTER** creating your first GitHub release.

## Prerequisites

Before following these steps, you must:

1. ✅ Complete Phase 3 (create release v0.1.0 on GitHub)
2. ✅ Verify release artifacts exist at: `https://github.com/YOUR_USERNAME/msc/releases/tag/v0.1.0`
3. ✅ Have access to the SHA256 checksums

---

## Step 1: Get SHA256 Hashes

After creating the release, cargo-dist will generate a `sha256.sum` file with all checksums.

**Download it from:**
```
https://github.com/YOUR_USERNAME/msc/releases/download/v0.1.0/sha256.sum
```

**Or download individual .sha256 files for each artifact.**

You'll need these hashes:
- `msc-x86_64-pc-windows-msvc.msi.sha256` - Windows MSI
- `msc-x86_64-apple-darwin.tar.xz.sha256` - Intel macOS
- `msc-aarch64-apple-darwin.tar.xz.sha256` - ARM64 macOS (Apple Silicon)
- `msc-x86_64-unknown-linux-gnu.tar.xz.sha256` - x86_64 Linux
- `msc-aarch64-unknown-linux-gnu.tar.xz.sha256` - ARM64 Linux

---

## Step 2: Windows Package Manager (winget)

### 2.1 Get MSI ProductCode

Download the MSI installer and extract its ProductCode:

**On Windows PowerShell:**
```powershell
# Download MSI
Invoke-WebRequest -Uri "https://github.com/YOUR_USERNAME/msc/releases/download/v0.1.0/msc-x86_64-pc-windows-msvc.msi" -OutFile "msc.msi"

# Get ProductCode (method 1)
$installer = "msc.msi"
$windowsInstaller = New-Object -ComObject WindowsInstaller.Installer
$database = $windowsInstaller.GetType().InvokeMember("OpenDatabase", "InvokeMethod", $null, $windowsInstaller, @($installer, 0))
$view = $database.GetType().InvokeMember("OpenView", "InvokeMethod", $null, $database, "SELECT Value FROM Property WHERE Property = 'ProductCode'")
$view.GetType().InvokeMember("Execute", "InvokeMethod", $null, $view, $null)
$record = $view.GetType().InvokeMember("Fetch", "InvokeMethod", $null, $view, $null)
$productCode = $record.GetType().InvokeMember("StringData", "GetProperty", $null, $record, 1)
Write-Host "ProductCode: $productCode"
```

**Or use lessmsi:**
```powershell
choco install lessmsi
lessmsi l msc.msi | Select-String "ProductCode"
```

### 2.2 Update winget manifests

1. Open `packaging/winget/Marco.MSC.installer.yaml`
2. Replace `YOUR_USERNAME` with your GitHub username
3. Replace `REPLACE_WITH_SHA256_FROM_RELEASE` with the MSI SHA256
4. Replace `{REPLACE_WITH_PRODUCT_CODE}` with the ProductCode from above

3. Open `packaging/winget/Marco.MSC.locale.en-US.yaml`
4. Replace `YOUR_USERNAME` with your GitHub username

### 2.3 Validate manifests

```bash
winget validate --manifest packaging/winget/
```

Expected output: `Manifest validation succeeded.`

### 2.4 Submit to winget-pkgs

```bash
# Fork https://github.com/microsoft/winget-pkgs on GitHub

# Clone your fork
git clone https://github.com/YOUR_USERNAME/winget-pkgs.git
cd winget-pkgs

# Create directory structure
mkdir -p manifests/m/Marco/MSC/0.1.0

# Copy manifests
cp ../msc/packaging/winget/*.yaml manifests/m/Marco/MSC/0.1.0/

# Create branch
git checkout -b add-msc-0.1.0

# Commit
git add manifests/m/Marco/MSC/
git commit -m "New package: Marco.MSC version 0.1.0"

# Push
git push origin add-msc-0.1.0

# Create PR on GitHub to microsoft/winget-pkgs
```

**PR Title:** `New package: Marco.MSC version 0.1.0`

**PR Description:**
```markdown
# MSC v0.1.0

Multi-purpose CLI tool for system monitoring and productivity.

## Testing

- [x] Manifest validated with `winget validate`
- [x] Installer tested on Windows 10/11
- [x] Silent install works correctly
- [x] Uninstall works correctly

## Links

- Repository: https://github.com/YOUR_USERNAME/msc
- Release: https://github.com/YOUR_USERNAME/msc/releases/tag/v0.1.0
```

**Wait for approval:** Usually 1-7 days

---

## Step 3: Homebrew Tap

### 3.1 Create homebrew-msc repository

1. Go to GitHub and create a new public repository named **`homebrew-msc`**
2. Clone it locally:

```bash
git clone https://github.com/YOUR_USERNAME/homebrew-msc.git
cd homebrew-msc
```

### 3.2 Update Formula

1. Create `Formula/` directory:
```bash
mkdir Formula
```

2. Copy and update the formula:
```bash
cp ../msc/packaging/homebrew/msc.rb Formula/msc.rb
```

3. Edit `Formula/msc.rb`:
   - Replace `YOUR_USERNAME` with your GitHub username
   - Replace `REPLACE_WITH_SHA256_ARM64_MACOS` with hash from `msc-aarch64-apple-darwin.tar.xz.sha256`
   - Replace `REPLACE_WITH_SHA256_X86_64_MACOS` with hash from `msc-x86_64-apple-darwin.tar.xz.sha256`
   - Replace `REPLACE_WITH_SHA256_ARM64_LINUX` with hash from `msc-aarch64-unknown-linux-gnu.tar.xz.sha256`
   - Replace `REPLACE_WITH_SHA256_X86_64_LINUX` with hash from `msc-x86_64-unknown-linux-gnu.tar.xz.sha256`

### 3.3 Publish

```bash
git add Formula/msc.rb
git commit -m "Add msc formula v0.1.0"
git push origin main
```

### 3.4 Users can now install with:

```bash
brew tap YOUR_USERNAME/msc
brew install msc
```

---

## Step 4: Arch User Repository (AUR)

### 4.1 Prerequisites

- AUR account at https://aur.archlinux.org
- SSH key added to AUR account

### 4.2 Update PKGBUILD

1. Open `packaging/aur/PKGBUILD`
2. Replace `YOUR_USERNAME` with your GitHub username
3. Replace `your-email@example.com` with your email
4. Replace `REPLACE_WITH_SHA256_X86_64_LINUX` with hash from `msc-x86_64-unknown-linux-gnu.tar.xz.sha256`
5. Replace `REPLACE_WITH_SHA256_ARM64_LINUX` with hash from `msc-aarch64-unknown-linux-gnu.tar.xz.sha256`

### 4.3 Publish to AUR

```bash
# Clone AUR repository (will be empty initially)
git clone ssh://aur@aur.archlinux.org/msc-bin.git
cd msc-bin

# Copy PKGBUILD
cp ../msc/packaging/aur/PKGBUILD .

# Generate .SRCINFO
makepkg --printsrcinfo > .SRCINFO

# Commit and push
git add PKGBUILD .SRCINFO
git commit -m "Initial commit: msc-bin 0.1.0"
git push
```

### 4.4 Users can now install with:

```bash
yay -S msc-bin
# or
paru -S msc-bin
```

---

## Step 5: Update Main README

Update your main `README.md` with installation instructions:

```markdown
## Installation

### Windows

#### Via winget (recommended)
```powershell
winget install Marco.MSC
```

#### Via MSI installer
Download the latest `.msi` from [releases](https://github.com/YOUR_USERNAME/msc/releases)

### macOS / Linux

#### Via Homebrew
```bash
brew tap YOUR_USERNAME/msc
brew install msc
```

#### Via install script
```bash
curl -sSL https://github.com/YOUR_USERNAME/msc/releases/latest/download/msc-installer.sh | sh
```

### Arch Linux

```bash
yay -S msc-bin
```

### From source

```bash
cargo install --git https://github.com/YOUR_USERNAME/msc
```

## Updating

MSC includes a built-in self-update feature:

```bash
msc update
```

**Windows Note:** You may need to run your terminal as Administrator to update.
```

---

## Checklist

Use this checklist after creating v0.1.0 release:

- [ ] Download `sha256.sum` from GitHub release
- [ ] Extract MSI ProductCode
- [ ] Update winget manifests with real values
- [ ] Validate winget manifests locally
- [ ] Submit PR to microsoft/winget-pkgs
- [ ] Create `homebrew-msc` repository
- [ ] Update Homebrew formula with SHA256 hashes
- [ ] Push Homebrew formula to GitHub
- [ ] Test Homebrew installation
- [ ] Update AUR PKGBUILD with SHA256 hashes
- [ ] Publish to AUR
- [ ] Test AUR installation
- [ ] Update main README.md with installation instructions
- [ ] Announce release on social media / forums

---

## Future Releases

For version 0.2.0 and beyond:

1. Update version in `Cargo.toml`
2. Create new git tag: `git tag -a v0.2.0 -m "Release v0.2.0"`
3. Push tag: `git push origin v0.2.0`
4. Wait for GitHub Actions to build release
5. Download new `sha256.sum`
6. Update all package manager files with new version and hashes
7. For **winget**: Create new PR with updated manifests in `manifests/m/Marco/MSC/0.2.0/`
8. For **Homebrew**: Update `Formula/msc.rb` and push
9. For **AUR**: Update PKGBUILD, regenerate .SRCINFO, and push

---

## Troubleshooting

### winget validation fails
- Ensure ProductCode format is correct: `{XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX}`
- Verify SHA256 hash matches exactly (no extra spaces or newlines)
- Check YAML indentation (2 spaces, not tabs)

### Homebrew formula fails
- Test locally: `brew install --build-from-source Formula/msc.rb`
- Verify URLs are accessible
- Ensure SHA256 hashes are correct

### AUR package fails
- Run `makepkg` locally to test
- Check .SRCINFO is in sync with PKGBUILD
- Verify source URLs are accessible

---

**Remember:** Replace all instances of `YOUR_USERNAME` with your actual GitHub username!
