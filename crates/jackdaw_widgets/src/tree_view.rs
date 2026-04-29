use bevy::prelude::*;

/// Marker for the tree view container
#[derive(Component)]
pub struct TreeView;

/// Links a tree row UI entity to the source entity it represents
#[derive(Component, Copy, Clone)]
#[relationship(relationship_target = TreeNodeSource)]
pub struct TreeNode(pub Entity);

/// Inverse relationship: source entity -> tree row
#[derive(Component)]
#[relationship_target(relationship = TreeNode)]
pub struct TreeNodeSource(Entity);

/// Marker for expand/collapse toggle button
#[derive(Component)]
pub struct TreeNodeExpandToggle;

/// Tracks whether a tree node is expanded
#[derive(Component, Default)]
pub struct TreeNodeExpanded(pub bool);

/// The clickable content area of a tree row (contains toggle + label)
#[derive(Component)]
pub struct TreeRowContent;

/// Marker on `TreeRowContent` when its source entity is selected
#[derive(Component)]
pub struct TreeRowSelected;

/// Container for displaying the row label
#[derive(Component)]
#[require(Text)]
pub struct TreeRowLabel;

/// Container for child rows (indented)
#[derive(Component)]
pub struct TreeRowChildren;

/// Tracks whether a tree node's children have been lazily populated.
/// Set to `true` after first expansion spawns children; prevents re-population on re-expand.
#[derive(Component, Default)]
pub struct TreeChildrenPopulated(pub bool);

/// Classifies a scene entity by type for sorting and colored dot display.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum EntityCategory {
    Camera,
    Light,
    Mesh,
    Scene,
    #[default]
    Entity,
}

/// Marker for the colored category dot in a tree row.
#[derive(Component)]
pub struct TreeRowDot;

/// Marker for the visibility toggle icon in a tree row.
#[derive(Component)]
pub struct TreeRowVisibilityToggle;

/// Event fired when a visibility toggle is clicked
#[derive(EntityEvent)]
pub struct TreeRowVisibilityToggled {
    #[event_target]
    pub entity: Entity,
    /// The source (scene) entity to toggle visibility
    pub source_entity: Entity,
}

/// Marker on the text input during inline rename
#[derive(Component)]
pub struct TreeRowInlineRename;

/// Tracks which tree row has keyboard focus (rendered with a focus ring).
#[derive(Resource, Default)]
pub struct TreeFocused(pub Option<Entity>);

/// Event fired when a tree row is clicked
#[derive(EntityEvent)]
pub struct TreeRowClicked {
    #[event_target]
    pub entity: Entity,
    /// The source entity this tree row represents
    pub source_entity: Entity,
}

/// Event fired when a tree row is dropped onto another tree row
#[derive(EntityEvent)]
pub struct TreeRowDropped {
    #[event_target]
    pub entity: Entity,
    /// The scene entity being moved
    pub dragged_source: Entity,
    /// The scene entity to become new parent
    pub target_source: Entity,
}

/// Event fired when a tree row is dropped onto the root container (deparent)
#[derive(EntityEvent)]
pub struct TreeRowDroppedOnRoot {
    #[event_target]
    pub entity: Entity,
    /// The scene entity being moved back to root
    pub dragged_source: Entity,
}

/// Event fired when an inline rename is committed
#[derive(EntityEvent)]
pub struct TreeRowRenamed {
    #[event_target]
    pub entity: Entity,
    /// The source (scene) entity
    pub source_entity: Entity,
    /// The new name entered by the user
    pub new_name: String,
}

/// Event fired to request starting an inline rename
#[derive(EntityEvent)]
pub struct TreeRowStartRename {
    #[event_target]
    pub entity: Entity,
    /// The source (scene) entity to rename
    pub source_entity: Entity,
}

pub struct TreeViewPlugin;

impl Plugin for TreeViewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TreeFocused>();
    }
}
