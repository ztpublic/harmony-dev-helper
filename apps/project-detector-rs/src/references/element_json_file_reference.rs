use crate::error::{DetectorError, Result};
use crate::files::element_json_file::ElementJsonFile;
use std::path::{Path, PathBuf};

pub struct ElementJsonFileReference {
    element_type: String,
    name_start: u32,
    name_end: u32,
    name_text: String,
    value_start: u32,
    value_end: u32,
    value_text: String,
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
        element_type: String,
    ) -> Self {
        Self {
            name_start,
            name_end,
            name_text,
            value_start,
            value_end,
            value_text,
            element_type,
        }
    }

    fn byte_to_char_index(source_code: &str, byte_offset: usize) -> usize {
        source_code[..byte_offset].chars().count()
    }

    fn path(element_json_file: &ElementJsonFile) -> PathBuf {
        element_json_file.uri().as_path().to_path_buf()
    }

    fn node_text(node: tree_sitter::Node<'_>, source_code: &str, path: &Path) -> Result<String> {
        node.utf8_text(source_code.as_bytes())
            .map(|text| text.to_string())
            .map_err(|source| DetectorError::InvalidUtf8Text {
                path: path.to_path_buf(),
                message: source.to_string(),
            })
    }

    pub fn find_all(element_json_file: &ElementJsonFile) -> Result<Vec<ElementJsonFileReference>> {
        let mut reference = Vec::new();
        let parser = element_json_file.parser();
        let source_code = element_json_file.content();
        let path = Self::path(element_json_file);
        let tree = parser
            .lock()
            .map_err(|_| DetectorError::ParserPoisoned { path: path.clone() })?
            .parse(source_code, None)
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
                            Self::node_text(element_type_value, source_code, &path)?;
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
                            let key_text = Self::node_text(filtered_nodes[0], source_code, &path)?;
                            if key_text == "\"name\"" {
                                name_start = Some(Self::byte_to_char_index(
                                    source_code,
                                    filtered_nodes[1].start_byte(),
                                ));
                                name_end = Some(Self::byte_to_char_index(
                                    source_code,
                                    filtered_nodes[1].end_byte(),
                                ));
                                name_text =
                                    Some(Self::node_text(filtered_nodes[1], source_code, &path)?);
                            } else if key_text == "\"value\"" {
                                value_start = Some(Self::byte_to_char_index(
                                    source_code,
                                    filtered_nodes[1].start_byte(),
                                ));
                                value_end = Some(Self::byte_to_char_index(
                                    source_code,
                                    filtered_nodes[1].end_byte(),
                                ));
                                value_text =
                                    Some(Self::node_text(filtered_nodes[1], source_code, &path)?);
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
                                current_element_type.clone(),
                            ))
                        }
                    }
                }
            }
        }

        Ok(reference)
    }

    pub fn name_start(&self) -> u32 {
        self.name_start
    }

    pub fn name_end(&self) -> u32 {
        self.name_end
    }

    pub fn value_start(&self) -> u32 {
        self.value_start
    }

    pub fn value_end(&self) -> u32 {
        self.value_end
    }

    pub fn name_text(&self) -> &str {
        unquote(&self.name_text)
    }

    pub fn name_full_text(&self) -> &str {
        &self.name_text
    }

    pub fn value_text(&self) -> &str {
        unquote(&self.value_text)
    }

    pub fn value_full_text(&self) -> &str {
        &self.value_text
    }

    pub fn element_type(&self) -> &str {
        unquote(&self.element_type)
    }

    pub fn full_element_type(&self) -> &str {
        &self.element_type
    }

    pub fn to_ets_format(&self) -> String {
        format!("app.{}.{}", self.element_type(), self.name_text())
    }

    pub fn to_json_format(&self) -> String {
        format!("${}:{}", self.element_type(), self.name_text())
    }
}

fn unquote(text: &str) -> &str {
    let stripped = text.strip_prefix('"').unwrap_or(text);
    stripped.strip_suffix('"').unwrap_or(stripped)
}
