class TryRs < Formula
  desc "Fast, native implementation of try - fresh directories for every vibe"
  homepage "https://github.com/BrainBuzzer/try-rs"
  head "https://github.com/BrainBuzzer/try-rs.git", branch: "main"

  depends_on "rust" => :build

  def install
    system "cargo", "build", "--release"
    bin.install "target/release/try"
  end

  def caveats
    <<~EOS
      To set up try with your shell, add one of the following to your shell configuration:

      For bash/zsh (add to .bashrc or .zshrc):
        eval "$(try init)"

      For fish (add to config.fish):
        try init | source

      The default workspace is ~/src/tries (configurable via TRY_PATH).
    EOS
  end

  test do
    system "#{bin}/try", "--help"
  end
end
