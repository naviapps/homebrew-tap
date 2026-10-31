cask "kiseki" do
  version "1.0.7"
  sha256 "99f90e1573d4f9ba66dbadcf8c565a560608ef79858154b1966b192737c0dfb3"

  url "https://github.com/naviapps/kiseki/releases/download/v#{version}/Kiseki.dmg"
  name "Kiseki"
  desc "Mouse gesture shortcuts"
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
