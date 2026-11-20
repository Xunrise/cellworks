cask "cellworks" do
  version "0.3.0"
  sha256 "f1602d562a41acbea5bda252df0702b0b1eafbf134c1928cb6b5e41d5051b0d5"

  url "https://github.com/Xunrise/cellworks/releases/download/v#{version}/cellworks-app-#{version}.zip"
  name "Cellworks"
  desc "Map of your projects, code, agent sessions and tickets as a zoomable city"
  homepage "https://xunrise.github.io/cellworks/"

  livecheck do
    url :url
    strategy :github_latest
  end

  auto_updates true
  depends_on arch: :arm64
  depends_on macos: ">= :ventura"

  app "Cellworks.app"

  zap trash: [
    "~/.cellworks",
    "~/Library/Application Support/Cellworks",
  ]

  caveats <<~EOS
    Cellworks is ad-hoc signed, not notarized. If macOS says it cannot be
    opened, clear the quarantine flag once:
      xattr -dr com.apple.quarantine "#{appdir}/Cellworks.app"
    or right-click Cellworks.app in Finder and choose Open.
  EOS
end
