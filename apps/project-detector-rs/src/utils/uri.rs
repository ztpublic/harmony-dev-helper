use crate::error::{DetectorError, Result};
use crate::utils::path::absolute_path;
use std::{fmt, path};
use url::Url;

#[derive(Clone, PartialEq, Eq)]
pub struct Uri {
    fs_path: path::PathBuf,
    url: Url,
}

impl Uri {
    pub fn file(path: impl AsRef<path::Path>) -> Result<Self> {
        let path = absolute_path(path.as_ref())?;
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

    pub fn as_path(&self) -> &path::Path {
        &self.fs_path
    }

    pub(crate) fn from_absolute_path(path: path::PathBuf) -> Self {
        debug_assert!(path.is_absolute());
        let url = Url::from_file_path(&path)
            .unwrap_or_else(|_| panic!("absolute filesystem path should convert to file URI"));
        Self { fs_path: path, url }
    }
}

impl fmt::Display for Uri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.url)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_uri_round_trip_keeps_the_normalized_filesystem_path() -> Result<()> {
        let uri = Uri::file("./src/../src")?;
        let expected_path = absolute_path(path::Path::new("./src/../src"))?;
        let reparsed = Uri::parse(uri.to_string())?;

        assert_eq!(uri.as_path(), expected_path.as_path());
        assert_eq!(reparsed.as_path(), expected_path.as_path());
        Ok(())
    }
}
