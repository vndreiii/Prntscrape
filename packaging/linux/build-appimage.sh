#!/usr/bin/env bash
set -euo pipefail

target=${1:-x86_64-unknown-linux-gnu}
[[ "$target" == x86_64-unknown-linux-gnu ]] || { echo 'Unsupported AppImage target' >&2; exit 1; }
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
build_dir=$(mktemp -d)
app_dir="$build_dir/Prntscrape.AppDir"
tools_dir="$build_dir/tools"
mkdir -p "$app_dir/usr/bin" "$app_dir/usr/lib/gio/modules" "$app_dir/apprun-hooks" "$tools_dir" "$repo_dir/dist"

curl --fail --location --retry 3 https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage -o "$tools_dir/linuxdeploy.AppImage"
curl --fail --location --retry 3 https://raw.githubusercontent.com/linuxdeploy/linuxdeploy-plugin-gtk/master/linuxdeploy-plugin-gtk.sh -o "$tools_dir/linuxdeploy-plugin-gtk.sh"
chmod +x "$tools_dir/linuxdeploy.AppImage" "$tools_dir/linuxdeploy-plugin-gtk.sh"
export APPIMAGE_EXTRACT_AND_RUN=1
export DEPLOY_GTK_VERSION=3
export ARCH=x86_64
export NO_STRIP=1
export OUTPUT="$repo_dir/dist/Prntscrape-$target.AppImage"
export PATH="$tools_dir:$PATH"

lib_dir=$(pkg-config --variable=libdir gtk+-3.0)
deploy_args=(--appdir "$app_dir" --executable "$repo_dir/target/$target/release/prntscrape"
  --executable /usr/bin/grim --library "$lib_dir/libayatana-appindicator3.so.1"
  --desktop-file "$repo_dir/packaging/linux/prntscrape.desktop"
  --icon-file "$repo_dir/packaging/linux/prntscrape.svg")

# WebKit spawns separate processes; ldd on the main app doesn't discover them.
webkit_count=0
while IFS= read -r process; do
  [[ -x "$process" ]] || continue
  mkdir -p "$app_dir$(dirname "$process")"
  cp -L "$process" "$app_dir$process"
  webkit_exec_dir=$(dirname "$process")
  deploy_args+=(--executable "$process")
  webkit_count=$((webkit_count + 1))
done < <((dpkg-query -L libwebkit2gtk-4.1-0 2>/dev/null || dpkg-query -L libwebkit2gtk-4.1-0t64 2>/dev/null) | awk '/\/WebKit(Web|Network|GPU)Process$/')
[[ "$webkit_count" -ge 2 ]] || { echo 'WebKit helper processes were not found' >&2; exit 1; }
while IFS= read -r bundle; do
  [[ -f "$bundle" ]] || continue
  mkdir -p "$app_dir$(dirname "$bundle")"
  cp -L "$bundle" "$app_dir$bundle"
  deploy_args+=(--library "$bundle")
done < <((dpkg-query -L libwebkit2gtk-4.1-0 2>/dev/null || dpkg-query -L libwebkit2gtk-4.1-0t64 2>/dev/null) | awk '/\/libwebkit2gtkinjectedbundle\.so$/')

# GIO loads TLS/proxy modules dynamically, so include them and their dependencies.
for module in "$lib_dir"/gio/modules/*.so; do
  [[ -f "$module" ]] || continue
  cp -L "$module" "$app_dir/usr/lib/gio/modules/"
  deploy_args+=(--library "$module")
done
gio-querymodules "$app_dir/usr/lib/gio/modules"
"$tools_dir/linuxdeploy.AppImage" "${deploy_args[@]}" --plugin gtk
# Upstream GTK hook forces X11. Tao/Wry support native Wayland as well.
sed -i '/^export GDK_BACKEND=x11/d' "$app_dir/apprun-hooks/linuxdeploy-plugin-gtk.sh"
install -m 644 "$repo_dir/packaging/linux/webkit-hook.sh" "$app_dir/apprun-hooks/webkit.sh"
webkit_library=$(readlink -f "$app_dir/usr/lib/libwebkit2gtk-4.1.so.0")
python3 "$repo_dir/packaging/linux/relocate-webkit.py" "$webkit_library" "$webkit_exec_dir"
install -m 755 "$repo_dir/packaging/linux/AppRun" "$tools_dir/Prntscrape-AppRun"
"$tools_dir/linuxdeploy.AppImage" --appdir "$app_dir" --custom-apprun "$tools_dir/Prntscrape-AppRun" --output appimage
chmod +x "$OUTPUT"
test -s "$OUTPUT"

# Exercise the bundled executable without starting capture or needing a display.
"$OUTPUT" --appimage-extract-and-run --help
smoke_dir=$(mktemp -d)
# Exits only after the WebKit process loads settings and completes the IPC handshake.
XDG_CONFIG_HOME="$smoke_dir/config" XDG_CACHE_HOME="$smoke_dir/cache" timeout 60s xvfb-run -a "$OUTPUT" --appimage-extract-and-run --smoke-test
