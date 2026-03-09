use crate::error::{DetectorError, Result};
use crate::utils::path::path_is_dir;
use crate::{resource::Resource, utils::uri::Uri};
use walkdir::WalkDir;

pub struct RawfileDirectory {
    uri: Uri,
}

impl RawfileDirectory {
    pub fn locate(resource: &Resource) -> Result<Option<RawfileDirectory>> {
        let rawfile_directory_path = resource.uri().as_path().join("rawfile");
        if !path_is_dir(&rawfile_directory_path)? {
            return Ok(None);
        }

        Ok(Some(RawfileDirectory {
            uri: Uri::file(&rawfile_directory_path)?,
        }))
    }

    pub fn uri(&self) -> &Uri {
        &self.uri
    }

    pub fn find_all(&self) -> Result<Vec<Uri>> {
        let rawfile_directory = self.uri();
        let mut files = Vec::new();
        for entry in WalkDir::new(rawfile_directory.as_path()) {
            let entry = entry.map_err(DetectorError::walkdir)?;
            if entry.file_type().is_file() {
                files.push(Uri::file(entry.path())?);
            }
        }
        Ok(files)
    }
}
