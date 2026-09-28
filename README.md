# Prntscrape

Background screenshot capture with a watchlist, tray controls, settings, and automatic updates.

The primary repository is [vndreiii/Prntscrape on GitHub](https://github.com/vndreiii/Prntscrape). GitHub Actions builds the app and publishes releases. [Forgejo](https://code.milfs.party/alex/Prntscrape) is a source backup.

## Downloads and updates

Download the portable package for your platform from [GitHub Releases](https://github.com/vndreiii/Prntscrape/releases). Keep it in a folder your user can write to, so the app can replace it during updates. No installer is needed.

- **Windows x64:** download and run `Prntscrape-x86_64-pc-windows-msvc.exe`. WebView2 Runtime is required. Click the bottom-right startup notification or the tray icon to open settings.
- **Linux x64:** download `Prntscrape-x86_64-unknown-linux-gnu.AppImage`, run `chmod +x Prntscrape-*.AppImage`, then launch it. GTK, WebKit (including helper processes), AppIndicator, and grim are bundled. Builds target Ubuntu 24.04 or newer compatible distributions. Automatic screenshots require a running Hyprland session. Settings support X11 and Wayland and open at startup; background tray access requires a desktop tray host. Without FUSE, add `--appimage-extract-and-run` when launching.
- **macOS (Apple Silicon or Intel):** unzip the matching `Prntscrape-*.app.zip`, move `Prntscrape.app` to a writable folder, and open it. Grant Screen Recording and Automation permissions. The bundle is ad-hoc signed but not Apple notarized; see [macOS instructions](MACOS_INSTRUCTIONS.md).

The app checks **only `vndreiii/Prntscrape` on GitHub** at startup and every six hours, downloading and installing newer stable releases automatically. It verifies SHA-256 checksums, replaces the outer Linux AppImage, the Windows executable, or the entire signed macOS app bundle. Settings also has **Check for Updates** and displays errors or a **Restart** button after installation. Capturing continues until you restart. Offline checks or missing releases leave the current installation intact. Source-built Linux/macOS binaries should be rebuilt manually; automatic updates support the portable packages. Release downloads must be publicly accessible.

## Development

Install Rust, then run:

```sh
cargo build --locked
cargo test --locked
cargo run -- --help
```

Ubuntu build dependencies (in addition to runtime capture tools):

```sh
sudo apt-get install libgtk-3-dev libwebkit2gtk-4.1-dev libayatana-appindicator3-dev libdbus-1-dev pkg-config
```

Native UI adapters and themes live in `src/ui/platform/{windows,linux,macos}.*`. Shared settings markup, behavior, and layout live in `src/ui/settings.{html,js,css}`; `src/ui/mod.rs` connects them to capture/configuration/update events.

## Publishing an update

1. Increase `version` in `Cargo.toml`, run `cargo check` to update `Cargo.lock`, then commit.
2. Push the commit to GitHub (`origin`) and Forgejo (`forgejo`).
3. GitHub Actions builds all four portable packages, creates a matching tag and release, uploads every package and `SHA256SUMS`, then publishes the release.

```sh
git push origin main
git push forgejo main
```

GitHub Actions tests and builds all four targets on pushes to `main`, pull requests, and manual runs. **Every new Cargo version pushed to `main` automatically becomes a release after all packages succeed.** Reusing an already published version leaves that release unchanged; bump the version for a new update. A manual Actions run on `main` can retry an unpublished version. Explicit `v*` tags are also supported and must match the package version. Pull requests produce artifacts only. Publication uses a draft until all uploads finish, so the updater never sees an incomplete release. GitHub runs CI; Forgejo stores the source backup.
