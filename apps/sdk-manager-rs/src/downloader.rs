use std::path::{Path, PathBuf};

use futures_util::StreamExt;
use reqwest::header::{CONTENT_LENGTH, CONTENT_RANGE, RANGE};
use reqwest::StatusCode;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::extract::extract_sdk_archives;
use crate::progress::{ProgressKind, ProgressReporter};
use crate::types::ResolvedInstallOptions;
use crate::{ProgressEvent, RemoteApiError, SdkManager, SdkManagerError, SdkSource};

#[derive(Debug, Clone)]
pub struct SdkDownloader {
    manager: SdkManager,
    options: ResolvedInstallOptions,
}

impl SdkDownloader {
    pub(crate) fn new(manager: SdkManager, options: ResolvedInstallOptions) -> Self {
        Self { manager, options }
    }

    pub fn source(&self) -> &SdkSource {
        &self.options.source
    }

    pub fn url(&self) -> &str {
        &self.options.url
    }

    pub fn cache_path(&self) -> PathBuf {
        self.options.archive_path.clone()
    }

    pub fn cache_dir(&self) -> &Path {
        &self.options.cache_dir
    }

    pub fn target_dir(&self) -> &Path {
        &self.options.target_dir
    }

    pub fn staging_dir(&self) -> &Path {
        &self.options.staging_dir
    }

    pub async fn download_without_progress(&self) -> Result<(), SdkManagerError> {
        self.download(|_| {}).await
    }

    pub async fn download<F>(&self, mut on_progress: F) -> Result<(), SdkManagerError>
    where
        F: FnMut(ProgressEvent) + Send,
    {
        self.download_with_progress(&mut on_progress).await
    }

    pub async fn fetch_remote_checksum(&self) -> Result<String, SdkManagerError> {
        let checksum_url = format!("{}.sha256", self.url());
        let response = self.manager.http_client().get(&checksum_url).send().await?;
        let status = response.status();

        if status != StatusCode::OK && status != StatusCode::CREATED {
            let body = response.text().await.ok();
            return Err(RemoteApiError {
                endpoint: "download checksum",
                status: Some(status.as_u16()),
                body: body.clone(),
                message: match body {
                    Some(body) if !body.trim().is_empty() => format!(
                        "download checksum failed with status {}: {}",
                        status.as_u16(),
                        body.trim()
                    ),
                    _ => format!("download checksum failed with status {}", status.as_u16()),
                },
            }
            .into());
        }

        let body = response.text().await?;
        normalize_checksum(&body)
    }

    pub async fn verify_checksum_without_progress(&self) -> Result<(), SdkManagerError> {
        self.verify_checksum(|_| {}).await
    }

    pub async fn verify_checksum<F>(&self, mut on_progress: F) -> Result<(), SdkManagerError>
    where
        F: FnMut(ProgressEvent) + Send,
    {
        let checksum = self.fetch_remote_checksum().await?;
        self.verify_checksum_value_with_progress(&checksum, &mut on_progress)
            .await
    }

    pub async fn verify_checksum_value_without_progress(
        &self,
        checksum: &str,
    ) -> Result<(), SdkManagerError> {
        self.verify_checksum_value(checksum, |_| {}).await
    }

    pub async fn verify_checksum_value<F>(
        &self,
        checksum: &str,
        mut on_progress: F,
    ) -> Result<(), SdkManagerError>
    where
        F: FnMut(ProgressEvent) + Send,
    {
        self.verify_checksum_value_with_progress(checksum, &mut on_progress)
            .await
    }

    pub async fn extract_without_progress(&self) -> Result<(), SdkManagerError> {
        self.extract(|_| {}).await
    }

    pub async fn extract<F>(&self, mut on_progress: F) -> Result<(), SdkManagerError>
    where
        F: FnMut(ProgressEvent) + Send,
    {
        self.extract_with_progress(&mut on_progress).await
    }

    pub async fn install_without_progress(&self) -> Result<(), SdkManagerError> {
        self.install(|_| {}).await
    }

    pub async fn install<F>(&self, mut on_progress: F) -> Result<(), SdkManagerError>
    where
        F: FnMut(ProgressEvent) + Send,
    {
        self.download_with_progress(&mut on_progress).await?;
        self.verify_checksum_with_progress(&mut on_progress).await?;
        self.extract_with_progress(&mut on_progress).await?;
        self.clean().await?;
        Ok(())
    }

