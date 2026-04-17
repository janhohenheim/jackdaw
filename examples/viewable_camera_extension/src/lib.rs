//! Viewable Camera extension.
//!
//! Demonstrates a more substantial extension that integrates with the
//! editor's viewport camera and the undo/redo system:
//!
//! - **F6** places a "viewable" camera entity at the current editor camera
//!   position. The placement is undoable via a custom `EditorCommand`.
//! - **F7** toggles between the editor view and looking *through* the
//!   selected viewable camera (or the only one, if there's just one).
//!   Preview is a view state, not scene data, so it is intentionally NOT
//!   recorded in the undo history.
//!
//! Undo / redo flow:
//!
//! 1. Press F6. Camera spawned at P0; undo stack: `[Place Viewable Camera]`.
//! 2. Select the camera in the hierarchy, drag it with the gizmo to P1.
//!    Jackdaw's existing `SetTransform` command is pushed.
//! 3. Drag to P2. Another `SetTransform` is pushed.
//! 4. Press F7. The viewport now looks through the camera at P2. No
//!    history entry is created.
//! 5. Press F7 again. Back to the editor view.
//! 6. Ctrl+Z twice. Camera returns to P0 (the two moves undo).
//! 7. Ctrl+Z. Camera despawns (placement undoes). If preview was active,
//!    the `Remove<ViewableCamera>` observer exits preview automatically so
//!    the editor view snaps back on.

use bevy::camera::RenderTarget;
use bevy::ecs::system::SystemId;
use bevy::prelude::*;
use bevy_enhanced_input::prelude::*;
use jackdaw_api::prelude::*;
use jackdaw_commands::EditorCommand;

// ============================================================================
// Extension
// ============================================================================

pub struct ViewableCameraExtension;

impl JackdawExtension for ViewableCameraExtension {
    fn name(&self) -> &str {
        "viewable_camera"
    }

    fn register_input_contexts(&self, app: &mut App) {
        app.add_input_context::<ViewableCameraContext>();
    }

    fn register(&self, ctx: &mut ExtensionContext) {
        ctx.world().init_resource::<CameraPreviewState>();

        ctx.register_operator::<PlaceViewableCamera>();
        ctx.register_operator::<ToggleCameraPreview>();

        // Contribute to the editor's Add menu. Menu entries dispatch
        // through the same pipeline as keybinds, so clicking
        // "Add > Viewable Camera" still produces exactly one undo entry.
        ctx.register_menu_entry(MenuEntryDescriptor {
            menu: "Add".into(),
            label: "Viewable Camera".into(),
            operator_id: PlaceViewableCamera::ID,
        });

        // BEI context + action bindings. Spawned as a child of the
        // extension entity so both are torn down on disable.
        ctx.spawn((
            ViewableCameraContext,
            actions!(ViewableCameraContext[
                (Action::<PlaceViewableCamera>::new(), bindings![KeyCode::F6]),
                (Action::<ToggleCameraPreview>::new(), bindings![KeyCode::F7]),
            ]),
        ));

        // Observer: if the currently-previewed camera gets despawned
        // (e.g. the user undoes the placement while preview is active),
        // exit preview so the viewport falls back to the editor camera.
        let ext_entity = ctx.entity();
        let observer = Observer::new(
            move |trigger: On<Remove, ViewableCamera>,
                  mut state: ResMut<CameraPreviewState>,
                  mut commands: Commands| {
                if state.active == Some(trigger.event_target()) {
                    state.active = None;
                    commands.queue(|world: &mut World| {
                        restore_editor_camera(world);
                    });
                }
            },
        );
        ctx.world().spawn((observer, ChildOf(ext_entity)));
    }
}

// ============================================================================
// Components, resources, markers
// ============================================================================

/// BEI context component. One per extension gives key-binding isolation.
#[derive(Component, Default)]
pub struct ViewableCameraContext;

/// Marker component on camera entities created by this extension. These
/// are scene entities (serialized with the scene), not editor-local
/// machinery.
#[derive(Component, Default, Reflect)]
pub struct ViewableCamera;

/// Editor-local state tracking the currently-previewed camera, if any.
/// Not saved to the scene; reset on editor restart.
#[derive(Resource, Default)]
struct CameraPreviewState {
    /// The `ViewableCamera` entity currently being previewed.
    active: Option<Entity>,
    /// Saved (editor-camera entity, render target) captured when entering
    /// preview so it can be restored on exit.
    saved: Option<(Entity, RenderTarget)>,
}

