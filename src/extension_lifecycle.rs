use bevy::prelude::*;
use jackdaw_api::prelude::*;

use crate::extensions_config::resolve_enabled_list;

pub(super) fn plugin(app: &mut App) {
    // Must run after every plugin's `finish()`: BEI initializes
    // `ContextInstances<PreUpdate>` there, and spawning a context
    // entity before that resource exists panics.
    app.add_systems(Startup, apply_enabled_extensions_startup);
}
/// Enable every catalog entry `resolve_enabled_list` reports as on.
fn apply_enabled_extensions_startup(world: &mut World) {
    let to_enable = resolve_enabled_list(world);
    for name in &to_enable {
        enable_extension(world, name);
    }
}

/// Unload an extension. Despawns the root entity; the cascade and
/// cleanup observers handle the rest.
pub fn unload_extension(world: &mut World, ext_entity: Entity) {
    let ext_name = world
        .get::<Extension>(ext_entity)
        .map(|e| e.name.clone())
        .unwrap_or_default();
    info!("Unloading extension: {}", ext_name);

    if let Some(stored) = world.entity_mut(ext_entity).take::<StoredExtension>() {
        stored.0.unregister(world, ext_entity);
    }
    if let Ok(ec) = world.get_entity_mut(ext_entity) {
        ec.despawn();
    }
}

/// Enable a named extension via the catalog. Returns the new extension
/// entity, or `None` if the name is unknown or already loaded.
pub fn enable_extension(world: &mut World, name: &str) -> Option<Entity> {
    {
        let mut query = world.query::<&Extension>();
        if query.iter(world).any(|e| e.name == name) {
            return None;
        }
    }

    let extension = world.resource::<ExtensionCatalog>().construct(name)?;
    Some(load_static_extension(world, extension))
}

/// Load an extension statically. Spawns an `Extension` entity, runs
/// `extension.register()` against it, returns the entity.
///
/// Takes `&mut World` (not `&mut App`) so this can be called from
/// world-scoped contexts like observer callbacks. BEI input context
/// registration belongs in
/// [`JackdawExtension::register_input_context`], which is called at
/// catalog registration time with App access.
pub fn load_static_extension(world: &mut World, extension: Box<dyn JackdawExtension>) -> Entity {
    let name = extension.dyn_name();
    info!("Loading extension: {}", name);

    let extension_entity = world.spawn(Extension { name }).id();

    let mut ctx = ExtensionContext::new(world, extension_entity);
    extension.register(&mut ctx);

    // Store the extension trait object on the entity so `unload_extension`
    // can call `unregister` before despawn.
    world
        .entity_mut(extension_entity)
        .insert(StoredExtension(extension));

    extension_entity
}

/// Disable a named extension by despawning its root entity.
pub fn disable_extension(world: &mut World, name: &str) -> bool {
    let mut query = world.query::<(Entity, &Extension)>();
    let Some(ext_entity) = query
        .iter(world)
        .find(|(_, e)| e.name == name)
        .map(|(e, _)| e)
    else {
        return false;
    };
    unload_extension(world, ext_entity);
    true
}

/// Internal component holding the extension trait object for the duration
/// of its lifetime. Used by `unload_extension` to invoke the optional
/// `unregister` hook before despawning.
#[derive(Component)]
pub(crate) struct StoredExtension(pub(crate) Box<dyn JackdawExtension>);
