class Msc < Formula
  desc "Multi-purpose CLI tool for system monitoring, media management, and productivity"
  homepage "https://github.com/MarcossIC/msc"
  version "0.1.13"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/MarcossIC/msc/releases/download/v#{version}/msc-aarch64-apple-darwin.tar.xz"
      sha256 "baf45f212fae2df23e2e6d74f414e94664989e0668d82dd1255e1f9bc5d6182a"
    else
      url "https://github.com/MarcossIC/msc/releases/download/v#{version}/msc-x86_64-apple-darwin.tar.xz"
      sha256 "20b6c245cd01878f09c3f5da4764ec72537f95eb46a8cfb1b5498f00437e4f90"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/MarcossIC/msc/releases/download/v#{version}/msc-aarch64-unknown-linux-gnu.tar.xz"
      sha256 "ed1eb7dc1bb40ab830885279d43d36e9add64a7c9537fd0d757966a0ef17aaf2"
    else
      url "https://github.com/MarcossIC/msc/releases/download/v#{version}/msc-x86_64-unknown-linux-gnu.tar.xz"
      sha256 "443809ab0eb1ee776aab869b42b5b8e8169b32d124e07eed9c17e5e655f09ddb"
    end
  end

  def install
    bin.install "msc"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/msc --version")
  end
end
