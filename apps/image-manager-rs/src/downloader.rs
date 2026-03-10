use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use futures_util::StreamExt;
use reqwest::header::{CONTENT_LENGTH, CONTENT_RANGE, RANGE};
use reqwest::StatusCode;
use sha2::{Digest, Sha256};
use tokio::io::AsyncReadExt;

use crate::error::{ImageManagerError, RemoteApiError};
use crate::manager::ImageManager;
use crate::types::{ensure_parent_dir, RemoteImage};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressKind {
    Download,
    Checksum,
    Extract,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpeedUnit {
    KB,
    MB,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProgressUpdate {
    pub increment: f64,
    pub progress: f64,
    pub network: Option<f64>,
    pub unit: Option<SpeedUnit>,
    pub reset: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProgressEvent {
    pub kind: ProgressKind,
    pub update: ProgressUpdate,
}

#[derive(Debug, Clone)]
pub struct Downloader {
    manager: ImageManager,
    remote_image: RemoteImage,
    url: String,
}

impl Downloader {
    pub(crate) fn new(manager: ImageManager, remote_image: RemoteImage, url: String) -> Self {
        Self {
            manager,
            remote_image,
            url,
        }
    }

    pub fn remote_image(&self) -> &RemoteImage {
        &self.remote_image
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn cache_path(&self) -> PathBuf {
        let file_name = reqwest::Url::parse(&self.url)
            .ok()
            .and_then(|url| {
                url.path_segments()
                    .and_then(|segments| segments.last().map(str::to_string))
            })
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "download.zip".to_string());
        self.manager.resolved_paths().cache_path.join(file_name)
    }

    pub async fn download_without_progress(&self) -> Result<(), ImageManagerError> {
        self.download(|_| {}).await
    }

    pub async fn download<F>(&self, mut on_progress: F) -> Result<(), ImageManagerError>
    where
        F: FnMut(ProgressEvent) + Send,
    {
        let cache_path = self.cache_path();
        ensure_parent_dir(&cache_path).await?;

        let start_byte = match tokio::fs::metadata(&cache_path).await {
            Ok(metadata) => metadata.len(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => 0,
            Err(source) => {
                return Err(ImageManagerError::Io {
                    path: cache_path.clone(),
                    source,
                })
            }
        };

        let response = self.request(start_byte, false).await?;
        let total_bytes = parse_total_bytes(&response);

        if start_byte > 0 {
            if let Some(total) = total_bytes {
                if total > 0 {
                    on_progress(ProgressEvent {
                        kind: ProgressKind::Download,
                        update: ProgressUpdate {
                            increment: 0.0,
                            progress: round_percent((start_byte as f64 / total as f64) * 100.0),
                            network: Some(0.0),
                            unit: Some(SpeedUnit::KB),
                            reset: true,
                        },
                    });
                }
            }
        }

        let mut file = if start_byte > 0 {
            tokio::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&cache_path)
                .await
                .map_err(|source| ImageManagerError::Io {
                    path: cache_path.clone(),
                    source,
                })?
        } else {
            tokio::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&cache_path)
                .await
                .map_err(|source| ImageManagerError::Io {
                    path: cache_path.clone(),
                    source,
                })?
        };

        let mut received_bytes = start_byte;
        let mut last_reported_bytes = start_byte;
        let mut last_reported_progress = if let Some(total) = total_bytes {
            if total > 0 {
                round_percent((start_byte as f64 / total as f64) * 100.0)
            } else {
                0.0
            }
        } else {
            0.0
        };
        let mut last_report_time = Instant::now();
        let report_threshold = 64 * 1024;

        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            tokio::io::AsyncWriteExt::write_all(&mut file, &chunk)
                .await
                .map_err(|source| ImageManagerError::Io {
                    path: cache_path.clone(),
                    source,
                })?;

            received_bytes += chunk.len() as u64;
            if received_bytes - last_reported_bytes >= report_threshold {
                let event = build_progress_event(
                    ProgressKind::Download,
                    total_bytes,
                    received_bytes,
                    &mut last_reported_bytes,
                    &mut last_reported_progress,
                    &mut last_report_time,
                    true,
                );
                if event.update.increment > 0.0 {
                    on_progress(event);
                }
            }
        }

        if received_bytes != last_reported_bytes {
            let event = build_progress_event(
                ProgressKind::Download,
                total_bytes,
                received_bytes,
                &mut last_reported_bytes,
                &mut last_reported_progress,
                &mut last_report_time,
                true,
            );
            if event.update.increment > 0.0 {
                on_progress(event);
            }
        }

        Ok(())
    }

    pub async fn verify_checksum_without_progress(&self) -> Result<bool, ImageManagerError> {
        self.verify_checksum(|_| {}).await
    }

    pub async fn verify_checksum<F>(&self, mut on_progress: F) -> Result<bool, ImageManagerError>
    where
        F: FnMut(ProgressEvent) + Send,
    {
        let cache_path = self.cache_path();
        let expected_checksum = self
            .remote_image
            .sdk()
            .archive
            .as_ref()
            .and_then(|archive| archive.complete.as_ref())
            .map(|complete| complete.checksum.clone());

        let Some(expected_checksum) = expected_checksum else {
            return Ok(false);
        };

        let metadata =
            tokio::fs::metadata(&cache_path)
                .await
                .map_err(|source| ImageManagerError::Io {
                    path: cache_path.clone(),
                    source,
                })?;
        let total_bytes = metadata.len();

        let mut file =
            tokio::fs::File::open(&cache_path)
                .await
                .map_err(|source| ImageManagerError::Io {
                    path: cache_path.clone(),
                    source,
                })?;
        let mut hasher = Sha256::new();
        let mut buffer = vec![0_u8; 64 * 1024];
        let mut read_bytes = 0_u64;
        let mut last_reported_bytes = 0_u64;
        let mut last_reported_progress = 0.0;
        let mut last_report_time = Instant::now();

        loop {
            let read = file
                .read(&mut buffer)
                .await
                .map_err(|source| ImageManagerError::Io {
                    path: cache_path.clone(),
                    source,
                })?;
            if read == 0 {
                break;
            }

            hasher.update(&buffer[..read]);
            read_bytes += read as u64;

            if read_bytes - last_reported_bytes >= 64 * 1024 {
                let event = build_progress_event(
                    ProgressKind::Checksum,
                    Some(total_bytes),
                    read_bytes,
                    &mut last_reported_bytes,
                    &mut last_reported_progress,
                    &mut last_report_time,
                    false,
                );
                if event.update.increment > 0.0 {
                    on_progress(event);
                }
            }
        }

        if read_bytes != last_reported_bytes {
            let event = build_progress_event(
                ProgressKind::Checksum,
                Some(total_bytes),
                read_bytes,
                &mut last_reported_bytes,
                &mut last_reported_progress,
                &mut last_report_time,
                false,
            );
            if event.update.increment > 0.0 {
                on_progress(event);
            }
        }

        let actual_checksum = hex::encode(hasher.finalize());
        Ok(actual_checksum == expected_checksum)
    }

    pub async fn extract_without_progress(&self) -> Result<(), ImageManagerError> {
        self.extract(|_| {}).await
    }

    pub async fn extract<F>(&self, mut on_progress: F) -> Result<(), ImageManagerError>
    where
        F: FnMut(ProgressEvent) + Send,
    {
        let archive_path = self.cache_path();
        let destination = self.remote_image.full_path();
        tokio::fs::create_dir_all(&destination)
            .await
            .map_err(|source| ImageManagerError::Io {
                path: destination.clone(),
                source,
            })?;

        let archive_file =
            std::fs::File::open(&archive_path).map_err(|source| ImageManagerError::Io {
                path: archive_path.clone(),
                source,
            })?;
        let mut archive =
            zip::ZipArchive::new(archive_file).map_err(|error| ImageManagerError::Archive {
                path: archive_path.clone(),
                message: error.to_string(),
            })?;

        let mut total_bytes = 0_u64;
        for index in 0..archive.len() {
            if let Ok(entry) = archive.by_index(index) {
                if !entry.is_dir() {
                    total_bytes += entry.size();
                }
            }
        }

        let mut written_bytes = 0_u64;
        let mut last_reported_bytes = 0_u64;
        let mut last_reported_progress = 0.0;
        let mut last_report_time = Instant::now();
        let mut buffer = vec![0_u8; 64 * 1024];

        for index in 0..archive.len() {
            let mut entry =
                archive
                    .by_index(index)
                    .map_err(|error| ImageManagerError::Archive {
                        path: archive_path.clone(),
                        message: error.to_string(),
                    })?;
            let Some(enclosed_name) = entry.enclosed_name().map(Path::to_path_buf) else {
                return Err(ImageManagerError::UnsafeArchivePath {
                    path: PathBuf::from(entry.name()),
                });
            };
            let output_path = destination.join(enclosed_name);

            if entry.is_dir() {
                std::fs::create_dir_all(&output_path).map_err(|source| ImageManagerError::Io {
                    path: output_path,
                    source,
                })?;
                continue;
            }

            if let Some(parent) = output_path.parent() {
                std::fs::create_dir_all(parent).map_err(|source| ImageManagerError::Io {
                    path: parent.to_path_buf(),
                    source,
                })?;
            }

            let mut output_file =
                std::fs::File::create(&output_path).map_err(|source| ImageManagerError::Io {
                    path: output_path.clone(),
                    source,
                })?;

            loop {
                let read = entry
                    .read(&mut buffer)
                    .map_err(|error| ImageManagerError::Archive {
                        path: archive_path.clone(),
                        message: error.to_string(),
                    })?;
                if read == 0 {
                    break;
                }

                output_file
                    .write_all(&buffer[..read])
                    .map_err(|source| ImageManagerError::Io {
                        path: output_path.clone(),
                        source,
                    })?;
                written_bytes += read as u64;

                if written_bytes - last_reported_bytes >= 64 * 1024 {
                    let event = build_progress_event(
                        ProgressKind::Extract,
                        Some(total_bytes),
                        written_bytes,
                        &mut last_reported_bytes,
                        &mut last_reported_progress,
                        &mut last_report_time,
                        false,
                    );
                    if event.update.increment > 0.0 {
                        on_progress(event);
                    }
                }
            }
        }

        if written_bytes != last_reported_bytes {
            let event = build_progress_event(
                ProgressKind::Extract,
                Some(total_bytes),
                written_bytes,
                &mut last_reported_bytes,
                &mut last_reported_progress,
                &mut last_report_time,
                false,
            );
            if event.update.increment > 0.0 {
                on_progress(event);
            }
        }

        Ok(())
    }

    async fn request(
        &self,
        start_byte: u64,
        retried_416: bool,
    ) -> Result<reqwest::Response, ImageManagerError> {
        let mut request = self.manager.http_client().get(&self.url);
        if start_byte > 0 {
            request = request.header(RANGE, format!("bytes={start_byte}-"));
        }
        let response = request.send().await?;
        let status = response.status();

        if status == StatusCode::RANGE_NOT_SATISFIABLE && !retried_416 {
            remove_file_if_exists(&self.cache_path()).await?;
            return Box::pin(self.request(0, true)).await;
        }

        if status == StatusCode::OK || status == StatusCode::PARTIAL_CONTENT {
            return Ok(response);
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

fn round_percent(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

fn build_progress_event(
    kind: ProgressKind,
    total_bytes: Option<u64>,
    current_bytes: u64,
    last_reported_bytes: &mut u64,
    last_reported_progress: &mut f64,
    last_report_time: &mut Instant,
    include_network: bool,
) -> ProgressEvent {
    let now = Instant::now();
    let bytes_since_last = current_bytes.saturating_sub(*last_reported_bytes);
    let progress = total_bytes
        .filter(|total| *total > 0)
        .map(|total| round_percent((current_bytes as f64 / total as f64) * 100.0))
        .unwrap_or(0.0);
    let increment = round_percent(progress - *last_reported_progress).clamp(0.0, 100.0);
    let elapsed_seconds = now.duration_since(*last_report_time).as_secs_f64();

    *last_reported_bytes = current_bytes;
    *last_reported_progress = progress;
    *last_report_time = now;

    let (network, unit) = if include_network && elapsed_seconds > 0.0 && bytes_since_last > 0 {
        let kb_per_second = (bytes_since_last as f64 / 1024.0) / elapsed_seconds;
        if kb_per_second >= 1024.0 {
            (
                Some(round_percent(kb_per_second / 1024.0)),
                Some(SpeedUnit::MB),
            )
        } else {
            (Some(round_percent(kb_per_second)), Some(SpeedUnit::KB))
        }
    } else {
        (None, None)
    };

    ProgressEvent {
        kind,
        update: ProgressUpdate {
            increment,
            progress,
            network,
            unit,
            reset: false,
        },
    }
}

async fn remove_file_if_exists(path: &Path) -> Result<(), ImageManagerError> {
    match tokio::fs::remove_file(path).await {
        Ok(()) => Ok(()),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(ImageManagerError::Io {
            path: path.to_path_buf(),
            source,
        }),
    }
}
