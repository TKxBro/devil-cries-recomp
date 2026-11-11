use std::path::PathBuf;
use serde::{Deserialize, Serialize};

#[allow(dead_code)]
pub const DMC3_TITLE_ID: u64 = 0x01007B600D5BC000;

#[derive(Clone, Serialize, Deserialize)]
pub struct Config {
    pub game_source: Option<PathBuf>,
    pub output_dir: PathBuf,
    pub saves_dir: PathBuf,
    pub fullscreen: bool,
    pub h264_backend: String,
    pub thread_type: String,
    pub thread_count: usize,
    pub enable_stats: bool,
}

impl Default for Config {
    fn default() -> Self {
        let base_dir = default_app_dir();

        // Auto-detect existing local build in workspace
        let local_candidates = [
            PathBuf::from("out-graphics-format-audio7"),
            PathBuf::from("out-base-no-flush-audio7"),
            PathBuf::from("out-base"),
        ];

        let mut output_dir = base_dir.join("build");
        let mut saves_dir = base_dir.join("saves");

        for candidate in local_candidates {
            if candidate.join("base").is_file() {
                output_dir = candidate;
                let local_saves = PathBuf::from("../../dmc3-acceptance-saves");
                if local_saves.is_dir() {
                    saves_dir = local_saves;
                }
                break;
            }
        }

        Self {
            game_source: None,
            output_dir,
            saves_dir,
            fullscreen: false,
            h264_backend: "ffmpeg".to_string(),
            thread_type: "slice".to_string(),
            thread_count: 4,
            enable_stats: false,
        }
    }
}

pub fn default_app_dir() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("DevilCriesRecomp")
}

impl Config {
    pub fn config_path() -> PathBuf {
        default_app_dir().join("config.json")
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if let Ok(data) = std::fs::read_to_string(&path) {
            if let Ok(mut cfg) = serde_json::from_str::<Config>(&data) {
                // If the configured path doesn't exist, try local fallback
                if !cfg.is_installed() {
                    let local = PathBuf::from("out-graphics-format-audio7");
                    if local.join("base").is_file() {
                        cfg.output_dir = local;
                    }
                }
                return cfg;
            }
        }
        Self::default()
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let data = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(&path, data).map_err(|e| e.to_string())
    }

    pub fn executable_path(&self) -> PathBuf {
        self.output_dir.join("base")
    }

    pub fn is_installed(&self) -> bool {
        let exe = self.executable_path();
        exe.is_file()
    }
}
