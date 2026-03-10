use crate::error::{DetectorError, Result};
use std::env;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

pub fn absolute_path(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }

    let cwd = env::current_dir().map_err(|source| DetectorError::io(".", source))?;
    Ok(path_clean::clean(cwd.join(path)))
}

pub fn canonicalize(path: &Path) -> Result<PathBuf> {
    fs::canonicalize(path).map_err(|source| DetectorError::io(path.to_path_buf(), source))
}

pub fn path_is_dir(path: &Path) -> Result<bool> {
    match fs::metadata(path) {
        Ok(metadata) => Ok(metadata.is_dir()),
        Err(source) if source.kind() == ErrorKind::NotFound => Ok(false),
        Err(source) => Err(DetectorError::io(path.to_path_buf(), source)),
    }
}

pub fn path_is_file(path: &Path) -> Result<bool> {
    match fs::metadata(path) {
        Ok(metadata) => Ok(metadata.is_file()),
        Err(source) if source.kind() == ErrorKind::NotFound => Ok(false),
        Err(source) => Err(DetectorError::io(path.to_path_buf(), source)),
    }
}

pub fn read_to_string(path: &Path) -> Result<String> {
    fs::read_to_string(path).map_err(|source| DetectorError::io(path.to_path_buf(), source))
}

pub fn resolve_within(base: &Path, child: &Path) -> Result<PathBuf> {
    let base = canonicalize(base)?;
    let candidate = if child.is_absolute() {
        path_clean::clean(child)
    } else {
        path_clean::clean(base.join(child))
    };

    if !candidate.starts_with(&base) {
        return Err(DetectorError::PathEscapesBase { base, candidate });
    }

    Ok(candidate)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn resolve_within_normalizes_relative_segments_inside_the_base() -> Result<()> {
        let temp_dir = tempdir().unwrap();
        let base = temp_dir.path().join("workspace");
        std::fs::create_dir_all(base.join("entry/src")).unwrap();

        let resolved = resolve_within(&base, Path::new("entry/./nested/../src"))?;

        assert_eq!(resolved, canonicalize(&base)?.join("entry/src"));
        Ok(())
    }

    #[test]
    fn resolve_within_rejects_absolute_paths_outside_the_base() -> Result<()> {
        let temp_dir = tempdir().unwrap();
        let base = temp_dir.path().join("workspace");
        let outside = temp_dir.path().join("outside");
        std::fs::create_dir_all(&base).unwrap();
        std::fs::create_dir_all(&outside).unwrap();

        let error = resolve_within(&base, &outside).unwrap_err();
        assert!(matches!(
            error,
            DetectorError::PathEscapesBase {
                base: error_base,
                candidate
            } if error_base == canonicalize(&base)? && candidate == path_clean::clean(&outside)
        ));
        Ok(())
    }
}
