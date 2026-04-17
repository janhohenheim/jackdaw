//! `File > Extensions...` dialog. Lets the user enable/disable compiled-in
//! extensions at runtime. Changes are applied immediately via
//! `enable_extension` / `disable_extension` and persisted to
//! `~/.config/jackdaw/extensions.json`.

use bevy::prelude::*;
use jackdaw_api::{Extension, ExtensionCatalog, ExtensionKind};
use jackdaw_feathers::{
    checkbox::{CheckboxCommitEvent, CheckboxProps, checkbox},
    dialog::{CloseDialogEvent, DialogChildrenSlot, OpenDialogEvent},
    icons::{EditorFont, IconFont},
    tokens,
};

use crate::extensions_config;

pub struct ExtensionsDialogPlugin;

impl Plugin for ExtensionsDialogPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ExtensionsDialogOpen>()
            .add_systems(Update, populate_extensions_dialog)
            .add_observer(on_extension_checkbox_commit)
            .add_observer(on_dialog_closed);
    }
}

/// Clear the open flag whenever any dialog closes. Safe because
/// [`populate_extensions_dialog`] also checks for existing checkboxes
/// before filling the slot, so it won't double-populate between closes.
fn on_dialog_closed(_: On<CloseDialogEvent>, mut open: ResMut<ExtensionsDialogOpen>) {
    open.0 = false;
}

/// Set to `true` while the dialog is being shown. Used by the populate
/// system to know whether to fill the dialog's children slot.
#[derive(Resource, Default)]
struct ExtensionsDialogOpen(bool);

/// Marks a checkbox as belonging to the extensions dialog. Stores the
/// extension name so the commit observer knows which one to toggle.
#[derive(Component)]
struct ExtensionCheckbox {
    extension_name: String,
}

/// Opened from `File > Extensions...`. Called from the menu action handler.
pub fn open_extensions_dialog(world: &mut World) {
    world.resource_mut::<ExtensionsDialogOpen>().0 = true;
    world.trigger(
        OpenDialogEvent::new("Extensions", "Close")
            .without_cancel()
            .with_max_width(Val::Px(380.0)),
    );
}

/// Populate the dialog's children slot with a row per catalog entry.
/// Runs each frame and short-circuits unless the dialog is open and
/// hasn't been populated yet.
///
/// The slot is detected by marker presence rather than by filtering on
/// `&Children`. A freshly-spawned `DialogChildrenSlot` with no children
/// has no `Children` component at all, which would cause the filter to
/// never match. Checking for existing `ExtensionCheckbox` entities is
/// how this system avoids re-populating the same dialog.
fn populate_extensions_dialog(
    mut commands: Commands,
    catalog: Res<ExtensionCatalog>,
    open: Res<ExtensionsDialogOpen>,
    slots: Query<Entity, With<DialogChildrenSlot>>,
    loaded: Query<&Extension>,
    editor_font: Res<EditorFont>,
    icon_font: Res<IconFont>,
    existing: Query<(), With<ExtensionCheckbox>>,
) {
    if !open.0 {
        return;
    }
    if !existing.is_empty() {
        return;
    }
    let Some(slot_entity) = slots.iter().next() else {
        return;
    };

    let font = editor_font.0.clone();
    let ifont = icon_font.0.clone();

    // Collect (name, is_enabled) for each catalog entry, split into
    // Built-in (Jackdaw feature areas) and Custom (example and
    // third-party extensions). Group membership is taken from each
    // extension's declared `ExtensionKind`, captured at
    // `register_extension` time. Adding a new built-in is therefore a
    // one-liner on the extension itself.
    let enabled_names: std::collections::HashSet<String> =
        loaded.iter().map(|e| e.name.clone()).collect();
    let mut builtin_rows: Vec<(String, bool)> = Vec::new();
    let mut custom_rows: Vec<(String, bool)> = Vec::new();
    for (name, kind) in catalog.iter_with_kind() {
        let row = (name.to_string(), enabled_names.contains(name));
        match kind {
            ExtensionKind::Builtin => builtin_rows.push(row),
            ExtensionKind::Custom => custom_rows.push(row),
        }
    }
    builtin_rows.sort_by(|a, b| a.0.cmp(&b.0));
    custom_rows.sort_by(|a, b| a.0.cmp(&b.0));

    let list = commands
        .spawn((
            ChildOf(slot_entity),
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(tokens::SPACING_XS),
                min_width: Val::Px(280.0),
                ..default()
            },
        ))
        .id();

    spawn_section_header(&mut commands, list, "Built-in");
    for (name, checked) in builtin_rows {
        let label = prettify(&name);
        commands.spawn((
            ChildOf(list),
            ExtensionCheckbox {
                extension_name: name.clone(),
            },
            checkbox(CheckboxProps::new(label).checked(checked), &font, &ifont),
        ));
    }

    spawn_section_header(&mut commands, list, "Custom");
    if custom_rows.is_empty() {
        // Empty-state hint so users learn where custom extensions will
        // appear. Without it the section header sits alone and reads as
        // broken UI.
        commands.spawn((
            ChildOf(list),
            Node {
                padding: UiRect::axes(Val::Px(tokens::SPACING_LG), Val::Px(tokens::SPACING_SM)),
                ..default()
            },
            children![(
                Text::new("No custom extensions installed"),
                TextFont {
                    font_size: tokens::FONT_SM,
                    ..default()
                },
                TextColor(tokens::TEXT_SECONDARY),
            )],
        ));
    } else {
        for (name, checked) in custom_rows {
            let label = prettify(&name);
            commands.spawn((
                ChildOf(list),
                ExtensionCheckbox {
                    extension_name: name.clone(),
                },
                checkbox(CheckboxProps::new(label).checked(checked), &font, &ifont),
            ));
        }
    }
}

