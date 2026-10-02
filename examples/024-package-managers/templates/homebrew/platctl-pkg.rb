# Homebrew formula (Ruby DSL) for a prebuilt binary – lives in a tap repository:
#   github.com/example-org/homebrew-tap/Formula/platctl-pkg.rb
# Users: brew install example-org/tap/platctl-pkg
class PlatctlPkg < Formula
  desc "{{description}}"
  homepage "{{homepage}}"
  version "{{version}}"
  license "MIT"

  # One archive per OS/CPU; Homebrew picks the matching block.
  on_macos do
    on_arm do
      url "{{url:aarch64-apple-darwin}}"
      sha256 "{{sha256:aarch64-apple-darwin}}"
    end
    on_intel do
      url "{{url:x86_64-apple-darwin}}"
      sha256 "{{sha256:x86_64-apple-darwin}}"
    end
  end

  on_linux do
    on_arm do
      url "{{url:aarch64-unknown-linux-musl}}"
      sha256 "{{sha256:aarch64-unknown-linux-musl}}"
    end
    on_intel do
      url "{{url:x86_64-unknown-linux-musl}}"
      sha256 "{{sha256:x86_64-unknown-linux-musl}}"
    end
  end

  def install
    # The archive's single top-level directory is the working directory here.
    bin.install "platctl-pkg"
  end

  # `brew test platctl-pkg` – smoke test after installation (also run by tap CI).
  test do
    assert_match version.to_s, shell_output("#{bin}/platctl-pkg --version")
  end
end