// ============================================================================
// Operators
// ============================================================================

/// Place a new viewable camera at the editor camera's current position.
/// One-shot, undoable.
#[derive(Default, InputAction)]
#[action_output(bool)]
pub struct PlaceViewableCamera;

impl Operator for PlaceViewableCamera {
    const ID: &'static str = "viewable_camera.place";
    const LABEL: &'static str = "Place Viewable Camera";
    const DESCRIPTION: &'static str = "Place a camera at the viewport position";

    fn register_execute(commands: &mut Commands) -> SystemId<(), OperatorResult> {
        commands.register_system(place_viewable_camera)
    }
}

fn place_viewable_camera(world: &mut World) -> OperatorResult {
    // Copy the editor camera's transform so the new camera starts where
    // the user is already looking. That makes "look through" visually
    // intuitive on the first toggle.
    let spawn_transform = find_editor_camera(world)
        .and_then(|e| world.get::<Transform>(e).copied())
        .unwrap_or_default();

    let mut cmd: Box<dyn EditorCommand> = Box::new(PlaceViewableCameraCommand {
        spawned: None,
        transform: spawn_transform,
    });
    cmd.execute(world);
    world.resource_mut::<OperatorCommandBuffer>().record(cmd);
    OperatorResult::Finished
}

/// Toggle "look through the viewable camera" vs. the editor view. Not
/// undoable: preview is view state, not a scene edit.
#[derive(Default, InputAction)]
#[action_output(bool)]
pub struct ToggleCameraPreview;

impl Operator for ToggleCameraPreview {
    const ID: &'static str = "viewable_camera.toggle_preview";
    const LABEL: &'static str = "Toggle Camera Preview";
    const DESCRIPTION: &'static str = "Look through the selected viewable camera";

    fn register_execute(commands: &mut Commands) -> SystemId<(), OperatorResult> {
        commands.register_system(toggle_preview)
    }
}

fn toggle_preview(world: &mut World) -> OperatorResult {
    let currently_active = world.resource::<CameraPreviewState>().active;
    if currently_active.is_some() {
        restore_editor_camera(world);
        info!("Exited viewable-camera preview");
    } else {
        let Some(target) = pick_preview_target(world) else {
            warn!("No viewable camera to preview; press F6 to place one first");
            return OperatorResult::Cancelled;
        };
        match enter_preview(world, target) {
            Ok(()) => info!("Entered preview through viewable camera {target:?}"),
            Err(reason) => warn!("Preview failed: {reason}"),
        }
    }
    OperatorResult::Finished
}

// ============================================================================
// Preview swap: RenderTarget handoff between editor cam and viewable cam
// ============================================================================

/// Walk the world looking for a camera that is currently acting as the
/// editor viewport: one with a `Camera3d`, an `Image` render target, and
/// `is_active = true`, that is NOT a viewable camera.
fn find_editor_camera(world: &mut World) -> Option<Entity> {
    let mut q = world.query_filtered::<
        (Entity, &Camera, &RenderTarget),
        (With<Camera3d>, Without<ViewableCamera>),
    >();
    for (entity, camera, target) in q.iter(world) {
        if camera.is_active && matches!(target, RenderTarget::Image(_)) {
            return Some(entity);
        }
    }
    None
}

/// Pick which viewable camera to preview. Preference order:
/// 1. Primary selection if it's a viewable camera.
/// 2. The only viewable camera in the scene, if there's exactly one.
/// 3. None.
fn pick_preview_target(world: &mut World) -> Option<Entity> {
    let cams: Vec<Entity> = world
        .query_filtered::<Entity, With<ViewableCamera>>()
        .iter(world)
        .collect();
    if cams.len() == 1 {
        return Some(cams[0]);
    }
    // Fall back to first matching entity. (Extending this to honour the
    // editor's `Selection` resource would require depending on the main
    // jackdaw crate; the simple rule above is enough for the demo.)
    cams.first().copied()
}

