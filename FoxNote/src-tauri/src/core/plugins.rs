use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};
use thiserror::Error;

const PLUGINS_CONFIG_VERSION: u32 = 1;
const PLUGINS_CONFIG_FILE: &str = "plugins.toml";

#[derive(Debug, Error)]
pub enum PluginError {
    #[error("plugin id is invalid: '{0}'")]
    InvalidPluginId(String),
    #[error("plugin source is invalid")]
    InvalidSource,
    #[error("plugin '{0}' already exists")]
    AlreadyExists(String),
    #[error("plugin '{0}' not found")]
    NotFound(String),
    #[error("failed to parse plugins config: {0}")]
    ParseConfig(#[from] toml::de::Error),
    #[error("failed to serialize plugins config: {0}")]
    SerializeConfig(#[from] toml::ser::Error),
    #[error("io failure: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginSourceKind {
    Local,
    Remote,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginEntry {
    pub id: String,
    pub name: String,
    pub source_kind: PluginSourceKind,
    pub source: String,
    pub enabled: bool,
    pub installed_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallPluginInput {
    pub id: String,
    pub name: String,
    pub source_kind: PluginSourceKind,
    pub source: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct PluginsConfigFile {
    version: u32,
    plugins: Vec<PluginEntry>,
}

#[derive(Debug)]
pub struct PluginService {
    root_dir: PathBuf,
}

impl PluginService {
    pub fn new(root_dir: impl Into<PathBuf>) -> Result<Self, PluginError> {
        let root_dir = root_dir.into();
        fs::create_dir_all(&root_dir)?;

        let service = Self { root_dir };
        service.ensure_config_exists()?;
        Ok(service)
    }

    pub fn config_path_string(&self) -> String {
        self.config_path().to_string_lossy().into_owned()
    }

    pub fn list_plugins(&self) -> Result<Vec<PluginEntry>, PluginError> {
        let mut plugins = self.read_config()?.plugins;
        plugins.sort_by(|left, right| left.name.cmp(&right.name).then(left.id.cmp(&right.id)));
        Ok(plugins)
    }

    pub fn install_plugin(&self, input: InstallPluginInput) -> Result<PluginEntry, PluginError> {
        let id = sanitize_plugin_id(&input.id)
            .ok_or_else(|| PluginError::InvalidPluginId(input.id.clone()))?;
        let name = input.name.trim();
        if name.is_empty() {
            return Err(PluginError::InvalidPluginId(input.id));
        }

        let source = input.source.trim();
        if source.is_empty() {
            return Err(PluginError::InvalidSource);
        }

        let mut config = self.read_config()?;
        if config.plugins.iter().any(|entry| entry.id == id) {
            return Err(PluginError::AlreadyExists(id));
        }

        let entry = PluginEntry {
            id: id.clone(),
            name: name.to_string(),
            source_kind: input.source_kind,
            source: source.to_string(),
            enabled: true,
            installed_at: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        };

        config.plugins.push(entry.clone());
        self.write_config(&config)?;
        Ok(entry)
    }

    pub fn set_enabled(&self, id: &str, enabled: bool) -> Result<PluginEntry, PluginError> {
        let normalized =
            sanitize_plugin_id(id).ok_or_else(|| PluginError::InvalidPluginId(id.to_string()))?;

        let mut config = self.read_config()?;
        let entry = config
            .plugins
            .iter_mut()
            .find(|entry| entry.id == normalized)
            .ok_or_else(|| PluginError::NotFound(normalized.clone()))?;

        entry.enabled = enabled;
        let updated = entry.clone();
        self.write_config(&config)?;
        Ok(updated)
    }

    pub fn remove_plugin(&self, id: &str) -> Result<(), PluginError> {
        let normalized =
            sanitize_plugin_id(id).ok_or_else(|| PluginError::InvalidPluginId(id.to_string()))?;

        let mut config = self.read_config()?;
        let before = config.plugins.len();
        config.plugins.retain(|entry| entry.id != normalized);
        if config.plugins.len() == before {
            return Err(PluginError::NotFound(normalized));
        }

        self.write_config(&config)?;
        Ok(())
    }

    fn ensure_config_exists(&self) -> Result<(), PluginError> {
        if self.config_path().exists() {
            return Ok(());
        }

        self.write_config(&PluginsConfigFile {
            version: PLUGINS_CONFIG_VERSION,
            plugins: Vec::new(),
        })
    }

    fn read_config(&self) -> Result<PluginsConfigFile, PluginError> {
        let path = self.config_path();
        if !path.exists() {
            return Ok(PluginsConfigFile {
                version: PLUGINS_CONFIG_VERSION,
                plugins: Vec::new(),
            });
        }

        let text = fs::read_to_string(path)?;
        if text.trim().is_empty() {
            return Ok(PluginsConfigFile {
                version: PLUGINS_CONFIG_VERSION,
                plugins: Vec::new(),
            });
        }

        let parsed: PluginsConfigFile = toml::from_str(&text)?;
        Ok(parsed)
    }

    fn write_config(&self, config: &PluginsConfigFile) -> Result<(), PluginError> {
        fs::create_dir_all(&self.root_dir)?;
        let text = toml::to_string_pretty(config)?;
        fs::write(self.config_path(), text)?;
        Ok(())
    }

    fn config_path(&self) -> PathBuf {
        self.root_dir.join(PLUGINS_CONFIG_FILE)
    }
}

fn sanitize_plugin_id(raw: &str) -> Option<String> {
    let cleaned = raw
        .trim()
        .to_lowercase()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>();

    let compact = cleaned
        .split('-')
        .filter(|chunk| !chunk.is_empty())
        .collect::<Vec<&str>>()
        .join("-");

    if compact.is_empty() {
        None
    } else {
        Some(compact)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_toggle_remove_plugin_roundtrip() {
        let temp = tempfile::tempdir().expect("tempdir");
        let service = PluginService::new(temp.path()).expect("service");

        let installed = service
            .install_plugin(InstallPluginInput {
                id: "my-canvas".to_string(),
                name: "My Canvas".to_string(),
                source_kind: PluginSourceKind::Local,
                source: "./plugins/my-canvas".to_string(),
            })
            .expect("install");

        assert_eq!(installed.id, "my-canvas");
        assert!(installed.enabled);

        let disabled = service.set_enabled("my-canvas", false).expect("toggle off");
        assert!(!disabled.enabled);

        let list = service.list_plugins().expect("list");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, "my-canvas");

        service.remove_plugin("my-canvas").expect("remove");
        let list = service.list_plugins().expect("list empty");
        assert!(list.is_empty());
    }

    #[test]
    fn normalize_plugin_id() {
        assert_eq!(
            sanitize_plugin_id("  Fancy Plugin  "),
            Some("fancy-plugin".to_string())
        );
        assert_eq!(sanitize_plugin_id("***"), None);
    }
}