    pub async fn clean(&self) -> Result<(), SdkManagerError> {
        if !self.options.clean {
            return Ok(());
        }

        remove_file_if_exists(&self.options.archive_path).await?;
        remove_dir_if_exists(&self.options.staging_dir).await?;
        remove_dir_if_exists(&self.options.cache_dir).await?;
        Ok(())
    }

    async fn download_with_progress<F>(&self, on_progress: &mut F) -> Result<(), SdkManagerError>
    where
        F: FnMut(ProgressEvent),
    {
        ensure_parent_dir(&self.options.archive_path).await?;
        tokio::fs::create_dir_all(&self.options.cache_dir)
            .await
            .map_err(|source| SdkManagerError::Io {
                path: self.options.cache_dir.clone(),
                source,
            })?;

        let requested_start = if self.options.resume_download {
            match tokio::fs::metadata(&self.options.archive_path).await {
                Ok(metadata) => metadata.len(),
                Err(source) if source.kind() == std::io::ErrorKind::NotFound => 0,
                Err(source) => {
                    return Err(SdkManagerError::Io {
                        path: self.options.archive_path.clone(),
                        source,
                    })
                }
            }
        } else {
            remove_file_if_exists(&self.options.archive_path).await?;
            0
        };

        let (response, effective_start) =
            self.request_archive(requested_start, false, false).await?;
        let total_bytes = parse_total_bytes(&response);
        let mut reporter = ProgressReporter::new(ProgressKind::Download, total_bytes, true);

        if effective_start > 0 {
            reporter.emit_reset(effective_start, on_progress);
        }

        let mut file = if effective_start > 0 {
            tokio::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.options.archive_path)
                .await
                .map_err(|source| SdkManagerError::Io {
                    path: self.options.archive_path.clone(),
                    source,
                })?
        } else {
            tokio::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&self.options.archive_path)
                .await
                .map_err(|source| SdkManagerError::Io {
                    path: self.options.archive_path.clone(),
                    source,
                })?
        };

        let mut received_bytes = effective_start;
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            file.write_all(&chunk)
                .await
                .map_err(|source| SdkManagerError::Io {
                    path: self.options.archive_path.clone(),
                    source,
                })?;
            received_bytes += chunk.len() as u64;
            reporter.maybe_emit(received_bytes, on_progress);
        }
        file.flush().await.map_err(|source| SdkManagerError::Io {
            path: self.options.archive_path.clone(),
            source,
        })?;

        reporter.finish(received_bytes, on_progress);
        Ok(())
    }

    async fn verify_checksum_with_progress<F>(
        &self,
        on_progress: &mut F,
    ) -> Result<(), SdkManagerError>
    where
        F: FnMut(ProgressEvent),
    {
        let checksum = self.fetch_remote_checksum().await?;
        self.verify_checksum_value_with_progress(&checksum, on_progress)
            .await
    }

    async fn verify_checksum_value_with_progress<F>(
        &self,
        checksum: &str,
        on_progress: &mut F,
    ) -> Result<(), SdkManagerError>
    where
        F: FnMut(ProgressEvent),
    {
        let expected = normalize_checksum(checksum)?;
        let metadata = tokio::fs::metadata(&self.options.archive_path)
            .await
            .map_err(|source| SdkManagerError::Io {
                path: self.options.archive_path.clone(),
                source,
            })?;
        let total_bytes = metadata.len();
        let mut file = tokio::fs::File::open(&self.options.archive_path)
            .await
            .map_err(|source| SdkManagerError::Io {
                path: self.options.archive_path.clone(),
                source,
            })?;
        let mut hasher = Sha256::new();
        let mut buffer = vec![0_u8; 64 * 1024];
        let mut read_bytes = 0_u64;
        let mut reporter = ProgressReporter::new(ProgressKind::Checksum, Some(total_bytes), false);

        loop {
            let read = file
                .read(&mut buffer)
                .await
                .map_err(|source| SdkManagerError::Io {
                    path: self.options.archive_path.clone(),
                    source,
                })?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
            read_bytes += read as u64;
            reporter.maybe_emit(read_bytes, on_progress);
        }

        reporter.finish(read_bytes, on_progress);
        let actual = hex::encode(hasher.finalize());
        if actual == expected {
            Ok(())
        } else {
            Err(SdkManagerError::ChecksumMismatch {
                path: self.options.archive_path.clone(),
                expected,
                actual,
            })
        }
    }

    async fn extract_with_progress<F>(&self, on_progress: &mut F) -> Result<(), SdkManagerError>
    where
        F: FnMut(ProgressEvent),
    {
        extract_sdk_archives(
            &self.options.archive_path,
            &self.options.staging_dir,
            &self.options.target_dir,
            self.manager.host_os(),
            on_progress,
        )
    }

    async fn request_archive(
        &self,
        start_byte: u64,
        retried_invalid_range: bool,
        retried_full_response: bool,
    ) -> Result<(reqwest::Response, u64), SdkManagerError> {
        let mut request = self.manager.http_client().get(&self.options.url);
        if start_byte > 0 {
            request = request.header(RANGE, format!("bytes={start_byte}-"));
        }
        let response = request.send().await?;
        let status = response.status();

        if status == StatusCode::RANGE_NOT_SATISFIABLE && !retried_invalid_range {
            remove_file_if_exists(&self.options.archive_path).await?;
            return Box::pin(self.request_archive(0, true, retried_full_response)).await;
        }

        if start_byte > 0 && status == StatusCode::OK && !retried_full_response {
            remove_file_if_exists(&self.options.archive_path).await?;
            return Box::pin(self.request_archive(0, retried_invalid_range, true)).await;
        }

        if status == StatusCode::OK || status == StatusCode::PARTIAL_CONTENT {
            let effective_start = if status == StatusCode::PARTIAL_CONTENT {
                start_byte
            } else {
                0
            };
            return Ok((response, effective_start));
        }

        let body = response.text().await.ok();
        Err(RemoteApiError {
            endpoint: "download archive",
            status: Some(status.as_u16()),
            body: body.clone(),
            message: match body {
                Some(body) if !body.trim().is_empty() => format!(
                    "download archive failed with status {}: {}",
                    status.as_u16(),
                    body.trim()
                ),
                _ => format!("download archive failed with status {}", status.as_u16()),
            },
        }
        .into())
    }
}

