class Msc < Formula
  desc "Multi-purpose CLI tool for system monitoring, media management, and productivity"
  homepage "https://github.com/MarcossIC/msc"
  version "0.1.14"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/MarcossIC/msc/releases/download/v#{version}/msc-aarch64-apple-darwin.tar.xz"
      sha256 "be7ac261a90cfe3f398cd9cbb96e5c2dabf023b3aa073ebc2d19feb2177ca4bb"
    else
      url "https://github.com/MarcossIC/msc/releases/download/v#{version}/msc-x86_64-apple-darwin.tar.xz"
      sha256 "f7b34bc98f8f82e4151186c47ef0fa7ea8bc82f7a99808c2a16bb21afe3f6beb"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/MarcossIC/msc/releases/download/v#{version}/msc-aarch64-unknown-linux-gnu.tar.xz"
      sha256 "8fe044ffa2bdd67aed1cc3e3ed5b127410101f12a39c15dc76f2b8da63e0d1e4"
    else
      url "https://github.com/MarcossIC/msc/releases/download/v#{version}/msc-x86_64-unknown-linux-gnu.tar.xz"
      sha256 "56a395cc5951afdd38fa4a761667d92ec5d63dc04b3247f5c3a4457da2b7ec3b"
    end
  end

  def install
    bin.install "msc"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/msc --version")
  end
end