/// Small underlined heading matching the `ComponentPickerSectionHeader`
/// look from the Add Component dialog, so the two modals feel uniform.
fn spawn_section_header(commands: &mut Commands, list: Entity, label: &str) {
    let header = commands
        .spawn((
            ChildOf(list),
            Node {
                padding: UiRect::new(
                    Val::Px(tokens::SPACING_LG),
                    Val::Px(tokens::SPACING_LG),
                    Val::Px(tokens::SPACING_MD),
                    Val::Px(tokens::SPACING_XS),
                ),
                width: Val::Percent(100.0),
                border: UiRect::bottom(Val::Px(1.0)),
                ..default()
            },
            BorderColor::all(tokens::BORDER_SUBTLE),
        ))
        .id();

    commands.spawn((
        ChildOf(header),
        Text::new(label.to_string()),
        TextFont {
            font_size: tokens::FONT_SM,
            ..default()
        },
        TextColor(tokens::TEXT_SECONDARY),
    ));
}

/// Observer: when an extension checkbox commits, enable/disable the
/// matching extension and rewrite the enabled list.
fn on_extension_checkbox_commit(
    event: On<CheckboxCommitEvent>,
    checkboxes: Query<&ExtensionCheckbox>,
    mut commands: Commands,
) {
    let Ok(cb) = checkboxes.get(event.entity) else {
        return;
    };
    let name = cb.extension_name.clone();
    let checked = event.checked;

    commands.queue(move |world: &mut World| {
        if checked {
            jackdaw_api::enable_extension(world, &name);
        } else {
            jackdaw_api::disable_extension(world, &name);
        }
        extensions_config::persist_current_enabled(world);
    });
}

/// Convert `"jackdaw.asset_browser"` → `"Asset Browser"`.
fn prettify(name: &str) -> String {
    let stripped = name.strip_prefix("jackdaw.").unwrap_or(name);
    let mut out = String::new();
    for (i, part) in stripped.split(&['_', '.'][..]).enumerate() {
        if i > 0 {
            out.push(' ');
        }
        let mut chars = part.chars();
        if let Some(c) = chars.next() {
            out.extend(c.to_uppercase());
            out.push_str(chars.as_str());
        }
    }
    out
}
