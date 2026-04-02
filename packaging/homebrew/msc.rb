class Msc < Formula
  desc "Multi-purpose CLI tool for system monitoring, media management, and productivity"
  homepage "https://github.com/MarcossIC/msc"
  version "0.1.8"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/MarcossIC/msc/releases/download/v#{version}/msc-aarch64-apple-darwin.tar.xz"
      sha256 "REPLACE_WITH_SHA256_ARM64_MACOS"
    else
      url "https://github.com/MarcossIC/msc/releases/download/v#{version}/msc-x86_64-apple-darwin.tar.xz"
      sha256 "REPLACE_WITH_SHA256_X86_64_MACOS"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/MarcossIC/msc/releases/download/v#{version}/msc-aarch64-unknown-linux-gnu.tar.xz"
      sha256 "REPLACE_WITH_SHA256_ARM64_LINUX"
    else
      url "https://github.com/MarcossIC/msc/releases/download/v#{version}/msc-x86_64-unknown-linux-gnu.tar.xz"
      sha256 "REPLACE_WITH_SHA256_X86_64_LINUX"
    end
  end

  def install
    bin.install "msc"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/msc --version")
  end
end
