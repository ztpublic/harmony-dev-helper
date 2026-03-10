use crate::error::{DetectorError, Result};
use crate::fs_discovery::find_matching_directories;
use crate::resource::Resource;
use crate::utils::path::{absolute_path, path_is_dir};
use crate::utils::qualifier::QualifierUtils;
use crate::utils::uri::Uri;
use serde_json::Value;
use std::path::{Path, PathBuf};

pub struct ResourceDirectory {
    path: PathBuf,
}

impl ResourceDirectory {
    pub fn find_all(resource: &Resource) -> Result<Vec<ResourceDirectory>> {
        find_matching_directories(resource.path(), is_resource_directory_name)?
            .into_iter()
            .map(|path| Ok(ResourceDirectory { path }))
            .collect()
    }

    pub fn load(resource_directory_path: impl AsRef<Path>) -> Result<Option<ResourceDirectory>> {
        let resource_directory_path = absolute_path(resource_directory_path.as_ref())?;
        if !path_is_dir(&resource_directory_path)? {
            return Err(DetectorError::ExpectedDirectory {
                path: resource_directory_path,
            });
        }

        let dir_name = resource_directory_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        if !is_resource_directory_name(dir_name) {
            return Ok(None);
        }
        Ok(Some(ResourceDirectory {
            path: resource_directory_path,
        }))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn uri(&self) -> Uri {
        Uri::from_absolute_path(self.path.clone())
    }

    pub fn qualifiers(&self) -> Value {
        let directory_name = self
            .path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        if directory_name == "base" || directory_name == "rawfile" || directory_name == "resfile" {
            Value::String(directory_name)
        } else {
            Value::Array(
                QualifierUtils::analyze_qualifier(directory_name)
                    .into_iter()
                    .map(|q| serde_json::to_value(q).unwrap_or(Value::Null))
                    .collect(),
            )
        }
    }
}

fn is_resource_directory_name(dir_name: &str) -> bool {
    dir_name == "base"
        || dir_name == "rawfile"
        || dir_name == "resfile"
        || !QualifierUtils::analyze_qualifier(dir_name.to_string()).is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn load_rejects_missing_directories() {
        let temp_dir = tempdir().unwrap();
        let missing_directory = temp_dir.path().join("base");

        let error = match ResourceDirectory::load(&missing_directory) {
            Ok(_) => panic!("expected a missing directory to be rejected"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            DetectorError::ExpectedDirectory { path } if path == missing_directory
        ));
    }

    #[test]
    fn load_returns_none_for_non_resource_directory_names() -> Result<()> {
        let temp_dir = tempdir().unwrap();
        let directory = temp_dir.path().join("not-a-resource-dir");
        std::fs::create_dir(&directory).unwrap();

        assert!(ResourceDirectory::load(&directory)?.is_none());
        Ok(())
    }
}
