use hdckit_rs::{Client as HdcClient, ClientOptions};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const DEVECO_DEFAULT_BIN_PATH: &str =
    "/Applications/DevEco-Studio.app/Contents/sdk/default/openharmony/toolchains/hdc";

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BinConfigSource {
    Custom,
    Path,
    Deveco,
    None,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BinConfigResult {
    pub custom_bin_path: Option<String>,
    pub resolved_bin_path: Option<String>,
    pub source: BinConfigSource,
    pub available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct PersistedBinConfig {
    custom_bin_path: Option<String>,
}

pub fn get_bin_config() -> BinConfigResult {
    resolve_bin_config(&read_persisted_config())
}

pub fn set_custom_bin_path(bin_path: Option<String>) -> Result<BinConfigResult, String> {
    let custom_bin_path = bin_path
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    let persisted = PersistedBinConfig { custom_bin_path };
    write_persisted_config(&persisted)?;

    Ok(resolve_bin_config(&persisted))
}

pub fn build_hdc_client_from_config() -> Result<HdcClient, String> {
    let config = get_bin_config();
    let resolved_bin = config.resolved_bin_path.clone().ok_or_else(|| {
        config.message.unwrap_or_else(|| {
            "HDC binary is not configured. Open settings and provide a valid path.".to_string()
        })
    })?;

    let mut options = ClientOptions::default();
    if let Ok(raw) = std::env::var("OHOS_HDC_SERVER_PORT") {
        if let Ok(port) = raw.parse::<u16>() {
            options.port = port;
        }
    }

    options.bin = PathBuf::from(resolved_bin);

    Ok(HdcClient::new(options))
}

fn resolve_bin_config(persisted: &PersistedBinConfig) -> BinConfigResult {
    if let Some(custom_value) = persisted.custom_bin_path.clone() {
        let custom_candidate = expand_user_home(&custom_value);
        match validate_hdc_candidate(&custom_candidate) {
            Ok(()) => {
                return BinConfigResult {
                    custom_bin_path: Some(custom_value),
                    resolved_bin_path: Some(render_path(&custom_candidate)),
                    source: BinConfigSource::Custom,
                    available: true,
                    message: None,
                }
            }
            Err(message) => {
                return BinConfigResult {
                    custom_bin_path: Some(custom_value),
                    resolved_bin_path: None,
                    source: BinConfigSource::Custom,
                    available: false,
                    message: Some(format!("Custom HDC path is invalid: {message}")),
                }
            }
        }
    }

    if let Some(path_candidate) = detect_hdc_in_path() {
        return BinConfigResult {
            custom_bin_path: None,
            resolved_bin_path: Some(render_path(&path_candidate)),
            source: BinConfigSource::Path,
            available: true,
            message: None,
        };
    }

    let deveco_candidate = PathBuf::from(DEVECO_DEFAULT_BIN_PATH);
    if validate_hdc_candidate(&deveco_candidate).is_ok() {
        return BinConfigResult {
            custom_bin_path: None,
            resolved_bin_path: Some(render_path(&deveco_candidate)),
            source: BinConfigSource::Deveco,
            available: true,
            message: None,
        };
    }

    BinConfigResult {
        custom_bin_path: None,
        resolved_bin_path: None,
        source: BinConfigSource::None,
        available: false,
        message: Some(
            "HDC binary not found in PATH or DevEco default path. Configure a custom path in settings."
                .to_string(),
        ),
    }
}

fn detect_hdc_in_path() -> Option<PathBuf> {
    let path_env = env::var_os("PATH")?;

    for directory in env::split_paths(&path_env) {
        let candidate = directory.join("hdc");
        if validate_hdc_candidate(&candidate).is_ok() {
            return Some(candidate);
        }
    }

    None
}

fn read_persisted_config() -> PersistedBinConfig {
    let Ok(path) = config_file_path() else {
        return PersistedBinConfig::default();
    };

    let contents = match fs::read_to_string(path) {
        Ok(value) => value,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return PersistedBinConfig::default()
        }
        Err(_) => return PersistedBinConfig::default(),
    };

    serde_json::from_str::<PersistedBinConfig>(&contents).unwrap_or_default()
}

fn write_persisted_config(config: &PersistedBinConfig) -> Result<(), String> {
    let path = config_file_path()?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create config dir {}: {error}", parent.display()))?;
    }

    let serialized = serde_json::to_string_pretty(config)
        .map_err(|error| format!("failed to serialize config: {error}"))?;

    fs::write(&path, serialized)
        .map_err(|error| format!("failed to write config {}: {error}", path.display()))
}

fn config_file_path() -> Result<PathBuf, String> {
    let home_dir = resolve_home_dir().ok_or_else(|| {
        "Unable to resolve user home directory for HDC config persistence".to_string()
    })?;

    Ok(home_dir.join(".harmony-dev-helper").join("hdc-bridge.json"))
}

fn resolve_home_dir() -> Option<PathBuf> {
    env::var_os("HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("USERPROFILE").map(PathBuf::from))
}

fn expand_user_home(path: &str) -> PathBuf {
    if path == "~" {
        return resolve_home_dir().unwrap_or_else(|| PathBuf::from(path));
    }

    if let Some(relative) = path.strip_prefix("~/") {
        if let Some(home_dir) = resolve_home_dir() {
            return home_dir.join(relative);
        }
    }

    PathBuf::from(path)
}

fn validate_hdc_candidate(path: &Path) -> Result<(), String> {
    let metadata = fs::metadata(path)
        .map_err(|error| format!("{} ({error})", path.display()))?;

    if !metadata.is_file() {
        return Err(format!("{} is not a file", path.display()));
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        if metadata.permissions().mode() & 0o111 == 0 {
            return Err(format!("{} is not executable", path.display()));
        }
    }

    Ok(())
}

fn render_path(path: &Path) -> String {
    fs::canonicalize(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .to_string()
}
