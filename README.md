# Prntscrape

Background screenshot capture with a watchlist, tray controls, settings, and automatic updates.

The primary repository is [vndreiii/Prntscrape on GitHub](https://github.com/vndreiii/Prntscrape). GitHub Actions builds the app and publishes releases. [Forgejo](https://code.milfs.party/alex/Prntscrape) is a source backup.

## Downloads and updates

Download and extract the archive for your platform from [GitHub Releases](https://github.com/vndreiii/Prntscrape/releases). Put the executable in a folder your user can write to, so the app can replace it during updates.

- **Windows x64:** run `prntscrape.exe`. WebView2 Runtime is required. The startup notification appears above the taskbar at the bottom right; click it to open settings. The tray icon also opens settings.
- **Linux x64:** run `./prntscrape`. Builds target Ubuntu 24.04 or newer compatible distributions. Install GTK 3, WebKitGTK 4.1, and AppIndicator. Automatic screenshots currently require Hyprland and `grim`; the settings UI supports both X11 and Wayland. Settings open at startup; background tray access requires a desktop tray host.
- **macOS (Apple Silicon or Intel):** run `./prntscrape` and grant Screen Recording and Automation permissions. Open settings using the menu bar icon. These are unsigned binaries, not notarized app bundles; see [macOS instructions](MACOS_INSTRUCTIONS.md).

The app checks **only `vndreiii/Prntscrape` on GitHub** at startup and every six hours, downloading and installing newer stable releases automatically. Settings also has **Check for Updates** and displays errors or a **Restart** button after installation. Capturing continues until you restart. Offline checks or missing releases leave the current installation intact. Release downloads must be publicly accessible.

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
3. Create a matching tag, for example `v0.1.1`, and push it to both remotes.

```sh
git push origin main
git push forgejo main
git tag v0.1.1
git push origin v0.1.1
git push forgejo v0.1.1
```

GitHub Actions tests and builds all four targets on pushes to `main`, pull requests, and manual runs. A `v*` tag must match the package version; it publishes a GitHub Release only after every build succeeds. Archive names contain the full Rust target triple so the updater selects the right platform. Ordinary CI builds are downloadable as workflow artifacts and do not trigger app updates.
