use crate::error::{DetectorError, Result};
use crate::files::element_json_file::ElementJsonFile;
use std::path::Path;
use tree_sitter::Node;

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

    fn node_text(node: tree_sitter::Node<'_>, source_code: &str, path: &Path) -> Result<String> {
        node.utf8_text(source_code.as_bytes())
            .map(|text| text.to_string())
            .map_err(|source| DetectorError::InvalidUtf8Text {
                path: path.to_path_buf(),
                message: source.to_string(),
            })
    }

    pub fn find_all(element_json_file: &ElementJsonFile) -> Result<Vec<ElementJsonFileReference>> {
        let source_code = element_json_file.content();
        let path = element_json_file.uri().as_path();
        let tree = element_json_file.parse_tree()?;
        let Some(root_object) = root_object(tree.root_node()) else {
            return Ok(Vec::new());
        };

        let mut references = Vec::new();
        for pair in named_children(root_object, "pair") {
            let Some((element_type, value_array)) =
                string_keyed_array_pair(pair, source_code, path)?
            else {
                continue;
            };

            references.extend(references_for_element_array(
                value_array,
                &element_type,
                source_code,
                path,
            )?);
        }

        Ok(references)
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

#[derive(Debug)]
struct StringNodeValue {
    start: u32,
    end: u32,
    text: String,
}

impl StringNodeValue {
    fn from_node(node: Node<'_>, source_code: &str, path: &Path) -> Result<Option<Self>> {
        if node.kind() != "string" {
            return Ok(None);
        }

        Ok(Some(Self {
            start: ElementJsonFileReference::byte_to_char_index(source_code, node.start_byte())
                as u32,
            end: ElementJsonFileReference::byte_to_char_index(source_code, node.end_byte()) as u32,
            text: ElementJsonFileReference::node_text(node, source_code, path)?,
        }))
    }
}

fn root_object(root: Node<'_>) -> Option<Node<'_>> {
    named_children(root, "object").into_iter().next()
}

fn named_children<'tree>(node: Node<'tree>, kind: &str) -> Vec<Node<'tree>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .filter(|child| child.kind() == kind)
        .collect()
}

fn string_keyed_array_pair<'tree>(
    pair: Node<'tree>,
    source_code: &str,
    path: &Path,
) -> Result<Option<(String, Node<'tree>)>> {
    let Some(key_node) = pair.child_by_field_name("key") else {
        return Ok(None);
    };
    let Some(value_node) = pair.child_by_field_name("value") else {
        return Ok(None);
    };

    if key_node.kind() != "string" || value_node.kind() != "array" {
        return Ok(None);
    }

    Ok(Some((
        ElementJsonFileReference::node_text(key_node, source_code, path)?,
        value_node,
    )))
}

fn references_for_element_array(
    value_array: Node<'_>,
    element_type: &str,
    source_code: &str,
    path: &Path,
) -> Result<Vec<ElementJsonFileReference>> {
    let mut references = Vec::new();

    for reference_object in named_children(value_array, "object") {
        if let Some(reference) =
            parse_reference_object(reference_object, element_type, source_code, path)?
        {
            references.push(reference);
        }
    }

    Ok(references)
}

fn parse_reference_object(
    reference_object: Node<'_>,
    element_type: &str,
    source_code: &str,
    path: &Path,
) -> Result<Option<ElementJsonFileReference>> {
    let mut name = None;
    let mut value = None;

    for field in named_children(reference_object, "pair") {
        let Some((field_name, field_value)) = string_property(field, source_code, path)? else {
            continue;
        };

        match field_name.as_str() {
            "\"name\"" => name = Some(field_value),
            "\"value\"" => value = Some(field_value),
            _ => {}
        }
    }

    let (name, value) = match (name, value) {
        (Some(name), Some(value)) => (name, value),
        _ => return Ok(None),
    };

    Ok(Some(ElementJsonFileReference::new(
        name.start,
        name.end,
        name.text,
        value.start,
        value.end,
        value.text,
        element_type.to_string(),
    )))
}

