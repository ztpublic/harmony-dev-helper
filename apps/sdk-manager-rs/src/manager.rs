use std::sync::Arc;

use crate::catalog::resolve_sdk_url;
use crate::downloader::SdkDownloader;
use crate::types::{ResolvedInstallOptions, SdkInstallOptions};
use crate::{SdkManagerError, SdkOs};

#[derive(Debug, Clone, Default)]
pub struct SdkManagerOptions {
    pub http_client: Option<reqwest::Client>,
    pub host_os: Option<SdkOs>,
}

#[derive(Clone)]
pub struct SdkManager(Arc<SdkManagerInner>);

#[derive(Debug)]
struct SdkManagerInner {
    http_client: reqwest::Client,
    host_os: SdkOs,
}

impl std::fmt::Debug for SdkManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SdkManager")
            .field("host_os", &self.0.host_os)
            .finish()
    }
}

impl SdkManager {
    pub fn new(options: SdkManagerOptions) -> Result<Self, SdkManagerError> {
        let http_client = match options.http_client {
            Some(client) => client,
            None => reqwest::Client::builder().build()?,
        };

        Ok(Self(Arc::new(SdkManagerInner {
            http_client,
            host_os: options.host_os.unwrap_or_else(SdkOs::current),
        })))
    }

    pub fn host_os(&self) -> SdkOs {
        self.0.host_os
    }

    pub fn resolve_url(
        &self,
        version: crate::SdkVersion,
        arch: crate::SdkArch,
        os: crate::SdkOs,
    ) -> Option<&'static str> {
        resolve_sdk_url(version, arch, os)
    }

    pub fn create_downloader(
        &self,
        options: SdkInstallOptions,
    ) -> Result<SdkDownloader, SdkManagerError> {
        let resolved = ResolvedInstallOptions::resolve(options)?;
        Ok(SdkDownloader::new(self.clone(), resolved))
    }

    pub(crate) fn http_client(&self) -> reqwest::Client {
        self.0.http_client.clone()
    }
}
