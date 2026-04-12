//! Transport + target adoption. Watches the selected clip and wires up
//! the target entity with the runtime components Bevy needs —
//! [`AnimationPlayer`], [`AnimationGraphHandle`], [`AnimationTargetId`],
//! [`AnimatedBy`] — so Bevy samples the curve automatically. Play,
//! pause, seek, and stop then drive through Bevy's own API.
//!
//! None of the runtime components are persisted to the AST. They're
//! installed when a clip is bound to its target and stripped when the
//! selection changes; the scene serializer's skip prefix for
//! `bevy_animation::` is a defense-in-depth against any future registry
//! changes. The only authored data is the clip + tracks + keyframes
//! from `clip.rs`.
//!
//! [`AnimationPlayer`]: bevy::animation::AnimationPlayer
//! [`AnimationGraphHandle`]: bevy::animation::graph::AnimationGraphHandle
//! [`AnimationTargetId`]: bevy::animation::AnimationTargetId
//! [`AnimatedBy`]: bevy::animation::AnimatedBy

use bevy::animation::{AnimatedBy, AnimationPlayer, AnimationTargetId, graph::AnimationGraphHandle};
use bevy::prelude::*;

use crate::blend_graph::{AnimationBlendGraph, ClipNodeRef, OutputNode};
use crate::clip::{Clip, GltfClipRef, SelectedClip};
use crate::compile::{CompiledClip, clip_display_duration};

/// Distinguishes whether [`auto_bind_player`] installed the full
/// runtime component stack (authored case) or just an
/// [`AnimationGraphHandle`] on top of Bevy's glTF-loader-installed
/// [`AnimationPlayer`] (glTF case). Strip reads this to remove only
/// what it installed.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindMode {
    #[default]
    Authored,
    Gltf,
}

/// The (clip, target) pair whose runtime `AnimationPlayer` is
/// currently installed on the target entity.
///
/// Named after Bevy's [`ActiveAnimation`]: an "active animation target"
/// is the entity that's being driven by the editor right now. The
/// [`auto_bind_player`] system consults this resource to decide
/// whether to strip stale runtime components before installing new
/// ones. A resource rather than a component because there's only ever
/// one active target at a time in the Phase 5A single-entity model.
///
/// `target` is the entity currently hosting the runtime components
/// Jackdaw installed — for authored clips that's the clip's parent,
/// and for glTF clips that's the descendant with Bevy's pre-installed
/// [`AnimationPlayer`]. Strip removes from wherever is stored here.
///
/// [`ActiveAnimation`]: bevy::animation::ActiveAnimation
/// [`auto_bind_player`]: crate::auto_bind_player
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct ActiveClipBinding {
    pub clip: Option<Entity>,
    pub target: Option<Entity>,
    pub mode: BindMode,
}

/// Whether the user is currently driving the selected clip. The target
/// entity's runtime animation components (`AnimationPlayer`, etc.) are
/// only installed while engagement is `Active`. In `Idle` the target
/// is free to edit like any other scene entity — gizmo drag, inspector
/// fields, manual Transform edits all work normally.
///
/// Transitions:
/// - `Idle → Active`: scrubber drag start, or Play button pressed
/// - `Active → Idle`: scrubber drag end, or Pause/Stop pressed, or
///   selection changes (handled implicitly by re-binding)
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimelineEngagement {
    #[default]
    Idle,
    Active,
}

/// Shared timeline state for the editor's transport bar. The widget writes
/// into this resource; the transport systems read it and drive Bevy.
///
/// Field naming matches Bevy's [`ActiveAnimation`] where there's overlap:
/// `seek_time` is the same concept as [`ActiveAnimation::seek_time`].
/// Moving between editor code and Bevy code is a literal copy.
///
/// [`ActiveAnimation`]: bevy::animation::ActiveAnimation
/// [`ActiveAnimation::seek_time`]: bevy::animation::ActiveAnimation::seek_time
#[derive(Resource, Debug, Clone, Copy)]
pub struct TimelineCursor {
    /// Playhead time in seconds from the clip start. Mirrors
    /// [`ActiveAnimation::seek_time`] while a clip is bound.
    ///
    /// [`ActiveAnimation::seek_time`]: bevy::animation::ActiveAnimation::seek_time
    pub seek_time: f32,
    /// True while the transport is actively playing. Set by `AnimationPlay`,
    /// cleared by `AnimationPause`/`AnimationStop`. This is the inverse
    /// of Bevy's [`ActiveAnimation::is_paused`] — see [`Self::is_paused`]
    /// for the Bevy-aligned accessor.
    ///
    /// [`ActiveAnimation::is_paused`]: bevy::animation::ActiveAnimation::is_paused
    pub is_playing: bool,
}

