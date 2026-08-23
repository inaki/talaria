# Homebrew formula. Live copy: https://github.com/inaki/homebrew-talaria
#   brew install inaki/talaria/talaria
class Talaria < Formula
  desc "Native Rust TUI client for Hermes Agent"
  homepage "https://github.com/inaki/talaria"
  url "https://github.com/inaki/talaria/archive/refs/tags/v0.1.4.tar.gz"
  sha256 "14bcc28504deea7391748fa794225ec1d6d78634608e115e34bdc5fc584f3e32"
  license "MIT"
  head "https://github.com/inaki/talaria.git", branch: "main"

  depends_on "rust" => :build

  def install
    system "cargo", "install", "--locked", "--root", prefix, "--path", "."
  end

  def caveats
    <<~EOS
      Talaria Client is an independent, unofficial native Rust TUI for Hermes Agent
      (Nous Research). It is not affiliated with or endorsed by Nous Research.

      Live mode uses the same ~/.hermes as official Hermes (models, keys, sessions).
      This formula only installs the talaria UI binary.

      If you do not have Hermes Agent yet:
        curl -fsSL https://hermes-agent.nousresearch.com/install.sh | bash

      Quit hermes --tui before starting talaria (they cannot share the
      session database at the same time).
    EOS
  end

  test do
    assert_match "talaria", shell_output("#{bin}/talaria --version")
  end
end