fn string_property(
    pair: Node<'_>,
    source_code: &str,
    path: &Path,
) -> Result<Option<(String, StringNodeValue)>> {
    let Some(key_node) = pair.child_by_field_name("key") else {
        return Ok(None);
    };
    let Some(value_node) = pair.child_by_field_name("value") else {
        return Ok(None);
    };

    if key_node.kind() != "string" {
        return Ok(None);
    }

    let Some(value) = StringNodeValue::from_node(value_node, source_code, path)? else {
        return Ok(None);
    };

    Ok(Some((
        ElementJsonFileReference::node_text(key_node, source_code, path)?,
        value,
    )))
}

fn unquote(text: &str) -> &str {
    let stripped = text.strip_prefix('"').unwrap_or(text);
    stripped.strip_suffix('"').unwrap_or(stripped)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::files::element_json_file::ElementJsonFile;

    fn slice(source: &str, start: usize, end_exclusive: usize) -> String {
        let mut byte_start = 0usize;
        let mut byte_end = source.len();
        for (i, (byte_offset, _)) in source.char_indices().enumerate() {
            if i == start {
                byte_start = byte_offset;
            }
            if i == end_exclusive {
                byte_end = byte_offset;
                break;
            }
        }
        source[byte_start..byte_end].to_string()
    }

    fn references_for(source: &str) -> Vec<ElementJsonFileReference> {
        let element_json_file = ElementJsonFile::from_source("test.json", source.to_string())
            .expect("test fixture should build");
        ElementJsonFileReference::find_all(&element_json_file)
            .expect("reference extraction should succeed")
    }

    #[test]
    fn extracts_name_and_value_ranges() {
        let source = "{ \"string\": [{ \"name\": \"test1\", \"value\": \"test1-value\" }] }";
        let references = references_for(source);

        assert_eq!(references.len(), 1);
        assert_eq!(references[0].name_full_text(), "\"test1\"");
        assert_eq!(references[0].value_full_text(), "\"test1-value\"");
        assert_eq!(
            slice(
                source,
                references[0].name_start() as usize,
                references[0].name_end() as usize,
            ),
            references[0].name_full_text()
        );
        assert_eq!(
            slice(
                source,
                references[0].value_start() as usize,
                references[0].value_end() as usize,
            ),
            references[0].value_full_text()
        );
    }

    #[test]
    fn skips_objects_missing_name() {
        let references = references_for(
            "{ \"string\": [{ \"value\": \"missing-name\" }, { \"name\": \"ok\", \"value\": \"v\" }] }",
        );

        assert_eq!(references.len(), 1);
        assert_eq!(references[0].name_text(), "ok");
    }

    #[test]
    fn skips_objects_missing_value() {
        let references = references_for(
            "{ \"string\": [{ \"name\": \"missing-value\" }, { \"name\": \"ok\", \"value\": \"v\" }] }",
        );

        assert_eq!(references.len(), 1);
        assert_eq!(references[0].value_text(), "v");
    }

    #[test]
    fn skips_non_string_name_or_value_fields() {
        let references = references_for(
            r#"{ "string": [
                { "name": 1, "value": "ignored" },
                { "name": "ignored", "value": true },
                { "name": "ok", "value": "v" }
            ] }"#,
        );

        assert_eq!(references.len(), 1);
        assert_eq!(references[0].to_json_format(), "$string:ok");
    }

    #[test]
    fn ignores_nested_unrelated_json_nodes() {
        let references = references_for(
            r#"{ "string": [
                {
                    "name": "ok",
                    "value": "v",
                    "metadata": { "name": "nested", "value": "ignored" }
                }
            ] }"#,
        );

        assert_eq!(references.len(), 1);
        assert_eq!(references[0].name_text(), "ok");
        assert_eq!(references[0].value_text(), "v");
    }
}
