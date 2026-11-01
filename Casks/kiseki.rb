cask "kiseki" do
  version "1.0.10"
  sha256 "186ef9f869c3ad6a04ab7ac257c6729a39dc3fd2e83baa303bc06cc262af7c1c"

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
