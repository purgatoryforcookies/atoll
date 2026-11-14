#!/bin/sh
# Builds Atoll.app. Usage: scripts/bundle.sh [app path], default ~/Applications/Atoll.app.
# SIGN: codesign identity, default ad-hoc "-". VERSION: default Cargo.toml's.
# UNIVERSAL=1: one binary for Apple silicon and Intel (needs both rustup targets).
set -eu
APP=${1:-$HOME/Applications/Atoll.app}
SIGN=${SIGN:--}
VERSION=${VERSION:-$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)}

if [ "${UNIVERSAL:-}" = 1 ]; then
  cargo build --release --target aarch64-apple-darwin --target x86_64-apple-darwin
  BIN=target/release/atoll-universal
  lipo -create -output "$BIN" target/aarch64-apple-darwin/release/atoll target/x86_64-apple-darwin/release/atoll
else
  cargo build --release
  BIN=target/release/atoll
fi

# Drop binaries from earlier builds, such as the pre-rename `notch`.
rm -rf "$APP/Contents/MacOS"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BIN" "$APP/Contents/MacOS/atoll"
cp icon.icns "$APP/Contents/Resources/icon.icns"
cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Atoll</string>
  <key>CFBundleDisplayName</key><string>Atoll</string>
  <key>CFBundleIdentifier</key><string>dev.atoll.app</string>
  <key>CFBundleExecutable</key><string>atoll</string>
  <key>CFBundleIconFile</key><string>icon</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$VERSION</string>
  <key>CFBundleVersion</key><string>$VERSION</string>
  <key>LSMinimumSystemVersion</key><string>13.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSHumanReadableCopyright</key><string>© 2026 purgatoryforcookies</string>
</dict>
</plist>
PLIST

if [ "$SIGN" = - ]; then
  codesign --force --sign - "$APP"
else
  # Notarization needs the hardened runtime and a secure timestamp.
  codesign --force --options runtime --timestamp --sign "$SIGN" "$APP"
fi
echo "$APP"
