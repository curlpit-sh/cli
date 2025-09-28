class Curlpit < Formula
  desc "Terminal-first HTTP runner"
  homepage "https://github.com/curlpit-sh/cli"
  version "0.2.7"
  url "https://github.com/curlpit-sh/cli/archive/refs/tags/v0.2.7.tar.gz"
  sha256 "REPLACE_WITH_TARBALL_SHA256"
  license "MIT"

  depends_on "rust" => :build

  def install
    system "cargo", "install", "--locked", "--path", ".", "--root", prefix
    bin.install_symlink prefix/"bin"/"curlpit"
  end

  test do
    assert_match "curlpit", shell_output("#{bin}/curlpit --help")
  end
end