fn enter_preview(world: &mut World, target: Entity) -> Result<(), &'static str> {
    let Some(editor_cam) = find_editor_camera(world) else {
        return Err("couldn't find the editor viewport camera");
    };
    let render_target = world
        .get::<RenderTarget>(editor_cam)
        .cloned()
        .ok_or("editor camera has no RenderTarget")?;

    // Disable the editor camera.
    if let Some(mut c) = world.get_mut::<Camera>(editor_cam) {
        c.is_active = false;
    }

    // Hand its render target to the viewable camera and activate it.
    if let Ok(mut ec) = world.get_entity_mut(target) {
        ec.insert(render_target.clone());
    }
    if let Some(mut c) = world.get_mut::<Camera>(target) {
        c.is_active = true;
    }
    // Bevy's `camera_system` only recomputes `camera.computed.target_info`
    // when the Projection is marked changed, the viewport size differs,
    // or the underlying image/window changes. Replacing `RenderTarget`
    // via `insert` triggers none of those, so without this touch the
    // viewable camera keeps a stale `target_info` from its previous
    // RenderTarget (1x1 for the `RenderTarget::None` default), renders
    // into a 1-pixel region, and the viewport appears empty. Marking
    // Projection changed forces a recompute against the freshly-inserted
    // Image target.
    if let Some(mut proj) = world.get_mut::<Projection>(target) {
        proj.set_changed();
    }

    let mut state = world.resource_mut::<CameraPreviewState>();
    state.active = Some(target);
    state.saved = Some((editor_cam, render_target));
    Ok(())
}

/// Exit preview: revoke the render target from the viewable camera and
/// re-activate the editor camera. Safe to call even if we're not
/// currently previewing.
fn restore_editor_camera(world: &mut World) {
    let Some((editor_cam, _target)) = world.resource_mut::<CameraPreviewState>().saved.take()
    else {
        return;
    };
    let active = world.resource_mut::<CameraPreviewState>().active.take();

    if let Some(preview) = active {
        if let Ok(mut ec) = world.get_entity_mut(preview) {
            // Replace the Image target with None rather than removing
            // the component outright. `Camera`'s required-components
            // contract expects `RenderTarget` to exist, and leaving an
            // Image target on an inactive camera can still trigger
            // ambiguity warnings when preview is re-entered.
            ec.insert(RenderTarget::None {
                size: UVec2::splat(1),
            });
        }
        if let Some(mut c) = world.get_mut::<Camera>(preview) {
            c.is_active = false;
        }
        // Match the Projection poke in `enter_preview` so Bevy's
        // `camera_system` notices the RenderTarget swap. Not strictly
        // required while the camera is inactive, but keeps both swap
        // directions symmetric and avoids stale `target_info` if another
        // system activates the camera before the next preview toggle.
        if let Some(mut proj) = world.get_mut::<Projection>(preview) {
            proj.set_changed();
        }
    }

    if let Some(mut c) = world.get_mut::<Camera>(editor_cam) {
        c.is_active = true;
    }
}

// ============================================================================
// Custom EditorCommand: spawn / despawn a viewable camera
// ============================================================================

/// Undoable placement of a viewable camera.
///
/// `execute` spawns a fresh camera entity with the stored transform and
/// caches the new id. `undo` despawns whatever was last spawned. Redoing
/// (re-executing after undo) spawns again with a new id. Downstream
/// references to the old id become invalid, which is fine for the scene
/// tree and inspector: both resolve entities by id on every frame.
struct PlaceViewableCameraCommand {
    spawned: Option<Entity>,
    transform: Transform,
}

impl EditorCommand for PlaceViewableCameraCommand {
    fn execute(&mut self, world: &mut World) {
        let entity = world
            .spawn((
                Name::new("Viewable Camera"),
                ViewableCamera,
                Camera3d::default(),
                Camera {
                    // is_active = false until we hand it the render target
                    // via toggle preview. Keeps it from competing with the
                    // editor viewport camera for rendering.
                    is_active: false,
                    order: -1,
                    ..default()
                },
                // Explicit `None` target. Without this, `Camera`'s
                // required components default to
                // `RenderTarget::Window(Primary)`. If `is_active` ever
                // flips true by accident, that would render the scene on
                // top of the editor UI. `None` keeps the camera inert
                // until `enter_preview` swaps in the viewport image
                // target.
                RenderTarget::None {
                    size: UVec2::splat(1),
                },
                self.transform,
                Visibility::default(),
            ))
            .id();
        self.spawned = Some(entity);
    }

    fn undo(&mut self, world: &mut World) {
        if let Some(entity) = self.spawned.take()
            && let Ok(ec) = world.get_entity_mut(entity)
        {
            ec.despawn();
        }
    }

    fn description(&self) -> &str {
        "Place Viewable Camera"
    }
}
