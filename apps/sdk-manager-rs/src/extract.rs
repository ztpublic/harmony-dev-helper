use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

use flate2::read::GzDecoder;
use tar::Archive;
use walkdir::WalkDir;
use zip::ZipArchive;

use crate::progress::{ProgressKind, ProgressReporter};
use crate::{ProgressEvent, SdkManagerError, SdkOs};

const S_IFMT: u32 = 0o170000;
const S_IFLNK: u32 = 0o120000;

pub(crate) fn extract_sdk_archives<F>(
    archive_path: &Path,
    staging_dir: &Path,
    target_dir: &Path,
    host_os: SdkOs,
    on_progress: &mut F,
) -> Result<(), SdkManagerError>
where
    F: FnMut(ProgressEvent),
{
    reset_directory(staging_dir)?;
    fs::create_dir_all(target_dir).map_err(|source| SdkManagerError::Io {
        path: target_dir.to_path_buf(),
        source,
    })?;

    extract_tar_to_staging(archive_path, staging_dir)?;
    let zip_paths = select_nested_archives(staging_dir, host_os)?;
    let total_bytes = total_zip_bytes(&zip_paths)?;
    let mut reporter = ProgressReporter::new(ProgressKind::Extract, Some(total_bytes), false);
    let mut written_bytes = 0_u64;

    for zip_path in zip_paths {
        extract_nested_zip(
            &zip_path,
            target_dir,
            &mut reporter,
            &mut written_bytes,
            on_progress,
        )?;
    }

    reporter.finish(written_bytes, on_progress);
    Ok(())
}

fn extract_tar_to_staging(archive_path: &Path, staging_dir: &Path) -> Result<(), SdkManagerError> {
    let archive_file = File::open(archive_path).map_err(|source| SdkManagerError::Io {
        path: archive_path.to_path_buf(),
        source,
    })?;
    let decoder = GzDecoder::new(archive_file);
    let mut archive = Archive::new(decoder);
    let entries = archive
        .entries()
        .map_err(|error| SdkManagerError::Archive {
            path: archive_path.to_path_buf(),
            message: error.to_string(),
        })?;

    for entry in entries {
        let mut entry = entry.map_err(|error| SdkManagerError::Archive {
            path: archive_path.to_path_buf(),
            message: error.to_string(),
        })?;
        let entry_path = entry.path().map_err(|error| SdkManagerError::Archive {
            path: archive_path.to_path_buf(),
            message: error.to_string(),
        })?;
        let relative_path = safe_relative_path(&entry_path)?;
        if relative_path.as_os_str().is_empty() {
            continue;
        }
        let output_path = staging_dir.join(relative_path);

        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).map_err(|source| SdkManagerError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        entry
            .unpack(&output_path)
            .map_err(|source| SdkManagerError::Io {
                path: output_path,
                source,
            })?;
    }

    Ok(())
}

fn select_nested_archives(
    staging_dir: &Path,
    host_os: SdkOs,
) -> Result<Vec<PathBuf>, SdkManagerError> {
    let mut all_archives = WalkDir::new(staging_dir)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| entry.into_path())
        .filter(|path| path.extension() == Some(OsStr::new("zip")))
        .collect::<Vec<_>>();
    all_archives.sort();

    if all_archives.is_empty() {
        return Err(SdkManagerError::MissingNestedArchive {
            path: staging_dir.to_path_buf(),
            host_os,
        });
    }

    let selected = match host_os.nested_archive_token() {
        Some(token) => {
            let mut filtered = all_archives
                .iter()
                .filter_map(|path| {
                    let relative = path.strip_prefix(staging_dir).ok()?;
                    let normalized = relative.to_string_lossy().replace('\\', "/").to_lowercase();
                    normalized.contains(token).then(|| path.clone())
                })
                .collect::<Vec<_>>();

            if filtered.is_empty() {
                all_archives
            } else {
                filtered.sort();
                filtered
            }
        }
        None => all_archives,
    };

    Ok(selected)
}

fn total_zip_bytes(paths: &[PathBuf]) -> Result<u64, SdkManagerError> {
    let mut total_bytes = 0_u64;

    for path in paths {
        let file = File::open(path).map_err(|source| SdkManagerError::Io {
            path: path.clone(),
            source,
        })?;
        let mut archive = ZipArchive::new(file).map_err(|error| SdkManagerError::Archive {
            path: path.clone(),
            message: error.to_string(),
        })?;

        for index in 0..archive.len() {
            let entry = archive
                .by_index(index)
                .map_err(|error| SdkManagerError::Archive {
                    path: path.clone(),
                    message: error.to_string(),
                })?;
            if !entry.is_dir() {
                total_bytes += entry.size();
            }
        }
    }

    Ok(total_bytes)
}

