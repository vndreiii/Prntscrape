#!/usr/bin/env bash
# Sourced by linuxdeploy's AppRun after it establishes APPDIR.
export GIO_MODULE_DIR="$APPDIR/usr/lib/gio/modules"
# linuxdeploy can retain its existing AppRun.wrapped executable instead of the
# custom launcher. Its hooks must also establish WebKit's relative helper root.
cd "$APPDIR/usr"
