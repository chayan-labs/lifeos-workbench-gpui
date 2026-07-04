#!/bin/zsh
# Assemble "Life OS Workbench.app" from a release build.
#
# The gpui frontend compiles to a single native macOS binary, so the .app is
# just that binary + an icon + an Info.plist in the standard layout - no bundler
# tool needed. Bundling as a real .app (rather than running the bare binary)
# also lets the app take the Accessibility path, which is what makes native
# menu / keyboard driving in end-to-end tests work.
#
#   scripts/bundle-app.sh [output-dir]   (default: dist/)
#
# Produces: "<out>/Life OS Workbench.app", ad-hoc codesigned, plus a zip.
set -euo pipefail

repo="$(cd "$(dirname "$0")/.." && pwd)"
out="${1:-$repo/dist}"
app="$out/Life OS Workbench.app"

echo "==> building release binary"
# Build from inside workbench/ so its rust-toolchain.toml pin (1.95.0, which
# gpui needs for stabilised APIs like std::hint::cold_path) is honored - a
# --manifest-path build from the repo root would fall back to the default
# toolchain and fail to compile gpui.
(cd "$repo/workbench" && cargo build --release)

bin="$repo/workbench/target/release/workbench"
version="$(grep -m1 '^version' "$repo/workbench/Cargo.toml" | cut -d'"' -f2)"

echo "==> assembling $app (v$version)"
mkdir -p "$out"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$bin" "$app/Contents/MacOS/workbench"
cp "$repo/workbench/assets/Workbench.icns" "$app/Contents/Resources/Workbench.icns"

cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key><string>Life OS Workbench</string>
    <key>CFBundleDisplayName</key><string>Life OS Workbench</string>
    <key>CFBundleIdentifier</key><string>com.chayan.lifeos.workbench</string>
    <key>CFBundleVersion</key><string>$version</string>
    <key>CFBundleShortVersionString</key><string>$version</string>
    <key>CFBundleExecutable</key><string>workbench</string>
    <key>CFBundleIconFile</key><string>Workbench</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>LSApplicationCategoryType</key><string>public.app-category.developer-tools</string>
    <key>LSMinimumSystemVersion</key><string>12.0</string>
    <key>NSHighResolutionCapable</key><true/>
    <key>NSSupportsAutomaticGraphicsSwitching</key><true/>
</dict>
</plist>
PLIST

echo "==> codesigning (ad-hoc)"
codesign --force --deep --sign - "$app"

echo "==> zipping for distribution"
(cd "$out" && ditto -c -k --keepParent "Life OS Workbench.app" "Life-OS-Workbench.app.zip")

echo "done: $app"
echo "      $out/Life-OS-Workbench.app.zip"
