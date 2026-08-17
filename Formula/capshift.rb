# typed: strict
# frozen_string_literal: true

class Capshift < Formula
  desc "Caps Lock chord shortcut daemon for macOS"
  homepage "https://github.com/njreid/capshift"
  version "0.2.0"

  depends_on :macos

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/njreid/capshift/releases/download/v#{version}/capshift-#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "0000000000000000000000000000000000000000000000000000000000000000"
    else
      odie "capshift supports Apple Silicon (arm64) Macs only"
    end
  end

  def install
    bin.install "capshift", "capshift-menu"
    pkgshare.install "dev.njreid.capshift.kvhd.plist", "dev.njreid.capshift-menu.plist"
  end

  service do
    run [opt_bin/"capshift"]
    keep_alive true
    require_root true
    log_path var/"log/capshift.log"
    error_log_path var/"log/capshift.err.log"
  end

  def caveats
    <<~EOS
      capshift runs as a root LaunchDaemon and reads this shared configuration:
        /Library/Application Support/capshift/config.kdl

      Install the virtual-HID driver, then start the root services at boot:
        brew install --cask njreid/capshift/karabiner-driverkit-virtualhiddevice
        sudo cp "#{opt_pkgshare}/dev.njreid.capshift.kvhd.plist" /Library/LaunchDaemons/
        sudo chown root:wheel /Library/LaunchDaemons/dev.njreid.capshift.kvhd.plist
        sudo chmod 644 /Library/LaunchDaemons/dev.njreid.capshift.kvhd.plist
        sudo launchctl bootstrap system /Library/LaunchDaemons/dev.njreid.capshift.kvhd.plist
        sudo brew services start capshift

      To start the menu-bar companion automatically when you log in:
        cp "#{opt_pkgshare}/dev.njreid.capshift-menu.plist" ~/Library/LaunchAgents/
        launchctl bootstrap gui/$(id -u) ~/Library/LaunchAgents/dev.njreid.capshift-menu.plist

      Allow capshift in System Settings → Privacy & Security → Input Monitoring.
    EOS
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/capshift --version")
  end
end