fn parse_total_bytes(response: &reqwest::Response) -> Option<u64> {
    if let Some(content_range) = response.headers().get(CONTENT_RANGE) {
        if let Ok(content_range) = content_range.to_str() {
            if let Some(total) = content_range.rsplit('/').next() {
                if let Ok(total) = total.parse::<u64>() {
                    return Some(total);
                }
            }
        }
    }

    response
        .headers()
        .get(CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
}

fn normalize_checksum(input: &str) -> Result<String, SdkManagerError> {
    let trimmed = input.trim();
    if trimmed.len() == 64 && trimmed.chars().all(|char| char.is_ascii_hexdigit()) {
        return Ok(trimmed.to_ascii_lowercase());
    }

    for token in trimmed.split_whitespace() {
        if token.len() == 64 && token.chars().all(|char| char.is_ascii_hexdigit()) {
            return Ok(token.to_ascii_lowercase());
        }
    }

    Err(SdkManagerError::InvalidChecksum(trimmed.to_string()))
}

async fn ensure_parent_dir(path: &Path) -> Result<(), SdkManagerError> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|source| SdkManagerError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
    }
    Ok(())
}

async fn remove_file_if_exists(path: &Path) -> Result<(), SdkManagerError> {
    match tokio::fs::remove_file(path).await {
        Ok(()) => Ok(()),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(SdkManagerError::Io {
            path: path.to_path_buf(),
            source,
        }),
    }
}

async fn remove_dir_if_exists(path: &Path) -> Result<(), SdkManagerError> {
    match tokio::fs::remove_dir_all(path).await {
        Ok(()) => Ok(()),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(SdkManagerError::Io {
            path: path.to_path_buf(),
            source,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::normalize_checksum;

    #[test]
    fn parses_checksum_from_sum_file() {
        let checksum = normalize_checksum(
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef  sdk.tar.gz",
        )
        .unwrap();
        assert_eq!(
            checksum,
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        );
    }
}
