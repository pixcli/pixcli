class Pixcli < Formula
  desc "CLI tool for Brazilian Pix payments"
  homepage "https://github.com/pixcli/pixcli"
  version "0.1.0"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/pixcli/pixcli/releases/download/v#{version}/pixcli-v#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "PLACEHOLDER"
    else
      url "https://github.com/pixcli/pixcli/releases/download/v#{version}/pixcli-v#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "PLACEHOLDER"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/pixcli/pixcli/releases/download/v#{version}/pixcli-v#{version}-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "PLACEHOLDER"
    else
      url "https://github.com/pixcli/pixcli/releases/download/v#{version}/pixcli-v#{version}-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "PLACEHOLDER"
    end
  end

  def install
    bin.install "pixcli"
    bin.install "pix-mcp"
    bin.install "pix-webhook-server"
  end

  test do
    assert_match "pixcli", shell_output("#{bin}/pixcli --version")
  end
end
