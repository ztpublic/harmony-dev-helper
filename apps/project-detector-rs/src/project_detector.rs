use crate::error::Result;
use crate::utils::path::absolute_path;
use crate::utils::uri::Uri;
use std::path::{Path, PathBuf};

#[derive(Clone)]
pub struct ProjectDetector {
    workspace_path: PathBuf,
}

impl ProjectDetector {
    pub fn new(workspace_path: impl AsRef<Path>) -> Result<Self> {
        Ok(Self {
            workspace_path: absolute_path(workspace_path.as_ref())?,
        })
    }

    pub fn from_uri(workspace_uri: impl AsRef<str>) -> Result<Self> {
        Ok(Self {
            workspace_path: Uri::parse(workspace_uri.as_ref())?.as_path().to_path_buf(),
        })
    }

    pub fn workspace_path(&self) -> &Path {
        &self.workspace_path
    }

    pub fn workspace_uri(&self) -> Uri {
        Uri::from_absolute_path(self.workspace_path.clone())
    }
}
