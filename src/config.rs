use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Capture,
    Notify,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CaptureRegion {
    Window,
    Monitor,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    Png,
    Jpeg,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub mode: Mode,
    pub interval_secs: u64,
    pub capture_region: CaptureRegion,
    pub save_directory: String,
    pub current_project: Option<String>,
    pub format: Format,
    pub quality: u8,
    pub skip_unchanged: bool,
    pub paused: bool,
    pub watchlist: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        let save_directory = directories::UserDirs::new()
            .and_then(|d| {
                d.picture_dir()
                    .map(|p| p.join("Prntscrape").to_string_lossy().into_owned())
            })
            .unwrap_or_else(|| "~/Pictures/Prntscrape".to_string());

        Self {
            mode: Mode::Capture,
            interval_secs: 120,
            capture_region: CaptureRegion::Window,
            save_directory,
            current_project: None,
            format: Format::Png,
            quality: 80,
            skip_unchanged: true,
            paused: false,
            watchlist: vec![
                "photoshop",
                "illustrator",
                "indesign",
                "premiere",
                "after effects",
                "lightroom",
                "xd",
                "animate",
                "figma",
                "penpot",
                "blender",
                "krita",
                "inkscape",
                "gimp",
                "affinity",
                "davinci resolve",
                "clip studio",
                "word",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
        }
    }
}

impl Config {
    pub fn validate(&self) -> Result<(), String> {
        if !(1..=86400).contains(&self.interval_secs) {
            return Err("Interval must be between 1 and 86400 seconds.".into());
        }
        if !(1..=100).contains(&self.quality) {
            return Err("JPEG quality must be between 1 and 100.".into());
        }
        if self.save_directory.trim().is_empty() {
            return Err("Choose a save folder.".into());
        }
        Ok(())
    }

    pub fn load_or_default() -> Self {
        let config_path = Self::config_path();
        if let Ok(content) = fs::read_to_string(&config_path) {
            if let Ok(config) = serde_json::from_str(&content) {
                return config;
            }
        }

        let default_config = Self::default();
        let _ = default_config.save();
        default_config
    }

    pub fn save(&self) -> std::io::Result<()> {
        let config_path = Self::config_path();
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        fs::write(config_path, content)
    }

    pub fn config_path() -> PathBuf {
        if let Some(proj_dirs) = ProjectDirs::from("com", "prntscrape", "Prntscrape") {
            proj_dirs.config_dir().join("config.json")
        } else {
            let home = directories::UserDirs::new()
                .map(|u| u.home_dir().to_path_buf())
                .unwrap_or_else(|| PathBuf::from("."));
            home.join(".prntscrape").join("config.json")
        }
    }
}
