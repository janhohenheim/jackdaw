use std::f32::consts::FRAC_PI_2;

use avian3d::prelude::*;
use bevy::prelude::*;

use crate::colors;
use crate::selection::Selected;

#[derive(Resource)]
pub struct PhysicsOverlayConfig {
    pub show_colliders: bool,
    pub show_hierarchy_arrows: bool,
}

impl Default for PhysicsOverlayConfig {
    fn default() -> Self {
        Self {
            show_colliders: true,
            show_hierarchy_arrows: false,
        }
    }
}

#[derive(Default, Reflect, GizmoConfigGroup)]
pub struct ColliderGizmoGroup;

pub struct PhysicsOverlaysPlugin;

impl Plugin for PhysicsOverlaysPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PhysicsOverlayConfig>()
            .init_gizmo_group::<ColliderGizmoGroup>()
            .add_systems(
                PostUpdate,
                (
                    draw_collider_gizmos,
                    draw_hierarchy_arrows,
                )
                    .after(bevy::transform::TransformSystems::Propagate)
                    .run_if(in_state(crate::AppState::Editor)),
            );

        // Configure the gizmo group
        let mut store = app.world_mut().resource_mut::<GizmoConfigStore>();
        let (config, _) = store.config_mut::<ColliderGizmoGroup>();
        config.depth_bias = -0.5;
        config.line.width = 1.5;
    }
}

fn draw_collider_gizmos(
    mut gizmos: Gizmos<ColliderGizmoGroup>,
    config: Res<PhysicsOverlayConfig>,
    colliders: Query<(
        Entity,
        &ColliderConstructor,
        &GlobalTransform,
        &InheritedVisibility,
        Option<&Sensor>,
    )>,
    selected_bodies: Query<Entity, (With<RigidBody>, With<Selected>)>,
    children_query: Query<&Children>,
    collider_check: Query<(), With<ColliderConstructor>>,
) {
    if !config.show_colliders {
        return;
    }

    // Collect all collider entities that belong to a selected rigid body
    let mut highlighted_colliders = bevy::ecs::entity::EntityHashSet::default();
    for body_entity in &selected_bodies {
        collect_descendant_colliders(
            body_entity,
            &children_query,
            &collider_check,
            &mut highlighted_colliders,
        );
        // The body itself might have a collider
        if collider_check.contains(body_entity) {
            highlighted_colliders.insert(body_entity);
        }
    }

    for (entity, constructor, tf, vis, sensor) in &colliders {
        if !vis.get() {
            continue;
        }

        let is_highlighted = highlighted_colliders.contains(&entity);
        let color = match (sensor.is_some(), is_highlighted) {
            (false, false) => colors::COLLIDER_WIREFRAME,
            (false, true) => colors::COLLIDER_SELECTED,
            (true, false) => colors::SENSOR_WIREFRAME,
            (true, true) => colors::SENSOR_SELECTED,
        };

        let transform = tf.compute_transform();
        let pos = transform.translation;
        let rot = transform.rotation;

        draw_collider_shape(&mut gizmos, constructor, pos, rot, color);
    }
}

