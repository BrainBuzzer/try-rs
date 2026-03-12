class TryRs < Formula
  desc "Fast, native implementation of try - fresh directories for every vibe"
  homepage "https://github.com/BrainBuzzer/try-rs"
  url "https://github.com/BrainBuzzer/try-rs/releases/download/v2.0.0/try-x86_64-apple-darwin.tar.gz"
  sha256 "61c616d50b716c9a948b046f060024c0eccf03d1e10bddf06e00d88fa7816de4"
  head "https://github.com/BrainBuzzer/try-rs.git", branch: "main"

  depends_on "rust" => :build if build.head?

  def install
    if build.head?
      system "cargo", "build", "--release"
      bin.install "target/release/try"
    else
      bin.install "try"
    end
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
