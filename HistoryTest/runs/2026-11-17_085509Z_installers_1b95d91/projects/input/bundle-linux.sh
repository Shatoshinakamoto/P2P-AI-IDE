#!/bin/bash
set -euo pipefail
# Build the Linux bundles of one Tauri app and list what the .deb installs (licence resources included).
app="$1"
cd "$app"
npx tauri build --bundles deb,rpm 2>&1 | grep -E "Bundling|Finished|error" || true
deb=$(ls src-tauri/target/release/bundle/deb/*.deb)
dpkg-deb -I "$deb" | grep -E "Package|Version|Depends"
dpkg-deb -c "$deb" | grep -E "LICENSE|NOTICE|usr/bin" | awk '{print $3, $6}'
dpkg-deb -c "$deb" > /tmp/deb-list.txt; grep -q THIRD-PARTY-NOTICES.txt /tmp/deb-list.txt
rpm_file=$(ls src-tauri/target/release/bundle/rpm/*.rpm)
sha256sum "$deb" "$rpm_file" | sed "s#$PWD/##"
