#!/bin/sh
# Renders assets/appicon.svg into the files the app uses:
#   assets/AppIcon.icns                     icon of the .app bundle (Dock, Finder)
#   crates/openroc-gui/assets/icon.png      window / Dock icon, built into the binary
# Needs rsvg-convert (brew install librsvg); iconutil comes with macOS.
set -eu
cd "$(dirname "$0")/.."

svg=assets/appicon.svg
set_dir=$(mktemp -d)
trap 'rm -rf "$set_dir"' EXIT
iconset="$set_dir/AppIcon.iconset"
mkdir "$iconset"

render() { rsvg-convert -w "$1" -h "$1" "$svg" -o "$2"; }

for size in 16 32 128 256 512; do
	render "$size" "$iconset/icon_${size}x${size}.png"
	render "$((size * 2))" "$iconset/icon_${size}x${size}@2x.png"
done

iconutil -c icns "$iconset" -o assets/AppIcon.icns
render 512 crates/openroc-gui/assets/icon.png
echo "wrote assets/AppIcon.icns and crates/openroc-gui/assets/icon.png"
