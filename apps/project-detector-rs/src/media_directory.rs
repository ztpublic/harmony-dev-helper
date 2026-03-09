use crate::error::{DetectorError, Result};
use crate::utils::path::path_is_dir;
use crate::{resource_directory::ResourceDirectory, utils::uri::Uri};
use std::fs;

pub struct MediaDirectory {
    uri: Uri,
}

impl MediaDirectory {
    pub fn locate(resource_directory: &ResourceDirectory) -> Result<Option<MediaDirectory>> {
        let media_directory_path = resource_directory.uri().as_path().join("media");
        if !path_is_dir(&media_directory_path)? {
            return Ok(None);
        }

        Ok(Some(Self {
            uri: Uri::file(&media_directory_path)?,
        }))
    }

    pub fn uri(&self) -> &Uri {
        &self.uri
    }

    pub fn find_all(&self) -> Result<Vec<Uri>> {
        let mut media_files = Vec::new();
        let media_directory = self.uri();
        let dirs = fs::read_dir(media_directory.as_path())
            .map_err(|source| DetectorError::io(media_directory.as_path(), source))?;

        for dir in dirs {
            let dir = dir.map_err(|source| DetectorError::io(media_directory.as_path(), source))?;
            let path = dir.path();
            let metadata = dir
                .metadata()
                .map_err(|source| DetectorError::io(path.clone(), source))?;
            if metadata.is_file() {
                media_files.push(Uri::file(&path)?);
            }
        }

        Ok(media_files)
    }
}
