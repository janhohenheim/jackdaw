//! Persistence for the "enabled extensions" list.
//!
//! Stores a JSON file at `~/.config/jackdaw/extensions.json`:
//!
//! ```json
//! {
//!   "enabled": [
//!     "core_windows",
//!     "inspector",
//!     "sample"
//!   ]
//! }
//! ```
//!
//! Read once on editor startup to decide which entries in the
//! [`ExtensionCatalog`] to enable. Rewritten whenever the user toggles
//! an extension in the Extensions dialog.

use std::collections::HashSet;
use std::path::PathBuf;

use bevy::prelude::*;
use jackdaw_api::{ExtensionCatalog, ExtensionKind};
use serde::{Deserialize, Serialize};

/// On-disk shape.
#[derive(Serialize, Deserialize, Default)]
pub struct ExtensionsConfig {
    pub enabled: Vec<String>,
}

fn config_path() -> Option<PathBuf> {
    crate::project::config_dir().map(|d| d.join("extensions.json"))
}

/// Read the enabled list from disk. Returns `None` if the file doesn't
/// exist; callers should interpret that as "enable everything".
pub fn read_enabled_list() -> Option<Vec<String>> {
    let path = config_path()?;
    let data = std::fs::read_to_string(&path).ok()?;
    let config: ExtensionsConfig = serde_json::from_str(&data).ok()?;
    Some(config.enabled)
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

/// Resolve which catalog entries should be enabled on startup given the
/// persisted list, if any. Called by the Startup system in `lib.rs`.
///
/// Includes a one-time upgrade migration: if the saved list predates the
/// built-in feature-area extensions (none of its entries are built-ins),
/// every catalog entry is enabled. The next toggle rewrites the file
/// with the full list. Once the file records at least one built-in, the
/// user's recorded preferences are trusted exactly as written, so an
/// intentional disable of `inspector` stays disabled across restarts.
///
/// "Which names are built-ins" comes from the catalog itself: each
/// extension declares its [`ExtensionKind`] on the `JackdawExtension`
/// trait and registration captures it.
pub fn resolve_enabled_list(world: &World) -> Vec<String> {
    let catalog = world.resource::<ExtensionCatalog>();
    let available: Vec<String> = catalog.iter().map(|s| s.to_string()).collect();
    let builtins: HashSet<String> = catalog
        .iter_with_kind()
        .filter(|(_, kind)| *kind == ExtensionKind::Builtin)
        .map(|(name, _)| name.to_string())
        .collect();

    match read_enabled_list() {
        Some(list) => {
            let on_disk: HashSet<String> = list.into_iter().collect();
            // Pre-dogfood files contain none of the built-in names.
            // Treat those as legacy and fall back to "enable everything"
            // so the editor stays usable. The next toggle rewrites the
            // file with the complete list.
            let has_any_builtin = builtins.iter().any(|name| on_disk.contains(name));
            if !has_any_builtin {
                return available;
            }
            available
                .into_iter()
                .filter(|n| on_disk.contains(n))
                .collect()
        }
        None => available, // first run: enable everything
    }
}

/// Compute the current enabled list from the loaded `Extension` entities
/// and write it to disk.
pub fn persist_current_enabled(world: &mut World) {
    let mut query = world.query::<&jackdaw_api::Extension>();
    let enabled: Vec<String> = query.iter(world).map(|e| e.name.clone()).collect();
    write_enabled_list(&enabled);
}
