#!/usr/bin/env bash
set -euo pipefail
target=${1:?Rust target required}
version=${2:?Version required}
case "$target" in aarch64-apple-darwin|x86_64-apple-darwin) ;; *) exit 1 ;; esac
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
build_dir=$(mktemp -d)
bundle="$build_dir/Prntscrape.app"
mkdir -p "$bundle/Contents/MacOS" "$bundle/Contents/Resources" "$repo_dir/dist"
install -m 755 "$repo_dir/target/$target/release/prntscrape" "$bundle/Contents/MacOS/prntscrape"
cp "$repo_dir/packaging/macos/Info.plist" "$bundle/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleShortVersionString $version" "$bundle/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleVersion $version" "$bundle/Contents/Info.plist"
plutil -lint "$bundle/Contents/Info.plist"
# Ad-hoc signing preserves bundle integrity; Developer ID/notarization requires an Apple account.
codesign --force --deep --sign - "$bundle"
codesign --verify --deep --strict "$bundle"
ditto -c -k --sequesterRsrc --keepParent "$bundle" "$repo_dir/dist/Prntscrape-$target.app.zip"
"$bundle/Contents/MacOS/prntscrape" --help