fn extract_nested_zip<F>(
    zip_path: &Path,
    target_dir: &Path,
    reporter: &mut ProgressReporter,
    written_bytes: &mut u64,
    on_progress: &mut F,
) -> Result<(), SdkManagerError>
where
    F: FnMut(ProgressEvent),
{
    let file = File::open(zip_path).map_err(|source| SdkManagerError::Io {
        path: zip_path.to_path_buf(),
        source,
    })?;
    let mut archive = ZipArchive::new(file).map_err(|error| SdkManagerError::Archive {
        path: zip_path.to_path_buf(),
        message: error.to_string(),
    })?;
    let mut buffer = vec![0_u8; 64 * 1024];

    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| SdkManagerError::Archive {
                path: zip_path.to_path_buf(),
                message: error.to_string(),
            })?;

        let relative_path = entry
            .enclosed_name()
            .map(Path::to_path_buf)
            .ok_or_else(|| SdkManagerError::UnsafeArchivePath {
                path: PathBuf::from(entry.name()),
            })?;
        let output_path = target_dir.join(relative_path);

        if entry.is_dir() {
            fs::create_dir_all(&output_path).map_err(|source| SdkManagerError::Io {
                path: output_path.clone(),
                source,
            })?;
            apply_unix_permissions(&output_path, entry.unix_mode());
            continue;
        }

        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).map_err(|source| SdkManagerError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        if is_unix_symlink(entry.unix_mode()) {
            #[cfg(unix)]
            {
                let mut target_bytes = Vec::new();
                entry
                    .read_to_end(&mut target_bytes)
                    .map_err(|error| SdkManagerError::Archive {
                        path: zip_path.to_path_buf(),
                        message: error.to_string(),
                    })?;
                remove_path_if_exists(&output_path)?;
                std::os::unix::fs::symlink(
                    String::from_utf8_lossy(&target_bytes).to_string(),
                    &output_path,
                )
                .map_err(|source| SdkManagerError::Io {
                    path: output_path,
                    source,
                })?;
                *written_bytes += target_bytes.len() as u64;
                reporter.maybe_emit(*written_bytes, on_progress);
            }

            #[cfg(not(unix))]
            {
                let mut output_file =
                    File::create(&output_path).map_err(|source| SdkManagerError::Io {
                        path: output_path.clone(),
                        source,
                    })?;
                loop {
                    let read =
                        entry
                            .read(&mut buffer)
                            .map_err(|error| SdkManagerError::Archive {
                                path: zip_path.to_path_buf(),
                                message: error.to_string(),
                            })?;
                    if read == 0 {
                        break;
                    }
                    output_file.write_all(&buffer[..read]).map_err(|source| {
                        SdkManagerError::Io {
                            path: output_path.clone(),
                            source,
                        }
                    })?;
                    *written_bytes += read as u64;
                    reporter.maybe_emit(*written_bytes, on_progress);
                }
            }

            continue;
        }

        let mut output_file = File::create(&output_path).map_err(|source| SdkManagerError::Io {
            path: output_path.clone(),
            source,
        })?;

        loop {
            let read = entry
                .read(&mut buffer)
                .map_err(|error| SdkManagerError::Archive {
                    path: zip_path.to_path_buf(),
                    message: error.to_string(),
                })?;
            if read == 0 {
                break;
            }
            output_file
                .write_all(&buffer[..read])
                .map_err(|source| SdkManagerError::Io {
                    path: output_path.clone(),
                    source,
                })?;
            *written_bytes += read as u64;
            reporter.maybe_emit(*written_bytes, on_progress);
        }

        apply_unix_permissions(&output_path, entry.unix_mode());
    }

    Ok(())
}

fn safe_relative_path(path: &Path) -> Result<PathBuf, SdkManagerError> {
    let mut output = PathBuf::new();

    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(segment) => output.push(segment),
            _ => {
                return Err(SdkManagerError::UnsafeArchivePath {
                    path: path.to_path_buf(),
                })
            }
        }
    }

    Ok(output)
}

fn reset_directory(path: &Path) -> Result<(), SdkManagerError> {
    if path.exists() {
        fs::remove_dir_all(path).map_err(|source| SdkManagerError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    }
    fs::create_dir_all(path).map_err(|source| SdkManagerError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(())
}

fn remove_path_if_exists(path: &Path) -> Result<(), SdkManagerError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.is_dir() && !metadata.file_type().is_symlink() {
                fs::remove_dir_all(path).map_err(|source| SdkManagerError::Io {
                    path: path.to_path_buf(),
                    source,
                })?;
            } else {
                fs::remove_file(path).map_err(|source| SdkManagerError::Io {
                    path: path.to_path_buf(),
                    source,
                })?;
            }
            Ok(())
        }
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(SdkManagerError::Io {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn is_unix_symlink(mode: Option<u32>) -> bool {
    mode.map(|mode| mode & S_IFMT == S_IFLNK).unwrap_or(false)
}

#[cfg(unix)]
fn apply_unix_permissions(path: &Path, mode: Option<u32>) {
    use std::os::unix::fs::PermissionsExt;

    if let Some(mode) = mode {
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(mode & 0o7777));
    }
}

#[cfg(not(unix))]
fn apply_unix_permissions(_path: &Path, _mode: Option<u32>) {}

#[cfg(test)]
mod tests {
    use super::safe_relative_path;

    #[test]
    fn rejects_parent_components() {
        let error = safe_relative_path(std::path::Path::new("../escape")).unwrap_err();
        assert!(matches!(
            error,
            crate::SdkManagerError::UnsafeArchivePath { .. }
        ));
    }
}
