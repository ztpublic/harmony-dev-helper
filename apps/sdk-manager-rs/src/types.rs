use std::path::PathBuf;

use reqwest::Url;

use crate::catalog::resolve_sdk_url;
use crate::{SdkArch, SdkManagerError, SdkOs, SdkVersion};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SdkSource {
    Release {
        version: SdkVersion,
        arch: SdkArch,
        os: SdkOs,
    },
    Url(String),
}

#[derive(Debug, Clone)]
pub struct SdkInstallOptions {
    pub source: SdkSource,
    pub cache_dir: PathBuf,
    pub target_dir: PathBuf,
    pub archive_path: Option<PathBuf>,
    pub resume_download: bool,
    pub clean: bool,
}

impl SdkInstallOptions {
    pub fn new(
        source: SdkSource,
        cache_dir: impl Into<PathBuf>,
        target_dir: impl Into<PathBuf>,
    ) -> Self {
        Self {
            source,
            cache_dir: cache_dir.into(),
            target_dir: target_dir.into(),
            archive_path: None,
            resume_download: true,
            clean: true,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ResolvedInstallOptions {
    pub(crate) source: SdkSource,
    pub(crate) url: String,
    pub(crate) cache_dir: PathBuf,
    pub(crate) target_dir: PathBuf,
    pub(crate) archive_path: PathBuf,
    pub(crate) staging_dir: PathBuf,
    pub(crate) resume_download: bool,
    pub(crate) clean: bool,
}

impl ResolvedInstallOptions {
    pub(crate) fn resolve(options: SdkInstallOptions) -> Result<Self, SdkManagerError> {
        let url = resolve_source_url(&options.source)?;
        let staging_dir = options.cache_dir.join(".tar-extracted");
        let archive_path = options
            .archive_path
            .clone()
            .unwrap_or_else(|| options.cache_dir.join(default_archive_name(&url)));

        Ok(Self {
            source: options.source,
            url,
            cache_dir: options.cache_dir,
            target_dir: options.target_dir,
            archive_path,
            staging_dir,
            resume_download: options.resume_download,
            clean: options.clean,
        })
    }
}

fn resolve_source_url(source: &SdkSource) -> Result<String, SdkManagerError> {
    match source {
        SdkSource::Release { version, arch, os } => resolve_sdk_url(*version, *arch, *os)
            .map(str::to_string)
            .ok_or(SdkManagerError::UnsupportedSdk {
                version: *version,
                arch: *arch,
                os: *os,
            }),
        SdkSource::Url(url) => {
            if Url::parse(url).is_err() {
                return Err(SdkManagerError::InvalidUrl(url.clone()));
            }
            Ok(url.clone())
        }
    }
}

fn default_archive_name(url: &str) -> String {
    Url::parse(url)
        .ok()
        .and_then(|parsed| {
            parsed
                .path_segments()
                .and_then(|segments| segments.last().map(str::to_string))
        })
        .filter(|segment| !segment.is_empty())
        .unwrap_or_else(|| "download.tar.gz".to_string())
}
