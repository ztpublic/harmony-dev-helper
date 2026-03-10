use std::path::{Path, PathBuf};

use indexmap::IndexMap;

use crate::error::ImageManagerError;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlatIni {
    entries: IndexMap<String, String>,
}

impl FlatIni {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_entries(entries: IndexMap<String, String>) -> Self {
        Self { entries }
    }

    pub fn entries(&self) -> &IndexMap<String, String> {
        &self.entries
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries.get(key).map(String::as_str)
    }

    pub fn insert<K, V>(&mut self, key: K, value: V)
    where
        K: Into<String>,
        V: Into<String>,
    {
        self.entries.insert(key.into(), value.into());
    }

    pub fn parse(path: impl AsRef<Path>, contents: &str) -> Result<Self, ImageManagerError> {
        let path = path.as_ref().to_path_buf();
        let mut entries = IndexMap::new();

        for (line_number, line) in contents.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }

            let Some((key, value)) = line.split_once('=') else {
                return Err(ImageManagerError::Ini {
                    path: path.clone(),
                    message: format!("line {} is missing '='", line_number + 1),
                });
            };

            let key = key.trim();
            if key.is_empty() {
                return Err(ImageManagerError::Ini {
                    path: path.clone(),
                    message: format!("line {} has an empty key", line_number + 1),
                });
            }

            entries.insert(key.to_string(), value.trim().to_string());
        }

        Ok(Self { entries })
    }

    pub fn to_ini_string(&self) -> String {
        let mut output = String::new();
        for (key, value) in &self.entries {
            output.push_str(key);
            output.push('=');
            output.push_str(value);
            output.push('\n');
        }
        output
    }

    pub async fn read_from_path(path: impl AsRef<Path>) -> Result<Self, ImageManagerError> {
        let path = path.as_ref().to_path_buf();
        let contents =
            tokio::fs::read_to_string(&path)
                .await
                .map_err(|source| ImageManagerError::Io {
                    path: path.clone(),
                    source,
                })?;
        Self::parse(path, &contents)
    }

    pub async fn write_to_path(&self, path: impl AsRef<Path>) -> Result<(), ImageManagerError> {
        let path = path.as_ref().to_path_buf();
        ensure_parent_dir(&path).await?;
        tokio::fs::write(&path, self.to_ini_string())
            .await
            .map_err(|source| ImageManagerError::Io { path, source })
    }
}

async fn ensure_parent_dir(path: &Path) -> Result<(), ImageManagerError> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    tokio::fs::create_dir_all(parent)
        .await
        .map_err(|source| ImageManagerError::Io {
            path: PathBuf::from(parent),
            source,
        })
}
