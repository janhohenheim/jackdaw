use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Top-level `.jsn/components.jsn` file.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct JsnComponentsFile {
    pub jsn: JsnComponentsHeader,
    /// All component definitions, keyed by full type path.
    pub components: HashMap<String, JsnComponentDef>,
}

impl Default for JsnComponentsFile {
    fn default() -> Self {
        Self {
            jsn: JsnComponentsHeader {
                format_version: [1, 0, 0],
            },
            components: HashMap::new(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct JsnComponentsHeader {
    pub format_version: [u32; 3],
}

/// A single component's editor definition.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct JsnComponentDef {
    /// Editor category ("Combat", "Physics", "Audio").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    /// Human-readable description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Icon identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// Field definitions, keyed by field name.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub fields: HashMap<String, JsnFieldDef>,
}

/// A single field's editor definition.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct JsnFieldDef {
    /// Reflect type path (e.g., "f32", "`bevy_math::Vec3`").
    #[serde(rename = "type")]
    pub type_path: String,
    /// Widget override: "slider", "`color_picker`", "`file_picker`",
    /// "dropdown", "`text_area`", "toggle", "angle", or auto-detected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub widget: Option<String>,
    /// Numeric range [min, max].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range: Option<[f64; 2]>,
    /// Numeric step size.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step: Option<f64>,
    /// Override display name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Tooltip description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Read-only in editor.
    #[serde(default, skip_serializing_if = "is_false")]
    pub read_only: bool,
    /// Hidden from inspector.
    #[serde(default, skip_serializing_if = "is_false")]
    pub hidden: bool,
    /// Conditional visibility: field path of a bool field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visible_when: Option<String>,
}

fn is_false(b: &bool) -> bool {
    !b
}

/// Extended registry response combining Bevy's type schema with Jackdaw's
/// component definitions. Returned by the `jackdaw/registry` BRP method.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct JsnRegistry {
    pub jsn: JsnRegistryHeader,
    /// ISO timestamp of when this registry was extracted.
    pub extracted_at: String,
    /// Source connection info.
    pub source: JsnRegistrySource,
    /// Raw `registry.schema` types from Bevy, keyed by type path.
    #[serde(default)]
    pub types: HashMap<String, serde_json::Value>,
    /// JSN component definitions (from `.jsn/components.jsn`), keyed by type path.
    #[serde(default)]
    pub components: HashMap<String, JsnComponentDef>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct JsnRegistryHeader {
    pub format_version: [u32; 3],
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct JsnRegistrySource {
    pub app_name: Option<String>,
    pub endpoint: String,
    pub bevy_version: String,
}
