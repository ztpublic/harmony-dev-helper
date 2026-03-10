use std::path::PathBuf;

use thiserror::Error;

pub type Result<T> = std::result::Result<T, DetectorError>;

#[derive(Debug, Error)]
pub enum DetectorError {
    #[error("invalid file path: {path}")]
    InvalidFilePath { path: String },
    #[error("invalid uri: {input}: {source}")]
    UriParse {
        input: String,
        #[source]
        source: url::ParseError,
    },
    #[error("unsupported uri scheme for filesystem path: {uri}")]
    UnsupportedUriScheme { uri: String },
    #[error("i/o error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("walkdir error at {path}: {source}")]
    WalkDir {
        path: PathBuf,
        #[source]
        source: walkdir::Error,
    },
    #[error("json5 parse error at {path}: {source}")]
    Json5 {
        path: PathBuf,
        #[source]
        source: serde_json5::Error,
    },
    #[error("tree-sitter language setup failed: {message}")]
    TreeSitterLanguage { message: String },
    #[error("tree-sitter parse failed for {path}")]
    TreeSitterParse { path: PathBuf },
    #[error("non-utf8 tree-sitter text at {path}: {message}")]
    InvalidUtf8Text { path: PathBuf, message: String },
    #[error("path '{candidate}' escapes base directory '{base}'")]
    PathEscapesBase { base: PathBuf, candidate: PathBuf },
    #[error("expected directory at {path}")]
    ExpectedDirectory { path: PathBuf },
    #[error("expected file at {path}")]
    ExpectedFile { path: PathBuf },
    #[error("invalid project build-profile.json5 at {path}")]
    InvalidProjectBuildProfile { path: PathBuf },
    #[error("invalid module build-profile.json5 at {path}")]
    InvalidModuleBuildProfile { path: PathBuf },
}

impl DetectorError {
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }

    pub fn walkdir(source: walkdir::Error) -> Self {
        Self::WalkDir {
            path: source.path().map(PathBuf::from).unwrap_or_default(),
            source,
        }
    }

    pub fn json5(path: impl Into<PathBuf>, source: serde_json5::Error) -> Self {
        Self::Json5 {
            path: path.into(),
            source,
        }
    }
}
