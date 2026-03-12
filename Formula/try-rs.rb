class TryRs < Formula
  desc "Fast, native implementation of try - fresh directories for every vibe"
  homepage "https://github.com/BrainBuzzer/try-rs"
  url "https://github.com/BrainBuzzer/try-rs/releases/download/v2.0.0/try-x86_64-apple-darwin.tar.gz"
  sha256 "0" * 64
  head "https://github.com/BrainBuzzer/try-rs.git", branch: "main"

  # TODO: Update url and sha256 after first release (v2.0.0):
  # 1. Create GitHub release tag v2.0.0
  # 2. Download the x86_64-apple-darwin tarball from GitHub Actions artifacts
  # 3. Get SHA256: sha256sum try-x86_64-apple-darwin.tar.gz
  # 4. Update url with actual release download link
  # 5. Update sha256 with actual hash value

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
