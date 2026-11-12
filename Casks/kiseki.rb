cask "kiseki" do
  version "1.0.12"
  sha256 "b20b25dac04608e4c3cf6a40347d813f71424ffee0dc6d4beeaf7c19a864d723"

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
