class Lazymqtt < Formula
  desc "Fast terminal UI MQTT client, inspired by MQTT Explorer"
  homepage "https://github.com/ScottFelder/lazymqtt"
  version "0.1.0"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/ScottFelder/lazymqtt/releases/download/v0.1.0/lazymqtt-aarch64-apple-darwin.tar.gz"
      sha256 "a40fa3bd46a1406c644afeb90b2adfa5e6e9bb2e2219d74d296087e2398dad64"
    end
    on_intel do
      url "https://github.com/ScottFelder/lazymqtt/releases/download/v0.1.0/lazymqtt-x86_64-apple-darwin.tar.gz"
      sha256 "9b0bf559f8a3110511b2a9215a9104d64659b7d263e4e09110f44d65109115b8"
    end
  end

  on_linux do
    on_intel do
      url "https://github.com/ScottFelder/lazymqtt/releases/download/v0.1.0/lazymqtt-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "ccd19b5916d82426b49b9b7f8f8439454c5d5531d6013ecb12b11c08e8e7f5cd"
    end
  end

  def install
    bin.install "lazymqtt"
  end

  test do
    assert_match "lazymqtt #{version}", shell_output("#{bin}/lazymqtt --version")
  end
end
