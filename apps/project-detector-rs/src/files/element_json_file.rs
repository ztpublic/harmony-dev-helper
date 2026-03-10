use crate::element_directory::ElementDirectory;
use crate::error::{DetectorError, Result};
use crate::fs_discovery::find_immediate_files;
use crate::utils::path::{absolute_path, path_is_file, read_to_string};
use crate::utils::uri::Uri;
use std::path::{Path, PathBuf};
use tree_sitter::{Parser, Tree};

pub struct ElementJsonFile {
    source_code: String,
    path: PathBuf,
}

impl ElementJsonFile {
    pub fn from_source(
        element_json_file_path: impl AsRef<Path>,
        source_code: impl Into<String>,
    ) -> Result<Self> {
        let path = absolute_path(element_json_file_path.as_ref())?;

        Ok(Self {
            source_code: source_code.into(),
            path,
        })
    }

    pub fn load(element_json_file_path: impl AsRef<Path>) -> Result<Option<ElementJsonFile>> {
        let element_json_file_path = absolute_path(element_json_file_path.as_ref())?;
        if !is_element_json_path(&element_json_file_path) {
            return Ok(None);
        }

        if !path_is_file(&element_json_file_path)? {
            return Err(DetectorError::ExpectedFile {
                path: element_json_file_path,
            });
        }

        Ok(Some(Self {
            source_code: read_source(&element_json_file_path)?,
            path: element_json_file_path,
        }))
    }

    fn build_parser() -> Result<Parser> {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_json::LANGUAGE.into())
            .map_err(|source| DetectorError::TreeSitterLanguage {
                message: source.to_string(),
            })?;
        Ok(parser)
    }

    pub fn reload(&mut self) -> Result<()> {
        self.replace_content(read_source(&self.path)?);
        Ok(())
    }

    pub fn find_all(element_directory: &ElementDirectory) -> Result<Vec<ElementJsonFile>> {
        let mut element_json_files = Vec::new();
        for file_path in find_immediate_files(element_directory.path())? {
            if let Some(element_json_file) = Self::load(&file_path)? {
                element_json_files.push(element_json_file);
            }
        }

        Ok(element_json_files)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn uri(&self) -> Uri {
        Uri::from_absolute_path(self.path.clone())
    }

    pub fn content(&self) -> &str {
        &self.source_code
    }

    pub fn replace_content(&mut self, source_code: String) {
        self.source_code = source_code;
    }

    pub fn parse(&self) -> Result<serde_json::Value> {
        parse_json5(self.content(), self.path())
    }

    pub(crate) fn parse_tree(&self) -> Result<Tree> {
        parse_tree(self.content(), self.path())
    }
}

fn is_element_json_path(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension == "json")
}

fn read_source(path: &Path) -> Result<String> {
    read_to_string(path)
}

fn parse_json5(source_code: &str, path: &Path) -> Result<serde_json::Value> {
    serde_json5::from_str(source_code).map_err(|source| DetectorError::json5(path, source))
}

fn parse_tree(source_code: &str, path: &Path) -> Result<Tree> {
    let mut parser = ElementJsonFile::build_parser()?;
    parser
        .parse(source_code, None)
        .ok_or_else(|| DetectorError::TreeSitterParse {
            path: path.to_path_buf(),
        })
}
