//! Blend-graph authoring — an alternative clip source that lives on the
//! node canvas instead of the keyframe timeline.
//!
//! A blend graph clip is a `Clip` entity that also carries an
//! [`AnimationBlendGraph`] marker plus a `jackdaw_node_graph::NodeGraph` +
//! `GraphCanvasView` so the canvas treats it as a graph root. Its
//! child entities are [`GraphNode`]s / [`Connection`]s (from the node
//! graph crate) rather than [`AnimationTrack`] / keyframe children.
//!
//! ## Node types
//!
//! Four node types are registered with the shared
//! [`NodeTypeRegistry`]:
//!
//! | Id               | Inputs              | Outputs | Body          | Compiles to                  |
//! |------------------|---------------------|---------|---------------|------------------------------|
//! | `anim.clip_ref`  | —                   | `pose`  | [`ClipNodeRef`] | `AnimationGraph::add_clip`   |
//! | `anim.blend`     | `a`, `b`, `weight`  | `pose`  | [`BlendNode`]   | `AnimationGraph::add_blend`  |
//! | `anim.additive`  | `base`, `add`       | `pose`  | [`AdditiveBlendNode`] | `AnimationGraph::add_additive_blend` |
//! | `anim.output`    | `pose`              | —       | [`OutputNode`]  | Graph root                   |
//!
//! Currently the compile step only supports the **single-clip
//! passthrough** case: one `anim.clip_ref` connected to one
//! `anim.output`, whose compiled clip is simply a clone of the
//! referenced clip's [`CompiledClip`]. More complex topologies warn
//! and skip during compile and come in a later phase.
//!
//! [`AnimationTrack`]: crate::clip::AnimationTrack
//! [`Clip`]: crate::clip::Clip
//! [`CompiledClip`]: crate::compile::CompiledClip
//! [`Connection`]: jackdaw_node_graph::Connection
//! [`GraphNode`]: jackdaw_node_graph::GraphNode
//! [`NodeTypeRegistry`]: jackdaw_node_graph::NodeTypeRegistry

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Marker on a [`Clip`](crate::clip::Clip) entity whose source is a
/// node-canvas blend tree. The clip entity also carries
/// `jackdaw_node_graph::NodeGraph` + `GraphCanvasView` so the canvas
/// can use it as a graph root.
#[derive(Component, Reflect, Serialize, Deserialize, Debug, Clone, Default)]
#[reflect(Component, Serialize, Deserialize)]
pub struct AnimationBlendGraph;

/// Body component for an `anim.clip_ref` node. Points at another
/// [`Clip`](crate::clip::Clip) entity in the scene whose
/// [`CompiledClip`](crate::compile::CompiledClip) handle should be
/// fed into this graph.
#[derive(Component, Reflect, Serialize, Deserialize, Debug, Clone)]
#[reflect(Component, Serialize, Deserialize)]
pub struct ClipNodeRef {
    pub clip_entity: Entity,
}

impl Default for ClipNodeRef {
    fn default() -> Self {
        Self {
            clip_entity: Entity::PLACEHOLDER,
        }
    }
}

/// Body component for an `anim.blend` node. Linear blend between
/// `a` and `b`; the `weight` terminal is a compile-time constant if
/// not connected, otherwise driven by the incoming scalar curve.
#[derive(Component, Reflect, Serialize, Deserialize, Debug, Clone, Copy)]
#[reflect(Component, Serialize, Deserialize)]
pub struct BlendNode {
    pub weight: f32,
}

impl Default for BlendNode {
    fn default() -> Self {
        Self { weight: 0.5 }
    }
}

/// Body component for an `anim.additive` node. Adds `add` on top of
/// `base` with intensity `weight`.
#[derive(Component, Reflect, Serialize, Deserialize, Debug, Clone, Copy)]
#[reflect(Component, Serialize, Deserialize)]
pub struct AdditiveBlendNode {
    pub weight: f32,
}

impl Default for AdditiveBlendNode {
    fn default() -> Self {
        Self { weight: 1.0 }
    }
}

/// Body component for an `anim.output` node. Exactly one per graph —
/// compile walks backwards from this entity to build the
/// `AnimationGraph`.
#[derive(Component, Reflect, Serialize, Deserialize, Debug, Clone, Default)]
#[reflect(Component, Serialize, Deserialize)]
pub struct OutputNode;

/// Startup system: register the four animation node types with the
/// shared [`NodeTypeRegistry`](jackdaw_node_graph::NodeTypeRegistry)
/// so the canvas knows how to render them and the right-click "Add
/// Node" menu knows how to spawn them.
///
/// Listed `body_components` are reflection type paths — when the
/// canvas spawns a new node via `AddGraphNodeCmd`, it also spawns
/// these components on the data entity, and the existing inspector
/// reflect-field UI picks them up for inline parameter editing.
pub fn register_animation_node_types(
    mut registry: ResMut<jackdaw_node_graph::NodeTypeRegistry>,
) {
    use jackdaw_node_graph::{NodeTypeDescriptor, TerminalDescriptor};

    const POSE: &str = "anim.pose";
    const SCALAR: &str = "anim.scalar";
    let pose_color = Color::srgb(0.95, 0.70, 0.30);
    let scalar_color = Color::srgb(0.55, 0.80, 0.95);
    let category = "Animation".to_string();

    let pose_out = || TerminalDescriptor {
        label: "pose".into(),
        data_type: POSE.into(),
        color: pose_color,
    };
    let pose_in = |label: &str| TerminalDescriptor {
        label: label.into(),
        data_type: POSE.into(),
        color: pose_color,
    };
    let scalar_in = |label: &str| TerminalDescriptor {
        label: label.into(),
        data_type: SCALAR.into(),
        color: scalar_color,
    };

    registry.register(NodeTypeDescriptor {
        id: "anim.clip_ref".into(),
        display_name: "Clip Reference".into(),
        category: category.clone(),
        accent_color: Color::srgb(0.38, 0.72, 1.0),
        inputs: vec![],
        outputs: vec![pose_out()],
        body_components: vec!["jackdaw_animation::blend_graph::ClipNodeRef".into()],
    });

    registry.register(NodeTypeDescriptor {
        id: "anim.blend".into(),
        display_name: "Blend".into(),
        category: category.clone(),
        accent_color: Color::srgb(0.55, 0.80, 0.95),
        inputs: vec![pose_in("a"), pose_in("b"), scalar_in("weight")],
        outputs: vec![pose_out()],
        body_components: vec!["jackdaw_animation::blend_graph::BlendNode".into()],
    });

    registry.register(NodeTypeDescriptor {
        id: "anim.additive".into(),
        display_name: "Additive Blend".into(),
        category: category.clone(),
        accent_color: Color::srgb(0.75, 0.60, 0.95),
        inputs: vec![pose_in("base"), pose_in("add"), scalar_in("weight")],
        outputs: vec![pose_out()],
        body_components: vec!["jackdaw_animation::blend_graph::AdditiveBlendNode".into()],
    });

    registry.register(NodeTypeDescriptor {
        id: "anim.output".into(),
        display_name: "Output".into(),
        category,
        accent_color: Color::srgb(0.95, 0.50, 0.40),
        inputs: vec![pose_in("pose")],
        outputs: vec![],
        body_components: vec!["jackdaw_animation::blend_graph::OutputNode".into()],
    });
}
