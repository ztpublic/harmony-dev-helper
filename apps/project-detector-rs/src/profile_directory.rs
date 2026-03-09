use crate::error::{DetectorError, Result};
use crate::utils::path::path_is_dir;
use crate::{resource_directory::ResourceDirectory, utils::uri::Uri};
use std::fs;

pub struct ProfileDirectory {
    uri: Uri,
}

impl ProfileDirectory {
    pub fn locate(resource_directory: &ResourceDirectory) -> Result<Option<ProfileDirectory>> {
        let profile_directory_path = resource_directory.uri().as_path().join("profile");
        if !path_is_dir(&profile_directory_path)? {
            return Ok(None);
        }

        Ok(Some(Self {
            uri: Uri::file(&profile_directory_path)?,
        }))
    }

    pub fn uri(&self) -> &Uri {
        &self.uri
    }

    pub fn find_all(&self) -> Result<Vec<Uri>> {
        let mut profile_directories = Vec::new();
        let profiles = fs::read_dir(self.uri.as_path())
            .map_err(|source| DetectorError::io(self.uri.as_path(), source))?;

        for profile in profiles {
            let profile =
                profile.map_err(|source| DetectorError::io(self.uri.as_path(), source))?;
            let path = profile.path();
            let metadata = profile
                .metadata()
                .map_err(|source| DetectorError::io(path.clone(), source))?;
            if metadata.is_file() {
                profile_directories.push(Uri::file(&path)?);
            }
        }

        Ok(profile_directories)
    }
}
