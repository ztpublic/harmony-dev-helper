use crate::utils::uri::Uri;

#[derive(Clone)]
pub struct ProjectDetector {
    workspace_folder: Uri,
}

impl ProjectDetector {
    pub fn create(workspace_folder: String) -> Self {
        Self {
            workspace_folder: Uri::parse(workspace_folder),
        }
    }

    pub fn get_workspace_folder(&self) -> Uri {
        self.workspace_folder.clone()
    }
}
