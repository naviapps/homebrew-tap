cask "kiseki" do
  version "1.0.11"
  sha256 "7ef9b0ef71ca31688ac1469692f2d194084b908d4d4d6a1625533946a4225e66"

  url "https://github.com/naviapps/kiseki/releases/download/v#{version}/Kiseki.dmg"
  name "Kiseki"
  desc "Mouse and trackpad gesture shortcuts"
  homepage "https://kisekiapp.com/"

  livecheck do
    url :url
    strategy :github_latest
  end

  auto_updates true
  depends_on macos: :tahoe

  app "Kiseki.app"

  uninstall quit: "com.naviapps.Kiseki"

  zap trash: [
    "~/Library/Application Support/com.naviapps.Kiseki",
    "~/Library/Caches/com.naviapps.Kiseki",
    "~/Library/HTTPStorages/com.naviapps.Kiseki",
    "~/Library/HTTPStorages/com.naviapps.Kiseki.binarycookies",
    "~/Library/Preferences/com.naviapps.Kiseki.plist",
    "~/Library/Saved Application State/com.naviapps.Kiseki.savedState",
  ]
end
