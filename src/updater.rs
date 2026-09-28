//! Install newer stable releases from the primary GitHub repository.
use serde::Serialize;

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

/// Never blocks the UI. self_update handles replacement of a running Windows
/// executable; the new binary is used on the next launch on every platform.
pub fn check(report: impl Fn(UpdateStatus) + Send + 'static) {
    std::thread::spawn(move || {
        report(UpdateStatus::new(
            "checking",
            "Checking GitHub for updates…",
        ));
        let (owner, name) = REPOSITORY.split_once('/').expect("owner/repository");
        let result = self_update::backends::github::Update::configure()
            .repo_owner(owner)
            .repo_name(name)
            .bin_name("prntscrape")
            .current_version(env!("CARGO_PKG_VERSION"))
            .show_download_progress(false)
            .show_output(false)
            .no_confirm(true)
            .build()
            .and_then(|update| update.update());
        let status = match result {
            Ok(status) if status.updated() => UpdateStatus::new(
                "updated",
                format!(
                    "Version {} installed. Restart Prntscrape to use it.",
                    status.version()
                ),
            ),
            Ok(_) => UpdateStatus::new("current", "Prntscrape is up to date."),
            Err(error) => {
                eprintln!("Update check failed: {error}");
                UpdateStatus::new("error", format!("Could not update: {error}"))
            }
        };
        report(status);
    });
}

#[cfg(test)]
mod tests {
    #[test]
    fn release_assets_match_updater_target_selection() {
        let workflow = include_str!("../.github/workflows/release.yml");
        for target in [
            "x86_64-pc-windows-msvc",
            "x86_64-unknown-linux-gnu",
            "aarch64-apple-darwin",
            "x86_64-apple-darwin",
        ] {
            let extension = if target.contains("windows") {
                "zip"
            } else {
                "tar.gz"
            };
            assert!(workflow.contains(&format!("archive: prntscrape-{target}.{extension}")));
        }
    }
}