impl Default for TimelineCursor {
    fn default() -> Self {
        Self {
            seek_time: 0.0,
            is_playing: false,
        }
    }
}

impl TimelineCursor {
    /// Matches Bevy's [`ActiveAnimation::is_paused`]: returns `true`
    /// when transport is currently stopped / paused. Editor code can
    /// use either `!cursor.is_playing` or `cursor.is_paused()`
    /// depending on which reads more naturally at the call site.
    ///
    /// [`ActiveAnimation::is_paused`]: bevy::animation::ActiveAnimation::is_paused
    #[inline]
    pub fn is_paused(&self) -> bool {
        !self.is_playing
    }
}

/// User pressed Play on the timeline. Ensures runtime components are
/// present on the target and starts playback from the current cursor.
#[derive(Message, Debug, Clone, Copy)]
pub struct AnimationPlay;

/// User pressed Pause. Leaves the active animation in place so Resume is
/// cheap.
#[derive(Message, Debug, Clone, Copy)]
pub struct AnimationPause;

/// User pressed Stop. Clears active animations and rewinds the cursor.
#[derive(Message, Debug, Clone, Copy)]
pub struct AnimationStop;

/// User dragged the playhead to a specific time. Updates the cursor and,
/// if an active animation exists, seeks it there.
#[derive(Message, Debug, Clone, Copy)]
pub struct AnimationSeek(pub f32);

/// Install or strip the runtime animation components on the clip's
/// target entity based on [`TimelineEngagement`]. Only installs while
/// engagement is `Active` (scrubbing or playing). In `Idle` the target
/// is stripped, leaving its Transform freely editable via gizmos or
/// the inspector — otherwise Bevy's `animate_targets` would clobber
/// every manual edit with the sampled curve value.
///
/// Re-binds eagerly when either the selection or the engagement
/// changes, so transitioning Idle → Active → Idle within a few frames
/// (the scrub-drag case) still works.
///
/// ## glTF case
///
/// When the selected clip has a [`GltfClipRef`], Bevy's glTF loader
/// has already installed an `AnimationPlayer` (and matching
/// `AnimationTarget` components on bone descendants) inside the
/// scene. We find the first descendant of the clip's parent that has
/// that player and install only our [`AnimationGraphHandle`] on it —
/// leaving Bevy's player, targets, and `AnimatedBy` pointers
/// untouched. Strip removes only the graph handle.
#[allow(clippy::too_many_arguments)]
pub fn auto_bind_player(
    selected: Res<SelectedClip>,
    engagement: Res<TimelineEngagement>,
    mut bound: ResMut<ActiveClipBinding>,
    mut cursor: ResMut<TimelineCursor>,
    compiled: Query<&CompiledClip>,
    gltf_refs: Query<&GltfClipRef>,
    blend_graphs: Query<(), With<AnimationBlendGraph>>,
    clip_refs: Query<&ClipNodeRef>,
    outputs: Query<(), With<OutputNode>>,
    graph_connections: Query<&jackdaw_node_graph::Connection>,
    parents: Query<&ChildOf>,
    names: Query<&Name>,
    children_q: Query<&Children>,
    anim_players: Query<(), With<AnimationPlayer>>,
    mut commands: Commands,
) {
    let want_bound = *engagement == TimelineEngagement::Active && selected.0.is_some();
    let currently_bound = bound.target.is_some() && bound.clip == selected.0;

    if want_bound == currently_bound && !want_bound {
        // Idle and already stripped — nothing to do.
        return;
    }
    if want_bound && currently_bound {
        // Already bound to the right clip. Nothing to do.
        return;
    }

    // Strip the previous bind (covers both "deactivating" and
    // "switching clips while active") so we can't leave stale
    // components behind. For authored binds we remove the whole
    // runtime stack; for glTF binds we leave Bevy's preinstalled
    // player + targets in place and only remove the graph handle.
    if let Some(old_target) = bound.target.take() {
        let old_mode = bound.mode;
        commands.queue(move |world: &mut World| {
            if let Ok(mut ent) = world.get_entity_mut(old_target) {
                match old_mode {
                    BindMode::Authored => {
                        ent.remove::<AnimationPlayer>();
                        ent.remove::<AnimationGraphHandle>();
                        ent.remove::<AnimationTargetId>();
                        ent.remove::<AnimatedBy>();
                    }
                    BindMode::Gltf => {
                        ent.remove::<AnimationGraphHandle>();
                    }
                }
            }
        });
    }
    bound.clip = None;

    if !want_bound {
        cursor.is_playing = false;
        return;
    }

    // From here on: engagement is Active and we need to install on
    // the clip's target.
    let Some(clip_entity) = selected.0 else {
        return;
    };

    // Blend graph passthrough: resolve the selected clip through its
    // single `ClipRef → Output` edge so runtime binding happens
    // against the *referenced* clip's target instead of the blend
    // graph's own parent. The blend graph's `CompiledClip` already
    // points at the same handles as the referenced clip, so all we
    // need to do is redirect the target resolution.
    let effective_clip = if blend_graphs.contains(clip_entity) {
        match resolve_blend_graph_passthrough_source(
            clip_entity,
            &children_q,
            &graph_connections,
            &clip_refs,
            &outputs,
        ) {
            Some(source) => source,
            None => return,
        }
    } else {
        clip_entity
    };

    // Clip not compiled yet (compile runs in PostUpdate; we're in
    // Update). Retry next frame.
    let Ok(compiled) = compiled.get(clip_entity) else {
        return;
    };
    let Ok(clip_parent) = parents.get(effective_clip) else {
        return;
    };
    let parent_entity = clip_parent.parent();

    let graph = compiled.graph.clone();
    let root_node = compiled.root_node;
    let seek_time = cursor.seek_time;
    let start_playing = cursor.is_playing;

    if gltf_refs.contains(effective_clip) {
        // glTF path: find the first descendant of `parent_entity`
        // (the GltfSource root) that has an `AnimationPlayer`. If the
        // scene hasn't finished spawning yet, none will exist and we
        // retry next frame.
        let Some(host) = find_animation_player_descendant(parent_entity, &children_q, &anim_players)
        else {
            return;
        };
        commands.queue(move |world: &mut World| {
            world
                .entity_mut(host)
                .insert(AnimationGraphHandle(graph));
            if let Some(mut player) = world.get_mut::<AnimationPlayer>(host) {
                if player.animation_mut(root_node).is_none() {
                    player.play(root_node);
                }
                if let Some(active) = player.animation_mut(root_node) {
                    active.seek_to(seek_time);
                    if start_playing {
                        active.resume();
                    } else {
                        active.pause();
                    }
                }
            }
        });
        bound.clip = Some(clip_entity);
        bound.target = Some(host);
        bound.mode = BindMode::Gltf;
        return;
    }

    // Authored path: install the full runtime stack on the clip's
    // parent.
    let Ok(target_name) = names.get(parent_entity) else {
        return;
    };
    let target_id = AnimationTargetId::from_name(target_name);

    commands.queue(move |world: &mut World| {
        // Build the player with an active animation seeded at the
        // current cursor. Bevy evaluates paused animations at their
        // `seek_time` without advancing time, so the scrub flow can
        // leave `paused = true` and still preview correctly. Play
        // inserts an already-running animation.
        let mut player = AnimationPlayer::default();
        {
            let active = player.play(root_node);
            active.seek_to(seek_time);
            if !start_playing {
                active.pause();
            }
        }
        world.entity_mut(parent_entity).insert((
            player,
            AnimationGraphHandle(graph),
            target_id,
            AnimatedBy(parent_entity),
        ));
    });

    bound.clip = Some(clip_entity);
    bound.target = Some(parent_entity);
    bound.mode = BindMode::Authored;
}

