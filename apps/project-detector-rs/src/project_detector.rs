use crate::error::Result;
use crate::utils::uri::Uri;

#[derive(Clone)]
pub struct ProjectDetector {
    workspace_folder: Uri,
}

impl ProjectDetector {
    pub fn new(workspace_folder: impl AsRef<str>) -> Result<Self> {
        Ok(Self {
            workspace_folder: Uri::from_path_or_uri(workspace_folder.as_ref())?,
        })
    }

    pub fn workspace_folder(&self) -> &Uri {
        &self.workspace_folder
    }
}
