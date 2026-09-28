//! Package-aware updates from the primary GitHub repository.
mod install;
pub use install::{launch_path, restart};

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{fs, io::Read, path::Path};

pub const REPOSITORY: &str = "vndreiii/Prntscrape";

#[derive(Clone, Debug, Serialize)]
pub struct UpdateStatus {
    pub phase: &'static str,
    pub message: String,
}

impl UpdateStatus {
    pub fn new(phase: &'static str, message: impl Into<String>) -> Self {
        Self {
            phase,
            message: message.into(),
        }
    }
}

fn asset_name(target: &str) -> Option<String> {
    let extension = match target {
        "x86_64-pc-windows-msvc" => "exe",
        "x86_64-unknown-linux-gnu" => "AppImage",
        "aarch64-apple-darwin" | "x86_64-apple-darwin" => "app.zip",
        _ => return None,
    };
    Some(format!("Prntscrape-{target}.{extension}"))
}

fn verify_checksum(
    package: &Path,
    checksums: &str,
    asset: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let expected = checksums
        .lines()
        .find_map(|line| {
            let (hash, filename) = line.split_once(' ')?;
            (filename.trim_start().trim_start_matches('*') == asset).then_some(hash)
        })
        .ok_or("Release is missing the checksum for this platform")?;
    let mut file = fs::File::open(package)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0; 65536];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    if format!("{:x}", hasher.finalize()) != expected {
        return Err(
            "Downloaded package failed its SHA-256 check; the installed app was left untouched."
                .into(),
        );
    }
    Ok(())
}

fn download(
    asset: &self_update::update::ReleaseAsset,
    path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut file = fs::File::create(path)?;
    self_update::Download::from_url(&asset.download_url)
        .set_header("accept".parse()?, "application/octet-stream".parse()?)
        .download_to(&mut file)?;
    file.sync_all()?;
    Ok(())
}

fn update(report: &impl Fn(UpdateStatus)) -> Result<Option<String>, Box<dyn std::error::Error>> {
    let (owner, name) = REPOSITORY.split_once('/').expect("owner/repository");
    let backend = self_update::backends::github::Update::configure()
        .repo_owner(owner)
        .repo_name(name)
        .bin_name("prntscrape")
        .current_version(env!("CARGO_PKG_VERSION"))
        .build()?;
    let release = backend.get_latest_release()?;
    if !self_update::version::bump_is_greater(env!("CARGO_PKG_VERSION"), &release.version)? {
        return Ok(None);
    }
    let filename = asset_name(self_update::get_target())
        .ok_or("No portable release is available for this platform")?;
    let asset = release
        .assets
        .iter()
        .find(|asset| asset.name == filename)
        .ok_or("Latest release is missing this platform's package")?;
    let checksums = release
        .assets
        .iter()
        .find(|asset| asset.name == "SHA256SUMS")
        .ok_or("Latest release is missing SHA256SUMS")?;
    let temp = tempfile::tempdir()?;
    let package_path = temp.path().join(&filename);
    let checksums_path = temp.path().join("SHA256SUMS");
    report(UpdateStatus::new(
        "downloading",
        format!("Downloading version {}…", release.version),
    ));
    download(checksums, &checksums_path)?;
    download(asset, &package_path)?;
    verify_checksum(
        &package_path,
        &fs::read_to_string(checksums_path)?,
        &filename,
    )?;
    report(UpdateStatus::new("installing", "Installing update…"));
    install::install(&package_path)?;
    Ok(Some(release.version))
}

/// Network, verification and replacement all happen off the UI thread.
pub fn check(report: impl Fn(UpdateStatus) + Send + 'static) {
    std::thread::spawn(move || {
        report(UpdateStatus::new(
            "checking",
            "Checking GitHub for updates…",
        ));
        let status = match update(&report) {
            Ok(Some(version)) => UpdateStatus::new(
                "updated",
                format!("Version {version} installed. Restart Prntscrape to use it."),
            ),
            Ok(None) => UpdateStatus::new("current", "Prntscrape is up to date."),
            Err(error) => {
                eprintln!("Update failed: {error}");
                UpdateStatus::new("error", format!("Could not update: {error}"))
            }
        };
        report(status);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packaged_assets_match_release_validation() {
        let workflow = include_str!("../.github/workflows/release.yml");
        for target in [
            "x86_64-pc-windows-msvc",
            "x86_64-unknown-linux-gnu",
            "aarch64-apple-darwin",
            "x86_64-apple-darwin",
        ] {
            let name = asset_name(target).unwrap();
            assert!(workflow.contains(&format!("test -s release-assets/{name}")));
        }
        assert!(asset_name("unsupported-target").is_none());
    }

    #[test]
    fn checksum_rejects_corruption_and_missing_assets() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("app");
        fs::write(&path, b"abc").unwrap();
        let checksums = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad  app";
        assert!(verify_checksum(&path, checksums, "app").is_ok());
        assert!(verify_checksum(&path, checksums, "another-app").is_err());
        fs::write(&path, b"corrupted").unwrap();
        assert!(verify_checksum(&path, checksums, "app").is_err());
    }
}
