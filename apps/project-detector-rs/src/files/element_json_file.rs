use crate::element_directory::ElementDirectory;
use crate::error::{DetectorError, Result};
use crate::utils::path::{path_is_file, read_to_string};
use crate::utils::uri::Uri;
use std::fs;
use std::path::Path;
use std::sync::Mutex;
use tree_sitter::Parser;

pub struct ElementJsonFile {
    parser: Mutex<Parser>,
    source_code: String,
    uri: Uri,
}

impl ElementJsonFile {
    pub fn from_source(
        element_json_file_uri: impl AsRef<str>,
        source_code: String,
    ) -> Result<Self> {
        let uri = Uri::from_path_or_uri(element_json_file_uri.as_ref())?;

        Ok(Self {
            parser: Self::build_parser()?,
            source_code,
            uri,
        })
    }

    pub fn load(element_json_file_path: impl AsRef<Path>) -> Result<Option<ElementJsonFile>> {
        let element_json_file_path = element_json_file_path.as_ref().to_path_buf();
        if element_json_file_path
            .extension()
            .is_none_or(|extension| extension != "json")
        {
            return Ok(None);
        }

        if !path_is_file(&element_json_file_path)? {
            return Err(DetectorError::ExpectedFile {
                path: element_json_file_path,
            });
        }

        Ok(Some(Self {
            parser: Self::build_parser()?,
            source_code: read_to_string(&element_json_file_path)?,
            uri: Uri::file(&element_json_file_path)?,
        }))
    }

    fn build_parser() -> Result<Mutex<Parser>> {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_json::LANGUAGE.into())
            .map_err(|source| DetectorError::TreeSitterLanguage {
                message: source.to_string(),
            })?;
        Ok(Mutex::new(parser))
    }

    pub fn reload(&mut self) -> Result<()> {
        self.replace_content(read_to_string(self.uri.as_path())?);
        Ok(())
    }

    pub fn find_all(element_directory: &ElementDirectory) -> Result<Vec<ElementJsonFile>> {
        let mut element_json_files = Vec::new();
        let resource_files = fs::read_dir(element_directory.uri().as_path())
            .map_err(|source| DetectorError::io(element_directory.uri().as_path(), source))?;

        for file_entry in resource_files {
            let file_entry = file_entry
                .map_err(|source| DetectorError::io(element_directory.uri().as_path(), source))?;
            let path = file_entry.path();
            let metadata = file_entry
                .metadata()
                .map_err(|source| DetectorError::io(path.clone(), source))?;
            if !metadata.is_file() {
                continue;
            }

            if let Some(element_json_file) = Self::load(&path)? {
                element_json_files.push(element_json_file);
            }
        }

        Ok(element_json_files)
    }

    pub fn uri(&self) -> &Uri {
        &self.uri
    }

    pub fn content(&self) -> &str {
        &self.source_code
    }

    pub fn replace_content(&mut self, source_code: String) {
        self.source_code = source_code;
    }

    pub fn parse(&self) -> Result<serde_json::Value> {
        serde_json5::from_str(&self.source_code)
            .map_err(|source| DetectorError::json5(self.uri.as_path(), source))
    }

    pub(crate) fn parser(&self) -> &Mutex<Parser> {
        &self.parser
    }
}

#[cfg(test)]
mod tests {
    use crate::references::element_json_file_reference::ElementJsonFileReference;

    use super::*;

    fn slice(s: &str, start: usize, end_exclusive: usize) -> String {
        let mut byte_start = 0usize;
        let mut byte_end = s.len();
        for (i, (bpos, _)) in s.char_indices().enumerate() {
            if i == start {
                byte_start = bpos;
            }
            if i == end_exclusive {
                byte_end = bpos;
                break;
            }
        }
        s[byte_start..byte_end].to_string()
    }

    #[test]
    fn test_get_reference() {
        let mock_str =
            String::from("{ \"string\": [{ \"name\": \"test1\", \"value\": \"test1-value\" }] }");
        let element_json_file =
            ElementJsonFile::from_source("test.json", mock_str.clone()).unwrap();
        let references = ElementJsonFileReference::find_all(&element_json_file).unwrap();
        assert_eq!(references.len(), 1);
        assert_eq!(references[0].name_full_text(), "\"test1\"");
        assert_eq!(references[0].value_full_text(), "\"test1-value\"");
        assert_eq!(
            slice(
                &mock_str,
                references[0].name_start() as usize,
                references[0].name_end() as usize
            ),
            references[0].name_full_text()
        );
        assert_eq!(
            slice(
                &mock_str,
                references[0].value_start() as usize,
                references[0].value_end() as usize
            ),
            references[0].value_full_text()
        );
    }
}
