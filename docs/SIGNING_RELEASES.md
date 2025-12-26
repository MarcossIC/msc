# Signing Releases

This document explains how to set up GPG signing for MSC releases to provide additional security and authenticity verification.

## Why Sign Releases?

Signing releases allows users to verify that:
1. The release artifacts were created by you (authenticity)
2. The files haven't been tampered with (integrity)
3. The release came from the official repository (provenance)

## Option 1: GPG Signatures (Traditional)

### Prerequisites

1. **Generate GPG key** (if you don't have one):

```bash
gpg --full-generate-key
```

Choose:
- Key type: RSA and RSA
- Key size: 4096 bits
- Expiration: 2 years (recommended)
- Name and email: Your GitHub account details

2. **Export public key**:

```bash
gpg --armor --export your-email@example.com > public-key.asc
```

3. **Add to GitHub**:
   - Go to GitHub Settings → SSH and GPG keys
   - Add your GPG public key

### Configure GitHub Actions

1. **Export private key** (keep this secure!):

```bash
gpg --export-secret-keys --armor your-email@example.com
```

2. **Add to GitHub Secrets**:
   - Go to your repository → Settings → Secrets → Actions
   - Create new secret: `GPG_PRIVATE_KEY`
   - Paste the private key content

3. **Add GPG passphrase** (if you set one):
   - Create another secret: `GPP_PASSPHRASE`

### Update cargo-dist Configuration

Edit `dist-workspace.toml`:

```toml
[dist]
# ... existing config ...

# Enable checksums
checksum = "sha256"

# Optional: Enable signing (requires GPG setup in CI)
# This will create .sig files alongside artifacts
# sign = true
```

### Sign Manually (Alternative)

If you prefer to sign releases manually:

```bash
# After cargo-dist generates artifacts
cd target/distrib

# Sign each artifact
for file in *.tar.xz *.zip *.msi; do
    gpg --armor --detach-sign "$file"
done

# Creates .asc signature files
# Users can verify with:
# gpg --verify msc-v0.1.0-x86_64-pc-windows-msvc.msi.asc msc-v0.1.0-x86_64-pc-windows-msvc.msi
```

---

## Option 2: Cosign (Modern, Recommended for 2025)

Cosign is a modern signing tool from the Sigstore project that doesn't require managing GPG keys.

### Setup

1. **Add to GitHub Actions workflow**:

Edit `.github/workflows/release.yml` and add after the build step:

```yaml
- name: Install cosign
  uses: sigstore/cosign-installer@v3

- name: Sign artifacts with cosign
  run: |
    # Sign all artifacts
    cosign sign-blob --yes \
      target/distrib/msc-*.tar.xz \
      target/distrib/msc-*.zip \
      target/distrib/msc-*.msi
  env:
    COSIGN_EXPERIMENTAL: 1  # Use keyless signing
```

### Benefits of Cosign

- ✅ No key management (uses OIDC tokens)
- ✅ Transparency log (Rekor)
- ✅ Automatic verification
- ✅ Industry standard for container/artifact signing

### User Verification

Users can verify signatures with:

```bash
# Install cosign
brew install cosign  # macOS
# or download from https://github.com/sigstore/cosign/releases

# Verify
cosign verify-blob \
  --certificate-identity="https://github.com/MarcossIC/msc/.github/workflows/release.yml@refs/tags/v0.1.0" \
  --certificate-oidc-issuer="https://token.actions.githubusercontent.com" \
  --signature=msc-x86_64-pc-windows-msvc.msi.sig \
  msc-x86_64-pc-windows-msvc.msi
```

---

## Recommended Approach

For modern projects in 2025, we recommend:

1. **Use cargo-dist's built-in checksums** (already configured ✅)
   - Provides integrity verification
   - No extra setup needed
   - Works for 99% of use cases

2. **Add Cosign for enhanced security** (optional)
   - Better than GPG for CI/CD
   - No key management overhead
   - Provides provenance guarantees

3. **Skip GPG** unless you have specific requirements
   - Complex key management
   - Users rarely verify GPG signatures
   - Cosign is more modern

---

## Current Status

✅ **SHA256 checksums**: Enabled via cargo-dist
⏸️ **GPG signatures**: Not configured (optional)
⏸️ **Cosign**: Not configured (optional)

## Implementation Steps (If You Want Signing)

### For Cosign (Recommended):

1. Copy the cosign workflow snippet above
2. Add it to `.github/workflows/release.yml`
3. Test with next release
4. Update README with verification instructions

### For GPG:

1. Generate GPG key
2. Add `GPG_PRIVATE_KEY` to GitHub Secrets
3. Add signing step to workflow
4. Publish public key
5. Update README with verification instructions

---

## Verification Documentation

Once signing is set up, add this to your README:

```markdown
## Verifying Releases

All releases include SHA256 checksums for integrity verification.

### Verify Checksum

```bash
# Download artifact and checksum
wget https://github.com/MarcossIC/msc/releases/download/v0.1.0/msc-x86_64-pc-windows-msvc.msi
wget https://github.com/MarcossIC/msc/releases/download/v0.1.0/msc-x86_64-pc-windows-msvc.msi.sha256

# Verify (Linux/macOS)
sha256sum -c msc-x86_64-pc-windows-msvc.msi.sha256

# Verify (Windows PowerShell)
$expected = Get-Content msc-x86_64-pc-windows-msvc.msi.sha256
$actual = (Get-FileHash msc-x86_64-pc-windows-msvc.msi -Algorithm SHA256).Hash
if ($expected -eq $actual) { "✓ Checksum verified" } else { "✗ Checksum mismatch!" }
```

### Verify Signature (if enabled)

See [Cosign/GPG verification instructions]
```
```

---

## References

- [cargo-dist signing docs](https://opensource.axo.dev/cargo-dist/book/reference/config.html#sign)
- [Cosign documentation](https://docs.sigstore.dev/cosign/overview/)
- [GPG signing guide](https://docs.github.com/en/authentication/managing-commit-signature-verification)
- [Sigstore project](https://www.sigstore.dev/)

---

**Current Recommendation**: Checksums are sufficient for initial releases. Add Cosign later if you want enhanced security and provenance guarantees.