/// Walk a blend graph's single `ClipRef → Output` connection to
/// find the `Entity` of the clip being passed through. Matches the
/// MVP scope of [`compile_blend_graphs`] — only "one clip ref, one
/// output, one connection between them" is recognized. Returns
/// `None` if the graph is empty, incomplete, or uses a topology the
/// compile step can't handle yet.
fn resolve_blend_graph_passthrough_source(
    blend_graph_entity: Entity,
    children_q: &Query<&Children>,
    connections: &Query<&jackdaw_node_graph::Connection>,
    clip_refs: &Query<&ClipNodeRef>,
    outputs: &Query<(), With<OutputNode>>,
) -> Option<Entity> {
    let graph_children = children_q.get(blend_graph_entity).ok()?;
    let output_node = graph_children.iter().find(|c| outputs.contains(*c))?;
    let incoming: Vec<&jackdaw_node_graph::Connection> = graph_children
        .iter()
        .filter_map(|c| connections.get(c).ok())
        .filter(|c| c.target_node == output_node)
        .collect();
    if incoming.len() != 1 {
        return None;
    }
    let source_node = incoming[0].source_node;
    let clip_ref = clip_refs.get(source_node).ok()?;
    if clip_ref.clip_entity == Entity::PLACEHOLDER {
        return None;
    }
    Some(clip_ref.clip_entity)
}

/// Breadth-first search the descendants of `root` for the first
/// entity carrying an `AnimationPlayer`. Used by the glTF bind path
/// to locate Bevy's loader-installed player inside a freshly-spawned
/// glTF scene. Returns `None` if the scene hasn't spawned yet or the
/// glTF has no animation roots.
fn find_animation_player_descendant(
    root: Entity,
    children_q: &Query<&Children>,
    anim_players: &Query<(), With<AnimationPlayer>>,
) -> Option<Entity> {
    let mut queue: std::collections::VecDeque<Entity> = std::collections::VecDeque::new();
    queue.push_back(root);
    while let Some(entity) = queue.pop_front() {
        if anim_players.contains(entity) {
            return Some(entity);
        }
        if let Ok(children) = children_q.get(entity) {
            for child in children.iter() {
                queue.push_back(child);
            }
        }
    }
    None
}

