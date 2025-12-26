class Msc < Formula
  desc "Multi-purpose CLI tool for system monitoring, media management, and productivity"
  homepage "https://github.com/MarcossIC/msc"
  version "0.1.8"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/MarcossIC/msc/releases/download/v0.1.8/msc-0.1.8-x86_64-apple-darwin.tar.xz"
      sha256 "Not"
    else
      url "https://github.com/MarcossIC/msc/releases/download/v0.1.8/msc-0.1.8-x86_64-apple-darwin.tar.xz"
      sha256 "Not"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/MarcossIC/msc/releases/download/v0.1.8/msc-0.1.8-x86_64-apple-darwin.tar.xz"
      sha256 "Not"
    else
      url "https://github.com/MarcossIC/msc/releases/download/v0.1.8/msc-0.1.8-x86_64-apple-darwin.tar.xz"
      sha256 "Not"
    end
  end

  def install
    bin.install "msc"

    # Optional: Install shell completions if generated
    # bash_completion.install "completions/msc.bash" => "msc"
    # zsh_completion.install "completions/_msc"
    # fish_completion.install "completions/msc.fish"

    # Optional: Install man pages if available
    # man1.install "man/msc.1"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/msc --version")
  end
end

# INSTRUCTIONS FOR COMPLETION:
#
# 1. Replace YOUR_USERNAME with your GitHub username
#
# 2. After creating release v0.1.0, get SHA256 hashes from:
#    https://github.com/MarcossIC/msc/releases/download/v0.1.0/sha256.sum
#
#    Or download each .sha256 file:
#    - msc-aarch64-apple-darwin.tar.xz.sha256      (ARM64 macOS)
#    - msc-x86_64-apple-darwin.tar.xz.sha256       (Intel macOS)
#    - msc-aarch64-unknown-linux-gnu.tar.xz.sha256 (ARM64 Linux)
#    - msc-x86_64-unknown-linux-gnu.tar.xz.sha256  (x86_64 Linux)
#
# 3. Create a new GitHub repository named "homebrew-msc"
#
# 4. Create directory structure:
#    homebrew-msc/
#    └── Formula/
#        └── msc.rb  (this file with SHA256s filled in)
#
# 5. Users can then install with:
#    brew tap YOUR_USERNAME/msc
#    brew install msc
#
# 6. To update the formula for new versions:
#    - Update version number
#    - Update URLs to new version tag
#    - Update all SHA256 hashes
#    - Commit and push to homebrew-msc repository
