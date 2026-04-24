//! Persistence for the enabled-extensions list at
//! `~/.config/jackdaw/extensions.json`. Read on startup, rewritten
//! whenever the user toggles an extension.

use std::path::PathBuf;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::paths::config_dir;

/// On-disk shape.
#[derive(Serialize, Deserialize, Default)]
pub struct ExtensionsConfig {
    pub enabled: Vec<String>,
}

fn config_path() -> Option<PathBuf> {
    config_dir().map(|d| d.join("extensions.json"))
}

/// Read the enabled list from disk. Returns `None` if the file doesn't
/// exist; callers should interpret that as "enable everything".
pub fn read_enabled_list() -> Option<Vec<String>> {
    let path = config_path()?;
    let data = std::fs::read_to_string(&path).ok()?;
    let config: ExtensionsConfig = serde_json::from_str(&data).ok()?;
    Some(config.enabled)
}

pub fn init_enabled(id: impl Into<String>) {
    let id = id.into();
    let Some(mut enabled) = read_enabled_list() else {
        write_enabled_list(&[id]);
        return;
    };
    enabled.push(id);
    write_enabled_list(&enabled);
}

/// Write the currently-enabled list to disk.
pub fn write_enabled_list(enabled: &[String]) {
    let Some(path) = config_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let config = ExtensionsConfig {
        enabled: enabled.to_vec(),
    };
    if let Ok(data) = serde_json::to_string_pretty(&config) {
        let _ = std::fs::write(&path, data);
    }
}
/// Compute the current enabled list from the loaded `Extension` entities
/// and write it to disk.
pub fn persist_current_enabled(world: &mut World) {
    let mut query = world.query::<&crate::lifecycle::Extension>();
    let enabled: Vec<String> = query.iter(world).map(|e| e.id.clone()).collect();
    write_enabled_list(&enabled);
}
