# Homebrew formula. Live copy for the tap:
#   brew install inaki/talaria/talaria
#
# After tagging, set `url` + `sha256` to the GitHub archive.
class Talaria < Formula
  desc "Unofficial native ratatui TUI host for Hermes Agent"
  homepage "https://github.com/inaki/talaria"
  version "0.1.0"
  head "https://github.com/inaki/talaria.git", branch: "main"
  license "MIT"

  depends_on "rust" => :build

  def install
    system "cargo", "install", "--locked", "--root", prefix, "--path", "."
  end

  def caveats
    <<~EOS
      Talaria is an independent, unofficial TUI host for Hermes Agent
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
