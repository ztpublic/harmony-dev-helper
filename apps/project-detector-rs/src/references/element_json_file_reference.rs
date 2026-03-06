use crate::error::{DetectorError, Result};
use crate::files::element_json_file::ElementJsonFile;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct ElementJsonFileReference {
    element_type: String,
    name_start: u32,
    name_end: u32,
    name_text: String,
    value_start: u32,
    value_end: u32,
    value_text: String,
    element_json_file: Arc<ElementJsonFile>,
}

impl ElementJsonFileReference {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        name_start: u32,
        name_end: u32,
        name_text: String,
        value_start: u32,
        value_end: u32,
        value_text: String,
        element_json_file: Arc<ElementJsonFile>,
        element_type: String,
    ) -> Self {
        Self {
            name_start,
            name_end,
            name_text,
            value_start,
            value_end,
            value_text,
            element_json_file,
            element_type,
        }
    }

    fn byte_to_char_index(source_code: &str, byte_offset: usize) -> usize {
        source_code[..byte_offset].chars().count()
    }

    fn path(element_json_file: &ElementJsonFile) -> PathBuf {
        PathBuf::from(element_json_file.get_uri().fs_path())
    }

    fn node_text(node: tree_sitter::Node<'_>, source_code: &str, path: &Path) -> Result<String> {
        node.utf8_text(source_code.as_bytes())
            .map(|text| text.to_string())
            .map_err(|source| DetectorError::InvalidUtf8Text {
                path: path.to_path_buf(),
                message: source.to_string(),
            })
    }

    pub fn find_all(
        element_json_file: &Arc<ElementJsonFile>,
    ) -> Result<Vec<ElementJsonFileReference>> {
        let mut reference = Vec::new();
        let parser = element_json_file.get_parser();
        let source_code = element_json_file.get_content();
        let path = Self::path(element_json_file);
        let tree = parser
            .lock()
            .map_err(|_| DetectorError::ParserPoisoned { path: path.clone() })?
            .parse(&source_code, None)
            .ok_or_else(|| DetectorError::TreeSitterParse { path: path.clone() })?;

        for child in tree.root_node().children(&mut tree.root_node().walk()) {
            if child.kind() != "object" {
                continue;
            }

            for element_type_key in child.children(&mut child.walk()) {
                if element_type_key.kind() != "pair" {
                    continue;
                }

                let mut current_element_type: String = String::new();
                for element_type_value in element_type_key.children(&mut element_type_key.walk()) {
                    if element_type_value.kind() == "string" {
                        current_element_type =
                            Self::node_text(element_type_value, &source_code, &path)?;
                        continue;
                    }
                    if element_type_value.kind() != "array" {
                        continue;
                    }

                    for element_name in element_type_value.children(&mut element_type_value.walk())
                    {
                        if element_name.kind() != "object" {
                            continue;
                        }

                        let mut name_start: Option<usize> = None;
                        let mut name_end: Option<usize> = None;
                        let mut name_text: Option<String> = None;
                        let mut value_start: Option<usize> = None;
                        let mut value_end: Option<usize> = None;
                        let mut value_text: Option<String> = None;

                        for element_name_key in element_name.children(&mut element_name.walk()) {
                            if element_name_key.kind() != "pair" {
                                continue;
                            }

                            let mut filtered_nodes = Vec::new();
                            for element_name_key_item in
                                element_name_key.children(&mut element_name_key.walk())
                            {
                                if element_name_key_item.kind() != "string" {
                                    continue;
                                }
                                filtered_nodes.push(element_name_key_item);
                            }
                            if filtered_nodes.len() != 2 {
                                continue;
                            }
                            let key_text = Self::node_text(filtered_nodes[0], &source_code, &path)?;
                            if key_text == "\"name\"" {
                                name_start = Some(Self::byte_to_char_index(
                                    &source_code,
                                    filtered_nodes[1].start_byte(),
                                ));
                                name_end = Some(Self::byte_to_char_index(
                                    &source_code,
                                    filtered_nodes[1].end_byte(),
                                ));
                                name_text =
                                    Some(Self::node_text(filtered_nodes[1], &source_code, &path)?);
                            } else if key_text == "\"value\"" {
                                value_start = Some(Self::byte_to_char_index(
                                    &source_code,
                                    filtered_nodes[1].start_byte(),
                                ));
                                value_end = Some(Self::byte_to_char_index(
                                    &source_code,
                                    filtered_nodes[1].end_byte(),
                                ));
                                value_text =
                                    Some(Self::node_text(filtered_nodes[1], &source_code, &path)?);
                            } else {
                                continue;
                            }
                        }

                        if let (
                            Some(name_start),
                            Some(name_end),
                            Some(name_text),
                            Some(value_start),
                            Some(value_end),
                            Some(value_text),
                        ) = (
                            name_start,
                            name_end,
                            name_text,
                            value_start,
                            value_end,
                            value_text,
                        ) {
                            reference.push(ElementJsonFileReference::new(
                                name_start as u32,
                                name_end as u32,
                                name_text,
                                value_start as u32,
                                value_end as u32,
                                value_text,
                                Arc::clone(element_json_file),
                                current_element_type.clone(),
                            ))
                        }
                    }
                }
            }
        }

        Ok(reference)
    }

    pub fn get_element_json_file(&self) -> Arc<ElementJsonFile> {
        Arc::clone(&self.element_json_file)
    }

    pub fn get_name_start(&self) -> u32 {
        self.name_start
    }

    pub fn get_name_end(&self) -> u32 {
        self.name_end
    }

    pub fn get_value_start(&self) -> u32 {
        self.value_start
    }

    pub fn get_value_end(&self) -> u32 {
        self.value_end
    }

    pub fn get_name_text(&self) -> String {
        let s = self.name_text.as_str();
        let s = if let Some(stripped) = s.strip_prefix('"') {
            stripped
        } else {
            s
        };
        let s = if let Some(stripped) = s.strip_suffix('"') {
            stripped
        } else {
            s
        };
        s.to_string()
    }

    pub fn get_name_full_text(&self) -> String {
        self.name_text.clone()
    }

    pub fn get_value_text(&self) -> String {
        let s = self.value_text.as_str();
        let s = if let Some(stripped) = s.strip_prefix('"') {
            stripped
        } else {
            s
        };
        let s = if let Some(stripped) = s.strip_suffix('"') {
            stripped
        } else {
            s
        };
        s.to_string()
    }

    pub fn get_value_full_text(&self) -> String {
        self.value_text.clone()
    }

    pub fn get_element_type(&self) -> String {
        let s = self.element_type.as_str();
        let s = if let Some(stripped) = s.strip_prefix('"') {
            stripped
        } else {
            s
        };
        let s = if let Some(stripped) = s.strip_suffix('"') {
            stripped
        } else {
            s
        };
        s.to_string()
    }

    pub fn get_full_element_type(&self) -> String {
        self.element_type.clone()
    }

    pub fn to_ets_format(&self) -> String {
        let text = self.get_name_text();
        format!("app.{}.{}", self.get_element_type(), text)
    }

    pub fn to_json_format(&self) -> String {
        let text = self.get_name_text();
        format!("${}:{}", self.get_element_type(), text)
    }
}
