use once_cell::sync::Lazy;

use crate::error::ImageManagerError;
use crate::types::{EmulatorEntry, ProductCatalog, ProductConfigItem};

const DEFAULT_PRODUCT_CONFIG_TS: &str =
    include_str!("../../../external-src/image-manager-main/src/default-product-config.ts");
const DEFAULT_EMULATOR_CONFIG_TS: &str =
    include_str!("../../../external-src/image-manager-main/src/default-emulator-config.ts");

pub(crate) static DEFAULT_PRODUCT_CATALOG: Lazy<Result<ProductCatalog, ImageManagerError>> =
    Lazy::new(|| {
        let raw = extract_exported_literal(DEFAULT_PRODUCT_CONFIG_TS, "default-product-config")?;
        let value: indexmap::IndexMap<String, Vec<ProductConfigItem>> = serde_json5::from_str(raw)
            .map_err(|source| ImageManagerError::Json5 {
                label: "default-product-config",
                source,
            })?;
        Ok(ProductCatalog::from_sections(value))
    });

pub(crate) static DEFAULT_EMULATOR_ENTRIES: Lazy<Result<Vec<EmulatorEntry>, ImageManagerError>> =
    Lazy::new(|| {
        let raw = extract_exported_literal(DEFAULT_EMULATOR_CONFIG_TS, "default-emulator-config")?;
        serde_json5::from_str(raw).map_err(|source| ImageManagerError::Json5 {
            label: "default-emulator-config",
            source,
        })
    });

fn extract_exported_literal<'a>(
    source: &'a str,
    label: &'static str,
) -> Result<&'a str, ImageManagerError> {
    let export_start =
        source
            .find("export default")
            .ok_or_else(|| ImageManagerError::DefaultConfig {
                label,
                message: "missing `export default`".to_string(),
            })?;
    let body = &source[export_start + "export default".len()..];
    let literal_end = body.rfind("satisfies").unwrap_or(body.len());
    let literal = body[..literal_end].trim().trim_end_matches(';').trim();
    if literal.is_empty() {
        return Err(ImageManagerError::DefaultConfig {
            label,
            message: "empty exported literal".to_string(),
        });
    }
    Ok(literal)
}
