use crate::error::Result;
use crate::utils::uri::Uri;

#[derive(Clone)]
pub struct ProjectDetector {
    workspace_folder: Uri,
}

impl ProjectDetector {
    pub fn create(workspace_folder: String) -> Result<Self> {
        Ok(Self {
            workspace_folder: Uri::from_path_or_uri(workspace_folder)?,
        })
    }

    pub fn get_workspace_folder(&self) -> Uri {
        self.workspace_folder.clone()
    }
}
