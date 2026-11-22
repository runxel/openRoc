#!/bin/sh
# Builds target/bundle/openRoc.app (release build, icon, Info.plist, ad-hoc signed).
# Run misc/make-icons.sh first if assets/appicon.svg changed.
set -eu
cd "$(dirname "$0")/.."
export PATH="/opt/homebrew/opt/rustup/bin:$PATH"

version=$(sed -n 's/^version = "\(.*\)"/\1/p' crates/openroc-gui/Cargo.toml | head -n 1)
app=target/bundle/openRoc.app

cargo build --release -p openroc

rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp target/release/openroc "$app/Contents/MacOS/openroc"
cp assets/AppIcon.icns "$app/Contents/Resources/AppIcon.icns"

cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleName</key><string>openRoc</string>
	<key>CFBundleDisplayName</key><string>openRoc</string>
	<key>CFBundleIdentifier</key><string>de.runxel.openroc</string>
	<key>CFBundleExecutable</key><string>openroc</string>
	<key>CFBundleIconFile</key><string>AppIcon</string>
	<key>CFBundlePackageType</key><string>APPL</string>
	<key>CFBundleShortVersionString</key><string>$version</string>
	<key>CFBundleVersion</key><string>$version</string>
	<key>LSMinimumSystemVersion</key><string>11.0</string>
	<key>LSApplicationCategoryType</key><string>public.app-category.utilities</string>
	<key>NSHighResolutionCapable</key><true/>
	<key>NSSupportsAutomaticGraphicsSwitching</key><true/>
</dict>
</plist>
PLIST

# Ad-hoc signature: enough to run on this Mac. Not notarised.
codesign --force --sign - "$app"
echo "built $app"