fn draw_collider_shape(
    gizmos: &mut Gizmos<ColliderGizmoGroup>,
    constructor: &ColliderConstructor,
    pos: Vec3,
    rot: Quat,
    color: Color,
) {
    match constructor {
        ColliderConstructor::Sphere { radius } => {
            let r = if *radius > 0.0 { *radius } else { 0.5 };
            gizmos.circle(Isometry3d::new(pos, rot * Quat::from_rotation_x(FRAC_PI_2)), r, color);
            gizmos.circle(Isometry3d::new(pos, rot), r, color);
            gizmos.circle(Isometry3d::new(pos, rot * Quat::from_rotation_y(FRAC_PI_2)), r, color);
        }
        ColliderConstructor::Cuboid {
            x_length,
            y_length,
            z_length,
        } => {
            let half = Vec3::new(*x_length, *y_length, *z_length) * 0.5;
            draw_box_wireframe(gizmos, pos, rot, half, color);
        }
        ColliderConstructor::RoundCuboid {
            x_length,
            y_length,
            z_length,
            ..
        } => {
            let half = Vec3::new(*x_length, *y_length, *z_length) * 0.5;
            draw_box_wireframe(gizmos, pos, rot, half, color);
        }
        ColliderConstructor::Cylinder { radius, height } => {
            let r = *radius;
            let half_h = *height * 0.5;
            let up = rot * Vec3::Y;
            // Top and bottom circles
            gizmos.circle(Isometry3d::new(pos + up * half_h, rot), r, color);
            gizmos.circle(Isometry3d::new(pos - up * half_h, rot), r, color);
            // Four vertical lines
            let right = rot * Vec3::X;
            let fwd = rot * Vec3::Z;
            for dir in [right, -right, fwd, -fwd] {
                gizmos.line(pos + dir * r + up * half_h, pos + dir * r - up * half_h, color);
            }
        }
        ColliderConstructor::Cone { radius, height } => {
            let r = *radius;
            let half_h = *height * 0.5;
            let up = rot * Vec3::Y;
            let apex = pos + up * half_h;
            // Base circle
            gizmos.circle(Isometry3d::new(pos - up * half_h, rot), r, color);
            // Four lines from base to apex
            let right = rot * Vec3::X;
            let fwd = rot * Vec3::Z;
            for dir in [right, -right, fwd, -fwd] {
                gizmos.line(pos + dir * r - up * half_h, apex, color);
            }
        }
        ColliderConstructor::Capsule { radius, height } => {
            let r = *radius;
            let half_h = *height * 0.5;
            let up = rot * Vec3::Y;
            let right = rot * Vec3::X;
            let fwd = rot * Vec3::Z;
            // Top and bottom circles
            gizmos.circle(Isometry3d::new(pos + up * half_h, rot), r, color);
            gizmos.circle(Isometry3d::new(pos - up * half_h, rot), r, color);
            // Hemisphere arcs (front/back and left/right)
            gizmos.arc_3d(std::f32::consts::PI, r, Isometry3d::new(pos + up * half_h, rot * Quat::from_rotation_z(FRAC_PI_2)), color);
            gizmos.arc_3d(std::f32::consts::PI, r, Isometry3d::new(pos - up * half_h, rot * Quat::from_rotation_z(-FRAC_PI_2)), color);
            gizmos.arc_3d(std::f32::consts::PI, r, Isometry3d::new(pos + up * half_h, rot * Quat::from_rotation_z(FRAC_PI_2) * Quat::from_rotation_y(FRAC_PI_2)), color);
            gizmos.arc_3d(std::f32::consts::PI, r, Isometry3d::new(pos - up * half_h, rot * Quat::from_rotation_z(-FRAC_PI_2) * Quat::from_rotation_y(FRAC_PI_2)), color);
            // Four vertical lines
            for dir in [right, -right, fwd, -fwd] {
                gizmos.line(pos + dir * r + up * half_h, pos + dir * r - up * half_h, color);
            }
        }
        // For complex shapes (trimesh, convex hull, etc.), just draw a small marker
        _ => {
            gizmos.sphere(Isometry3d::new(pos, rot), 0.05, color);
        }
    }
}

fn draw_box_wireframe(
    gizmos: &mut Gizmos<ColliderGizmoGroup>,
    pos: Vec3,
    rot: Quat,
    half: Vec3,
    color: Color,
) {
    let corners = [
        Vec3::new(-half.x, -half.y, -half.z),
        Vec3::new(half.x, -half.y, -half.z),
        Vec3::new(half.x, half.y, -half.z),
        Vec3::new(-half.x, half.y, -half.z),
        Vec3::new(-half.x, -half.y, half.z),
        Vec3::new(half.x, -half.y, half.z),
        Vec3::new(half.x, half.y, half.z),
        Vec3::new(-half.x, half.y, half.z),
    ];
    let edges = [
        (0, 1), (1, 2), (2, 3), (3, 0), // bottom face
        (4, 5), (5, 6), (6, 7), (7, 4), // top face
        (0, 4), (1, 5), (2, 6), (3, 7), // vertical edges
    ];
    for (a, b) in edges {
        gizmos.line(
            pos + rot * corners[a],
            pos + rot * corners[b],
            color,
        );
    }
}

fn draw_hierarchy_arrows(
    mut gizmos: Gizmos<ColliderGizmoGroup>,
    config: Res<PhysicsOverlayConfig>,
    selected_bodies: Query<(Entity, &GlobalTransform), (With<RigidBody>, With<Selected>)>,
    children_query: Query<&Children>,
    collider_transforms: Query<&GlobalTransform, With<ColliderConstructor>>,
    collider_check: Query<(), With<ColliderConstructor>>,
) {
    if !config.show_hierarchy_arrows {
        return;
    }

    for (body_entity, body_tf) in &selected_bodies {
        let body_pos = body_tf.translation();
        let mut descendant_colliders = bevy::ecs::entity::EntityHashSet::default();
        collect_descendant_colliders(
            body_entity,
            &children_query,
            &collider_check,
            &mut descendant_colliders,
        );

        for collider_entity in &descendant_colliders {
            // Don't draw arrow to self
            if *collider_entity == body_entity {
                continue;
            }
            if let Ok(collider_tf) = collider_transforms.get(*collider_entity) {
                gizmos.arrow(body_pos, collider_tf.translation(), colors::COLLIDER_HIERARCHY_ARROW);
            }
        }
    }
}

fn collect_descendant_colliders(
    entity: Entity,
    children_query: &Query<&Children>,
    collider_check: &Query<(), With<ColliderConstructor>>,
    out: &mut bevy::ecs::entity::EntityHashSet,
) {
    if let Ok(children) = children_query.get(entity) {
        for child in children.iter() {
            if collider_check.contains(child) {
                out.insert(child);
            }
            collect_descendant_colliders(child, children_query, collider_check, out);
        }
    }
}
