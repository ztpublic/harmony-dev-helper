use crate::error::{DetectorError, Result};
use crate::resource::Resource;
use crate::utils::path::path_is_dir;
use crate::utils::qualifier::utils_impl::QualifierUtils;
use crate::utils::uri::Uri;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct ResourceDirectory {
    uri: Uri,
    resource: Arc<Resource>,
}

impl ResourceDirectory {
    pub fn find_all(resource: &Arc<Resource>) -> Result<Vec<Arc<ResourceDirectory>>> {
        let mut resource_directories = Vec::new();
        let resource_directory = resource.get_uri();

        let dirs = fs::read_dir(resource_directory.fs_path())
            .map_err(|source| DetectorError::io(resource_directory.fs_path(), source))?;

        for dir in dirs {
            let dir =
                dir.map_err(|source| DetectorError::io(resource_directory.fs_path(), source))?;
            let path = dir.path();
            let metadata = dir
                .metadata()
                .map_err(|source| DetectorError::io(path.clone(), source))?;
            if !metadata.is_dir() {
                continue;
            }

            let dir_name = dir.file_name().to_string_lossy().to_string();
            if !is_resource_directory_name(&dir_name) {
                continue;
            }

            resource_directories.push(Arc::new(ResourceDirectory {
                uri: Uri::file(&path)?,
                resource: Arc::clone(resource),
            }))
        }

        Ok(resource_directories)
    }

    pub fn create(
        resource: &Arc<Resource>,
        resource_directory_uri: String,
    ) -> Result<Option<Arc<ResourceDirectory>>> {
        let resource_directory_path = PathBuf::from(&resource_directory_uri);
        if !path_is_dir(&resource_directory_path)? {
            return Err(DetectorError::ExpectedDirectory {
                path: resource_directory_path,
            });
        }

        let uri = Uri::file(&resource_directory_path)?;
        let dir_name = Uri::base_name(&uri);
        if !is_resource_directory_name(&dir_name) {
            return Ok(None);
        }
        Ok(Some(Arc::new(ResourceDirectory {
            uri,
            resource: Arc::clone(resource),
        })))
    }

    pub fn get_uri(&self) -> Uri {
        self.uri.clone()
    }

    pub fn get_resource(&self) -> Arc<Resource> {
        Arc::clone(&self.resource)
    }

    pub fn get_qualifiers(&self) -> Value {
        let directory_name = Path::new(&self.uri.fs_path())
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
