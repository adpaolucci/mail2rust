class Mail2rust < Formula
  desc "Pure-Rust Iced GUI, CLI, [MS-PST] container engine, and MCP server for mailbox migration and eDiscovery"
  homepage "https://mail2rust.guap.ca"
  version "3.5.0"
  license "Apache-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/adpaolucci/mail2rust/releases/download/v3.5.0/mail2rust-aarch64-apple-darwin.tar.gz"
      sha256 "0000000000000000000000000000000000000000000000000000000000000000"
    else
      url "https://github.com/adpaolucci/mail2rust/releases/download/v3.5.0/mail2rust-x86_64-apple-darwin.tar.gz"
      sha256 "0000000000000000000000000000000000000000000000000000000000000000"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/adpaolucci/mail2rust/releases/download/v3.5.0/mail2rust-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "0000000000000000000000000000000000000000000000000000000000000000"
    end
  end

  def install
    bin.install "mail2rust"
    bin.install "mail2rust-iced"
    bin.install "mail2rust-mcp"
    doc.install "README.md", "CHANGELOG.md"
    doc.install "SBOM-crates.txt" if File.exist?("SBOM-crates.txt")
    prefix.install "LICENSE", "NOTICE"
  end

  test do
    assert_match "mail2rust", shell_output("#{bin}/mail2rust --help")
    assert_match "mail2rust_sniff", shell_output("#{bin}/mail2rust mcp --tools")
    assert_match "mail2rust-iced", shell_output("#{bin}/mail2rust-iced --help")
  end
end
