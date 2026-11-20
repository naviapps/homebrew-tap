cask "kiseki" do
  version "1.0.13"
  sha256 "7721ac2b27a1972baf5f6d21056e9963d174d72d0108f53e30e5559c351f68f2"

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
