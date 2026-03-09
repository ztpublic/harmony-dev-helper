use crate::error::{DetectorError, Result};
use std::{env, fmt, path};
use url::Url;

#[derive(Clone, PartialEq, Eq)]
pub struct Uri {
    fs_path: path::PathBuf,
    url: Url,
}

impl Uri {
    pub fn file(path: impl AsRef<path::Path>) -> Result<Self> {
        let path = Self::absolute_path(path.as_ref())?;
        let url = Url::from_file_path(&path).map_err(|_| DetectorError::InvalidFilePath {
            path: path.to_string_lossy().to_string(),
        })?;

        Ok(Self { fs_path: path, url })
    }

    pub fn parse(url: impl AsRef<str>) -> Result<Self> {
        let input = url.as_ref();
        let url = Url::parse(input).map_err(|source| DetectorError::UriParse {
            input: input.to_string(),
            source,
        })?;
        if url.scheme() != "file" {
            return Err(DetectorError::UnsupportedUriScheme {
                uri: input.to_string(),
            });
        }
        let fs_path = url
            .to_file_path()
            .map_err(|_| DetectorError::InvalidFilePath {
                path: input.to_string(),
            })?;

        Ok(Self { fs_path, url })
    }

    pub fn from_path_or_uri(value: impl AsRef<str>) -> Result<Self> {
        let value = value.as_ref();
        match Url::parse(value) {
            Ok(_) => Self::parse(value),
            Err(source) if value.contains("://") => Err(DetectorError::UriParse {
                input: value.to_string(),
                source,
            }),
            Err(_) => Self::file(path::Path::new(value)),
        }
    }

    pub fn base_name(uri: &Uri) -> String {
        uri.url
            .path_segments()
            .and_then(|mut segments| segments.next_back())
            .unwrap_or_default()
            .to_string()
    }

    pub fn as_path(&self) -> &path::Path {
        &self.fs_path
    }

    pub fn dir_name(uri: &Uri) -> Result<Uri> {
        let path = path::Path::new(&uri.fs_path);
        let parent = path
            .parent()
            .ok_or_else(|| DetectorError::InvalidFilePath {
                path: path.to_string_lossy().to_string(),
            })?;
        Uri::file(parent)
    }
    pub fn is_equal(&self, other: &Uri) -> bool {
        self.fs_path == other.fs_path || self.to_string() == other.to_string()
    }
    pub fn fs_path(&self) -> String {
        self.fs_path.to_string_lossy().to_string()
    }
    pub fn path(&self) -> String {
        self.url.path().to_string()
    }
    pub fn scheme(&self) -> String {
        self.url.scheme().to_string()
    }
    pub fn host(&self) -> String {
        self.url.host().map(|h| h.to_string()).unwrap_or_default()
    }
    pub fn query(&self) -> String {
        self.url.query().map(|q| q.to_string()).unwrap_or_default()
    }
    pub fn fragment(&self) -> String {
        self.url
            .fragment()
            .map(|f| f.to_string())
            .unwrap_or_default()
    }
    pub fn username(&self) -> String {
        self.url.username().to_string()
    }
    pub fn password(&self) -> String {
        self.url
            .password()
            .map(|p| p.to_string())
            .unwrap_or_default()
    }
    pub fn port(&self) -> u16 {
        self.url.port().unwrap_or_default()
    }

    pub fn url(&self) -> &Url {
        &self.url
    }

    #[allow(clippy::inherent_to_string_shadow_display)]
    pub fn to_string(&self) -> String {
        self.url.to_string()
    }

    fn absolute_path(path: &path::Path) -> Result<path::PathBuf> {
        if path.is_absolute() {
            return Ok(path.to_path_buf());
        }

        let cwd = env::current_dir().map_err(|source| DetectorError::io(".", source))?;
        Ok(path_clean::clean(cwd.join(path)))
    }
}

impl fmt::Display for Uri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.url)
    }
}
