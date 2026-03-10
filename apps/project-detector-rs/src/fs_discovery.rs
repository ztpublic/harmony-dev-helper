use crate::error::{DetectorError, Result};
use crate::utils::path::path_is_dir;
use crate::utils::uri::Uri;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

pub(crate) fn locate_subdirectory(parent: &Path, name: &str) -> Result<Option<Uri>> {
    let subdirectory_path = parent.join(name);
    if !path_is_dir(&subdirectory_path)? {
        return Ok(None);
    }

    Ok(Some(Uri::file(&subdirectory_path)?))
}

pub(crate) fn find_immediate_files(directory: &Path) -> Result<Vec<Uri>> {
    let mut files = Vec::new();
    let entries = fs::read_dir(directory).map_err(|source| DetectorError::io(directory, source))?;

    for entry in entries {
        let entry = entry.map_err(|source| DetectorError::io(directory, source))?;
        let path = entry.path();
        let metadata = entry
            .metadata()
            .map_err(|source| DetectorError::io(path.clone(), source))?;
        if metadata.is_file() {
            files.push(Uri::file(&path)?);
        }
    }

    Ok(files)
}

pub(crate) fn find_matching_directories(
    directory: &Path,
    accept: impl Fn(&str) -> bool,
) -> Result<Vec<Uri>> {
    let mut directories = Vec::new();
    let entries = fs::read_dir(directory).map_err(|source| DetectorError::io(directory, source))?;

    for entry in entries {
        let entry = entry.map_err(|source| DetectorError::io(directory, source))?;
        let path = entry.path();
        let metadata = entry
            .metadata()
            .map_err(|source| DetectorError::io(path.clone(), source))?;
        if !metadata.is_dir() {
            continue;
        }

        let directory_name = entry.file_name().to_string_lossy().to_string();
        if accept(&directory_name) {
            directories.push(Uri::file(&path)?);
        }
    }

    Ok(directories)
}

pub(crate) fn find_recursive_files(directory: &Path) -> Result<Vec<Uri>> {
    let mut files = Vec::new();

    for entry in WalkDir::new(directory) {
        let entry = entry.map_err(DetectorError::walkdir)?;
        if entry.file_type().is_file() {
            files.push(Uri::file(entry.path())?);
        }
    }

    Ok(files)
}
