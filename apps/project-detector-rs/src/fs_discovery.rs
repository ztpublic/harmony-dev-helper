use crate::error::{DetectorError, Result};
use crate::utils::path::path_is_dir;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub(crate) fn locate_subdirectory(parent: &Path, name: &str) -> Result<Option<PathBuf>> {
    let subdirectory_path = parent.join(name);
    if !path_is_dir(&subdirectory_path)? {
        return Ok(None);
    }

    Ok(Some(subdirectory_path))
}

pub(crate) fn find_immediate_files(directory: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let entries = fs::read_dir(directory).map_err(|source| DetectorError::io(directory, source))?;

    for entry in entries {
        let entry = entry.map_err(|source| DetectorError::io(directory, source))?;
        let path = entry.path();
        let metadata = entry
            .metadata()
            .map_err(|source| DetectorError::io(path.clone(), source))?;
        if metadata.is_file() {
            files.push(path);
        }
    }

    Ok(files)
}

pub(crate) fn find_matching_directories(
    directory: &Path,
    accept: impl Fn(&str) -> bool,
) -> Result<Vec<PathBuf>> {
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
            directories.push(path);
        }
    }

    Ok(directories)
}

pub(crate) fn find_recursive_files(directory: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    for entry in WalkDir::new(directory) {
        let entry = entry.map_err(DetectorError::walkdir)?;
        if entry.file_type().is_file() {
            files.push(entry.path().to_path_buf());
        }
    }

    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    #[cfg(unix)]
    struct PermissionRestore {
        path: PathBuf,
        mode: u32,
    }

    #[cfg(unix)]
    impl PermissionRestore {
        fn lock(path: &Path) -> Self {
            let mode = std::fs::metadata(path).unwrap().permissions().mode();
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o000)).unwrap();
            Self {
                path: path.to_path_buf(),
                mode,
            }
        }
    }

    #[cfg(unix)]
    impl Drop for PermissionRestore {
        fn drop(&mut self) {
            let _ =
                std::fs::set_permissions(&self.path, std::fs::Permissions::from_mode(self.mode));
        }
    }

    #[test]
    fn find_immediate_files_reports_missing_directories() {
        let temp_dir = tempdir().unwrap();
        let missing_directory = temp_dir.path().join("missing");

        let error = find_immediate_files(&missing_directory).unwrap_err();
        assert!(matches!(
            error,
            DetectorError::Io { path, .. } if path == missing_directory
        ));
    }

    #[cfg(unix)]
    #[test]
    fn find_matching_directories_propagates_read_dir_failures() {
        let temp_dir = tempdir().unwrap();
        let locked_directory = temp_dir.path().join("locked");
        std::fs::create_dir(&locked_directory).unwrap();
        let _restore = PermissionRestore::lock(&locked_directory);

        let error = find_matching_directories(&locked_directory, |_| true).unwrap_err();
        assert!(matches!(
            error,
            DetectorError::Io { path, .. } if path == locked_directory
        ));
    }

    #[cfg(unix)]
    #[test]
    fn find_recursive_files_propagates_walkdir_failures() {
        let temp_dir = tempdir().unwrap();
        let locked_directory = temp_dir.path().join("locked");
        std::fs::create_dir(&locked_directory).unwrap();
        let _restore = PermissionRestore::lock(&locked_directory);

        let error = find_recursive_files(&locked_directory).unwrap_err();
        assert!(matches!(
            error,
            DetectorError::WalkDir { path, .. } if path == locked_directory
        ));
    }
}
