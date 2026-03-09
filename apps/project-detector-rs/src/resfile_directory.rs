use crate::error::{DetectorError, Result};
use crate::utils::path::path_is_dir;
use crate::{resource::Resource, utils::uri::Uri};
use walkdir::WalkDir;

pub struct ResfileDirectory {
    uri: Uri,
}

impl ResfileDirectory {
    pub fn locate(resource: &Resource) -> Result<Option<ResfileDirectory>> {
        let resfile_directory_path = resource.uri().as_path().join("resfile");
        if !path_is_dir(&resfile_directory_path)? {
            return Ok(None);
        }

        Ok(Some(ResfileDirectory {
            uri: Uri::file(&resfile_directory_path)?,
        }))
    }

    pub fn uri(&self) -> &Uri {
        &self.uri
    }

    pub fn find_all(&self) -> Result<Vec<Uri>> {
        let resfile_directory = self.uri();
        let mut files = Vec::new();
        for entry in WalkDir::new(resfile_directory.as_path()) {
            let entry = entry.map_err(DetectorError::walkdir)?;
            if entry.file_type().is_file() {
                files.push(Uri::file(entry.path())?);
            }
        }
        Ok(files)
    }
}