pub fn handle_play(
    mut events: MessageReader<AnimationPlay>,
    mut cursor: ResMut<TimelineCursor>,
    mut engagement: ResMut<TimelineEngagement>,
    bound: Res<ActiveClipBinding>,
    clips: Query<&CompiledClip>,
    mut players: Query<&mut AnimationPlayer>,
) {
    if events.read().count() == 0 {
        return;
    }
    cursor.is_playing = true;
    *engagement = TimelineEngagement::Active;

    // If we happen to already be bound (e.g. coming out of a pause),
    // resume the player in place. If we're Idle, auto_bind_player
    // will install a freshly-unpaused player on the next frame based
    // on `cursor.is_playing == true`.
    let (Some(clip_entity), Some(target_entity)) = (bound.clip, bound.target) else {
        return;
    };
    let Ok(compiled) = clips.get(clip_entity) else {
        return;
    };
    if let Ok(mut player) = players.get_mut(target_entity) {
        if player.animation_mut(compiled.root_node).is_none() {
            player.play(compiled.root_node);
        }
        if let Some(active) = player.animation_mut(compiled.root_node) {
            active.seek_to(cursor.seek_time);
            active.resume();
        }
    }
}

pub fn handle_pause(
    mut events: MessageReader<AnimationPause>,
    mut cursor: ResMut<TimelineCursor>,
    bound: Res<ActiveClipBinding>,
    clips: Query<&CompiledClip>,
    mut players: Query<&mut AnimationPlayer>,
) {
    if events.read().count() == 0 {
        return;
    }
    cursor.is_playing = false;
    // Deliberately leave engagement alone: pausing keeps the target
    // bound so the user can see the frozen frame. Stop is the action
    // that releases the target.
    let (Some(clip_entity), Some(target_entity)) = (bound.clip, bound.target) else {
        return;
    };
    let Ok(compiled) = clips.get(clip_entity) else {
        return;
    };
    if let Ok(mut player) = players.get_mut(target_entity) {
        if let Some(active) = player.animation_mut(compiled.root_node) {
            active.pause();
        }
    }
}

pub fn handle_stop(
    mut events: MessageReader<AnimationStop>,
    mut cursor: ResMut<TimelineCursor>,
    mut engagement: ResMut<TimelineEngagement>,
) {
    if events.read().count() == 0 {
        return;
    }
    cursor.seek_time = 0.0;
    cursor.is_playing = false;
    // Drop engagement to Idle — auto_bind_player will strip the
    // runtime components on the next frame, releasing the target so
    // the user can edit its Transform via gizmos again.
    *engagement = TimelineEngagement::Idle;
}

pub fn handle_seek(
    mut events: MessageReader<AnimationSeek>,
    mut cursor: ResMut<TimelineCursor>,
    bound: Res<ActiveClipBinding>,
    clips: Query<&CompiledClip>,
    mut players: Query<&mut AnimationPlayer>,
) {
    let Some(AnimationSeek(time)) = events.read().last().copied() else {
        return;
    };
    cursor.seek_time = time;
    let (Some(clip_entity), Some(target_entity)) = (bound.clip, bound.target) else {
        return;
    };
    let Ok(compiled) = clips.get(clip_entity) else {
        return;
    };
    if let Ok(mut player) = players.get_mut(target_entity) {
        if let Some(active) = player.animation_mut(compiled.root_node) {
            active.seek_to(time);
        }
    }
}

/// While playing, mirror the Bevy animation's seek time back into the
/// cursor so the timeline widget draws an accurate playhead. The clip
/// duration is derived from the keyframe data at every call, not
/// stored as authored data.
pub fn sync_cursor_from_player(
    mut cursor: ResMut<TimelineCursor>,
    bound: Res<ActiveClipBinding>,
    compiled: Query<&CompiledClip>,
    clips: Query<(&Clip, Option<&Children>)>,
    players: Query<&AnimationPlayer>,
) {
    if !cursor.is_playing {
        return;
    }
    let (Some(clip_entity), Some(target_entity)) = (bound.clip, bound.target) else {
        return;
    };
    let Ok(compiled) = compiled.get(clip_entity) else {
        return;
    };
    let duration = clip_display_duration(clip_entity, &clips);
    if let Ok(player) = players.get(target_entity) {
        if let Some(active) = player.animation(compiled.root_node) {
            cursor.seek_time = active.seek_time().clamp(0.0, duration);
        }
    }
}

