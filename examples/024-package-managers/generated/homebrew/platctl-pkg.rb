# Homebrew formula (Ruby DSL) for a prebuilt binary – lives in a tap repository:
#   github.com/example-org/homebrew-tap/Formula/platctl-pkg.rb
# Users: brew install example-org/tap/platctl-pkg
class PlatctlPkg < Formula
  desc "Platform CLI distributed through package managers"
  homepage "https://github.com/example-org/platctl-pkg"
  version "1.2.0"
  license "MIT"

  # One archive per OS/CPU; Homebrew picks the matching block.
  on_macos do
    on_arm do
      url "https://github.com/example-org/platctl-pkg/releases/download/v1.2.0/platctl-pkg-v1.2.0-aarch64-apple-darwin.tar.gz"
      sha256 "81d913fac5c5b344828d39f48fbefc410a35172f795b87915faf0002e4a7ed26"
    end
    on_intel do
      url "https://github.com/example-org/platctl-pkg/releases/download/v1.2.0/platctl-pkg-v1.2.0-x86_64-apple-darwin.tar.gz"
      sha256 "61433adc479da06c3fb3fdfc7075193bf39eee7633b24ee36357372b8c12f99b"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/example-org/platctl-pkg/releases/download/v1.2.0/platctl-pkg-v1.2.0-aarch64-unknown-linux-musl.tar.gz"
      sha256 "aacdd3c6668596190b3200d648302e3c43505424699e5883f5cafe9cd51aeafc"
    end
    on_intel do
      url "https://github.com/example-org/platctl-pkg/releases/download/v1.2.0/platctl-pkg-v1.2.0-x86_64-unknown-linux-musl.tar.gz"
      sha256 "5b863e36fb1b5eb73e057fa0dc41d5dd850b0fb8ae3f72bd5504f6caeec09fde"
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
