use serde::{Deserialize, Serialize};
use directories::ProjectDirs;
use std::path::PathBuf;
use std::fs;

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
pub struct Config {
    pub mode: Mode,
    pub interval_minutes: u64,
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
            .and_then(|d| d.picture_dir().map(|p| p.join("Prntscrape").to_string_lossy().into_owned()))
            .unwrap_or_else(|| "~/Pictures/Prntscrape".to_string());

        Self {
            mode: Mode::Capture,
            interval_minutes: 2,
            capture_region: CaptureRegion::Window,
            save_directory,
            current_project: None,
            format: Format::Png,
            quality: 80,
            skip_unchanged: true,
            paused: false,
            watchlist: vec![
                "photoshop", "illustrator", "indesign", "premiere", "after effects",
                "lightroom", "xd", "animate", "figma", "penpot", "blender", "krita",
                "inkscape", "gimp", "affinity", "davinci resolve", "clip studio", "word"
            ].into_iter().map(String::from).collect(),
        }
    }
}

impl Config {
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

    fn config_path() -> PathBuf {
        let proj_dirs = ProjectDirs::from("com", "prntscrape", "Prntscrape")
            .expect("Could not find project directories");
        proj_dirs.config_dir().join("config.json")
    }
}
