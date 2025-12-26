# Package Manager Integration - Phase 5

This directory contains all files needed to publish MSC to various package managers.

## 📋 Status

All files have been **prepared with placeholders**. They need to be completed **AFTER** creating the first GitHub release (Phase 3).

## 📁 Directory Structure

```
packaging/
├── README.md                     # This file
├── POST_RELEASE_STEPS.md         # Detailed guide for completing after release
│
├── winget/                       # Windows Package Manager
│   ├── Marco.MSC.yaml           # Main manifest
│   ├── Marco.MSC.installer.yaml # Installer configuration
│   └── Marco.MSC.locale.en-US.yaml # Locale/metadata
│
├── homebrew/                     # Homebrew (macOS/Linux)
│   └── msc.rb                   # Formula file
│
└── aur/                          # Arch User Repository
    ├── PKGBUILD                 # Build instructions
    └── .SRCINFO.template        # Metadata template
```

## 🚀 Quick Start

### Before Release (Current State)

✅ **All preparation is complete!** Files are ready with placeholders.

### After Release (Phase 3)

Follow the detailed guide in **`POST_RELEASE_STEPS.md`** which covers:

1. Getting SHA256 hashes from GitHub release
2. Extracting MSI ProductCode
3. Updating all manifests with real values
4. Submitting to each package manager
5. Testing installations

## 📦 Package Managers Supported

| Platform | Package Manager | Status | Users Can Install With |
|----------|----------------|---------|------------------------|
| Windows | winget | 🟡 Prepared | `winget install Marco.MSC` |
| macOS/Linux | Homebrew | 🟡 Prepared | `brew tap marco/msc && brew install msc` |
| Arch Linux | AUR | 🟡 Prepared | `yay -S msc-bin` |

🟡 = Prepared with placeholders, needs completion after release

## ⚠️ Important Placeholders to Replace

In **ALL** files, you must replace:

- `YOUR_USERNAME` → Your actual GitHub username
- `REPLACE_WITH_SHA256_*` → Actual SHA256 hashes from release
- `{REPLACE_WITH_PRODUCT_CODE}` → MSI ProductCode (winget only)
- `your-email@example.com` → Your email (AUR only)

## 🎯 Next Steps

1. **Complete Phase 3** - Create first GitHub release
   ```bash
   git tag -a v0.1.0 -m "Release v0.1.0"
   git push origin v0.1.0
   ```

2. **Wait for GitHub Actions** to build and publish artifacts

3. **Follow POST_RELEASE_STEPS.md** to complete package manager integration

4. **Announce** your project on:
   - Reddit: r/rust, r/commandline
   - Hacker News
   - Twitter/Mastodon
   - Your blog

## 📚 Documentation

- **POST_RELEASE_STEPS.md** - Complete step-by-step guide
- **winget/\*.yaml** - Contains inline instructions
- **homebrew/msc.rb** - Contains inline instructions
- **aur/PKGBUILD** - Contains inline instructions

## 🔄 For Future Releases

When releasing v0.2.0, v0.3.0, etc:

1. Update version in `Cargo.toml`
2. Create new git tag
3. Download new SHA256 hashes
4. Update package manager files:
   - **winget**: Create new manifest directory for new version
   - **Homebrew**: Update version and hashes in `msc.rb`
   - **AUR**: Update `pkgver` and hashes in `PKGBUILD`

## ❓ Need Help?

- Check **POST_RELEASE_STEPS.md** for detailed instructions
- Look for inline comments in each file
- Review Phase 5 in `docs/DISTRIBUTION_PLAN.md`

## ✅ Checklist

After release, use this checklist:

- [ ] Get `sha256.sum` from GitHub release
- [ ] Extract MSI ProductCode
- [ ] Update winget manifests
- [ ] Validate winget manifests: `winget validate --manifest packaging/winget/`
- [ ] Submit PR to microsoft/winget-pkgs
- [ ] Create homebrew-msc repository
- [ ] Update and publish Homebrew formula
- [ ] Update and publish AUR package
- [ ] Test installations on each platform
- [ ] Update main README to remove "Coming Soon" labels
- [ ] Announce release!

---

**Current Phase:** 5 (Package Manager Preparation) ✅ **COMPLETE**

**Next Phase:** Wait for Phase 3 release, then follow POST_RELEASE_STEPS.md
