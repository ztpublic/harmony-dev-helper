use crate::project_detector::ProjectDetector;
use crate::utils::uri::Uri;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use walkdir::WalkDir;

pub struct Project {
    project_detector: Arc<ProjectDetector>,
    uri: Uri,
    parsed_build_profile: serde_json::Value,
    build_profile_uri: Uri,
    build_profile_content: String,
}

impl Project {
    pub fn get_project_detector(&self) -> Arc<ProjectDetector> {
        Arc::clone(&self.project_detector)
    }

    pub fn is_in_exclude_dirs(entry: &walkdir::DirEntry) -> bool {
        entry.path().iter().any(|component| {
            if let Some(component_str) = component.to_str() {
                component_str == "node_modules"
                    || component_str == "oh_modules"
                    || component_str.starts_with('.')
            } else {
                false
            }
        })
    }

    pub fn find_all(project_detector: &Arc<ProjectDetector>) -> Vec<Arc<Project>> {
        let mut projects = Vec::new();
        let workspace_folder = project_detector.get_workspace_folder().fs_path();
        let entries: Vec<_> = WalkDir::new(workspace_folder)
            .into_iter()
            .filter_entry(|entry| !Self::is_in_exclude_dirs(entry))
            .filter_map(|res| res.ok())
            .collect();

        for entry in entries {
            let path = entry.path();
            if path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name == "build-profile.json5")
                && entry.file_type().is_file()
            {
                let project_dir = path
                    .parent()
                    .unwrap_or(Path::new(""))
                    .to_string_lossy()
                    .to_string();
                if let Some(project) = Self::create(project_detector, project_dir) {
                    projects.push(project);
                }
            }
        }

        projects
    }

    pub fn create(
        project_detector: &Arc<ProjectDetector>,
        project_uri: String,
    ) -> Option<Arc<Project>> {
        let uri = Uri::file(project_uri);
        if !fs::metadata(uri.fs_path())
            .map(|m| m.is_dir())
            .unwrap_or(false)
        {
            return None;
        }

        let build_profile_path = Path::new(&uri.fs_path()).join("build-profile.json5");
        let build_profile_uri = Uri::file(build_profile_path.to_string_lossy().to_string());
        let build_profile_content =
            fs::read_to_string(build_profile_uri.fs_path()).unwrap_or_default();
        let parsed_build_profile: serde_json::Value =
            serde_json5::from_str(&build_profile_content).unwrap_or_default();

        if parsed_build_profile.is_object()
            && parsed_build_profile.get("app").is_some_and(|app| {
                app.is_object()
                    && parsed_build_profile
                        .get("modules")
                        .and_then(|modules| modules.as_array())
                        .is_some()
            })
        {
            Some(Arc::new(Project {
                project_detector: Arc::clone(project_detector),
                uri,
                parsed_build_profile,
                build_profile_uri,
                build_profile_content,
            }))
        } else {
            None
        }
    }

    pub fn reload(&mut self) {
        let project_uri = self.get_uri();
        let build_profile_path = Path::new(&project_uri.fs_path()).join("build-profile.json5");
        let build_profile_content = fs::read_to_string(build_profile_path).unwrap_or_default();
        let parsed_build_profile: serde_json::Value =
            serde_json5::from_str(&build_profile_content).unwrap_or_default();
        if !parsed_build_profile.is_object()
            || !parsed_build_profile.get("app").is_some_and(|app| {
                app.is_object()
                    && parsed_build_profile
                        .get("modules")
                        .and_then(|modules| modules.as_array())
                        .is_some()
            })
        {
            return;
        }

        self.update_parsed_build_profile(parsed_build_profile);
        self.update_build_profile_content(build_profile_content);
    }

    pub fn get_uri(&self) -> Uri {
        self.uri.clone()
    }

    pub fn get_parsed_build_profile(&self) -> serde_json::Value {
        self.parsed_build_profile.clone()
    }

    pub fn update_parsed_build_profile(&mut self, parsed_build_profile: serde_json::Value) {
        self.parsed_build_profile = parsed_build_profile;
    }

    pub fn get_build_profile_uri(&self) -> Uri {
        self.build_profile_uri.clone()
    }

    pub fn get_build_profile_content(&self) -> String {
        self.build_profile_content.clone()
    }

    pub fn update_build_profile_content(&mut self, build_profile_content: String) {
        self.build_profile_content = build_profile_content;
    }
}
