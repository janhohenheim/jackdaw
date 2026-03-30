use std::f32::consts::FRAC_PI_2;

use avian3d::prelude::*;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use parry3d::shape::TypedShape;

use crate::colors;
use crate::selection::Selected;

/// Cache of computed collider shapes, keyed by mesh asset ID.
/// For brush entities (no mesh asset), keyed by entity ID converted to a
/// synthetic AssetId. Collider is Arc-backed so clones are cheap.
#[derive(Resource, Default)]
pub struct ColliderPreviewCache {
    /// Mesh asset → (constructor, collider) pairs (same pattern as Avian's ColliderCache).
    by_mesh: HashMap<AssetId<Mesh>, Vec<(ColliderConstructor, Collider)>>,
    /// Entity → collider for brush entities that build meshes from BrushMeshCache.
    by_entity: HashMap<Entity, (ColliderConstructor, Collider)>,
}

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
            .init_resource::<ColliderPreviewCache>()
            .init_gizmo_group::<ColliderGizmoGroup>()
            .add_systems(
                PostUpdate,
                (
                    draw_collider_gizmos,
                    draw_hierarchy_arrows,
                )
                    .after(bevy::transform::TransformSystems::Propagate)
                    .run_if(in_state(crate::AppState::Editor)),
            )
            .add_systems(
                PostUpdate,
                evict_collider_cache
                    .run_if(in_state(crate::AppState::Editor)),
            );

        let mut store = app.world_mut().resource_mut::<GizmoConfigStore>();
        let (config, _) = store.config_mut::<ColliderGizmoGroup>();
        config.depth_bias = -0.5;
        config.line.width = 1.5;
    }
}

/// Convert a parry3d Vector (glam 0.30) to bevy Vec3 (glam 0.32).
fn parry_vec(v: parry3d::math::Vector) -> Vec3 {
    Vec3::new(v.x, v.y, v.z)
}

fn parry_point(p: &parry3d::math::Vector) -> Vec3 {
    Vec3::new(p.x, p.y, p.z)
}

fn draw_collider_gizmos(
    mut gizmos: Gizmos<ColliderGizmoGroup>,
    config: Res<PhysicsOverlayConfig>,
    mut cache: ResMut<ColliderPreviewCache>,
    colliders: Query<(
        Entity,
        &ColliderConstructor,
        &GlobalTransform,
        &InheritedVisibility,
        Option<&Sensor>,
        Option<&Mesh3d>,
        Option<&crate::brush::BrushMeshCache>,
    )>,
    selected_bodies: Query<Entity, (With<RigidBody>, With<Selected>)>,
    children_query: Query<&Children>,
    collider_check: Query<(), With<ColliderConstructor>>,
    meshes: Res<Assets<Mesh>>,
) {
    if !config.show_colliders {
        return;
    }

    // Collect highlighted colliders (belonging to a selected rigid body)
    let mut highlighted_colliders = bevy::ecs::entity::EntityHashSet::default();
    for body_entity in &selected_bodies {
        collect_descendant_colliders(
            body_entity,
            &children_query,
            &collider_check,
            &mut highlighted_colliders,
        );
        if collider_check.contains(body_entity) {
            highlighted_colliders.insert(body_entity);
        }
    }

    for (entity, constructor, tf, vis, sensor, mesh3d, brush_cache) in &colliders {
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

        // Try to get the collider from cache or compute it.
        let collider = get_or_compute_collider(
            &mut cache,
            entity,
            constructor,
            mesh3d,
            brush_cache,
            &meshes,
        );

        if let Some(c) = &collider {
            draw_parry_shape(&mut gizmos, c.shape(), pos, rot, color);
        }
    }
}

/// Get a cached collider or compute and cache a new one.
/// Uses mesh AssetId for Mesh3d entities, entity ID for brush entities.
fn get_or_compute_collider(
    cache: &mut ColliderPreviewCache,
    entity: Entity,
    constructor: &ColliderConstructor,
    mesh3d: Option<&Mesh3d>,
    brush_cache: Option<&crate::brush::BrushMeshCache>,
    meshes: &Assets<Mesh>,
) -> Option<Collider> {
    // For mesh-asset-backed entities, cache by AssetId
    if let Some(mesh3d) = mesh3d {
        let asset_id = mesh3d.0.id();
        if let Some(entries) = cache.by_mesh.get(&asset_id) {
            if let Some((_, collider)) = entries.iter().find(|(c, _)| c == constructor) {
                return Some(collider.clone());
            }
        }
        let mesh = meshes.get(&mesh3d.0)?;
        let collider = Collider::try_from_constructor(constructor.clone(), Some(mesh))?;
        cache.by_mesh
            .entry(asset_id)
            .or_default()
            .push((constructor.clone(), collider.clone()));
        return Some(collider);
    }

    // For brush entities, cache by entity
    if let Some(brush) = brush_cache {
        if let Some((cached_ctor, cached_collider)) = cache.by_entity.get(&entity) {
            if cached_ctor == constructor {
                return Some(cached_collider.clone());
            }
        }
        let mesh = brush_mesh_from_cache(brush)?;
        let collider = Collider::try_from_constructor(constructor.clone(), Some(&mesh))?;
        cache.by_entity.insert(entity, (constructor.clone(), collider.clone()));
        return Some(collider);
    }

    // Primitive shapes (no mesh needed)
    if !constructor.requires_mesh() {
        // Primitives are cheap to construct, but cache by entity anyway
        if let Some((cached_ctor, cached_collider)) = cache.by_entity.get(&entity) {
            if cached_ctor == constructor {
                return Some(cached_collider.clone());
            }
        }
        let collider = Collider::try_from_constructor(constructor.clone(), None)?;
        cache.by_entity.insert(entity, (constructor.clone(), collider.clone()));
        return Some(collider);
    }

    None
}

/// Build a triangulated Mesh from BrushMeshCache.
fn brush_mesh_from_cache(cache: &crate::brush::BrushMeshCache) -> Option<Mesh> {
    if cache.vertices.is_empty() {
        return None;
    }
    let positions: Vec<[f32; 3]> = cache.vertices.iter().map(|v| [v.x, v.y, v.z]).collect();
    let mut indices: Vec<u32> = Vec::new();
    for polygon in &cache.face_polygons {
        if polygon.len() >= 3 {
            for i in 1..polygon.len() - 1 {
                indices.push(polygon[0] as u32);
                indices.push(polygon[i] as u32);
                indices.push(polygon[i + 1] as u32);
            }
        }
    }
    let mut m = Mesh::new(
        bevy::mesh::PrimitiveTopology::TriangleList,
        bevy::asset::RenderAssetUsages::default(),
    );
    m.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    m.insert_indices(bevy::mesh::Indices::U32(indices));
    Some(m)
}

/// Evict cache entries for entities that no longer have ColliderConstructor
/// or mesh assets that were removed.
fn evict_collider_cache(
    mut cache: ResMut<ColliderPreviewCache>,
    colliders: Query<Entity, With<ColliderConstructor>>,
    mut mesh_events: MessageReader<AssetEvent<Mesh>>,
) {
    // Remove entries for despawned/changed entities
    cache.by_entity.retain(|entity, _| colliders.contains(*entity));

    // Remove entries for removed/changed mesh assets
    for event in mesh_events.read() {
        match event {
            AssetEvent::Removed { id } | AssetEvent::Modified { id } => {
                cache.by_mesh.remove(id);
            }
            _ => {}
        }
    }
}

/// Draw a wireframe for any parry shape using TypedShape pattern matching.
fn draw_parry_shape(
    gizmos: &mut Gizmos<ColliderGizmoGroup>,
    shape: &parry3d::shape::SharedShape,
    pos: Vec3,
    rot: Quat,
    color: Color,
) {
    match shape.as_typed_shape() {
        TypedShape::Ball(ball) => {
            let r = if ball.radius > 0.0 { ball.radius } else { 0.5 };
            gizmos.circle(Isometry3d::new(pos, rot * Quat::from_rotation_x(FRAC_PI_2)), r, color);
            gizmos.circle(Isometry3d::new(pos, rot), r, color);
            gizmos.circle(Isometry3d::new(pos, rot * Quat::from_rotation_y(FRAC_PI_2)), r, color);
        }
        TypedShape::Cuboid(cuboid) => {
            let he = cuboid.half_extents;
            let half = Vec3::new(he.x, he.y, he.z);
            let half = if half.length_squared() < 0.0001 { Vec3::splat(0.5) } else { half };
            draw_box_wireframe(gizmos, pos, rot, half, color);
        }
        TypedShape::RoundCuboid(rc) => {
            let he = rc.inner_shape.half_extents;
            let half = Vec3::new(he.x, he.y, he.z);
            let half = if half.length_squared() < 0.0001 { Vec3::splat(0.5) } else { half };
            draw_box_wireframe(gizmos, pos, rot, half, color);
        }
        TypedShape::Cylinder(cyl) => {
            let r = cyl.radius;
            let half_h = cyl.half_height;
            let up = rot * Vec3::Y;
            gizmos.circle(Isometry3d::new(pos + up * half_h, rot), r, color);
            gizmos.circle(Isometry3d::new(pos - up * half_h, rot), r, color);
            let right = rot * Vec3::X;
            let fwd = rot * Vec3::Z;
            for dir in [right, -right, fwd, -fwd] {
                gizmos.line(pos + dir * r + up * half_h, pos + dir * r - up * half_h, color);
            }
        }
        TypedShape::Cone(cone) => {
            let r = cone.radius;
            let half_h = cone.half_height;
            let up = rot * Vec3::Y;
            let apex = pos + up * half_h;
            gizmos.circle(Isometry3d::new(pos - up * half_h, rot), r, color);
            let right = rot * Vec3::X;
            let fwd = rot * Vec3::Z;
            for dir in [right, -right, fwd, -fwd] {
                gizmos.line(pos + dir * r - up * half_h, apex, color);
            }
        }
        TypedShape::Capsule(cap) => {
            let r = cap.radius;
            let a = parry_point(&cap.segment.a);
            let b = parry_point(&cap.segment.b);
            let half_h = (b - a).length() * 0.5;
            let up = rot * Vec3::Y;
            let right = rot * Vec3::X;
            let fwd = rot * Vec3::Z;
            gizmos.circle(Isometry3d::new(pos + up * half_h, rot), r, color);
            gizmos.circle(Isometry3d::new(pos - up * half_h, rot), r, color);
            // Hemisphere arcs
            gizmos.arc_3d(std::f32::consts::PI, r, Isometry3d::new(pos + up * half_h, rot * Quat::from_rotation_z(FRAC_PI_2)), color);
            gizmos.arc_3d(std::f32::consts::PI, r, Isometry3d::new(pos - up * half_h, rot * Quat::from_rotation_z(-FRAC_PI_2)), color);
            gizmos.arc_3d(std::f32::consts::PI, r, Isometry3d::new(pos + up * half_h, rot * Quat::from_rotation_z(FRAC_PI_2) * Quat::from_rotation_y(FRAC_PI_2)), color);
            gizmos.arc_3d(std::f32::consts::PI, r, Isometry3d::new(pos - up * half_h, rot * Quat::from_rotation_z(-FRAC_PI_2) * Quat::from_rotation_y(FRAC_PI_2)), color);
            for dir in [right, -right, fwd, -fwd] {
                gizmos.line(pos + dir * r + up * half_h, pos + dir * r - up * half_h, color);
            }
        }
        TypedShape::TriMesh(trimesh) => {
            let vertices = trimesh.vertices();
            let indices = trimesh.indices();
            for tri in indices {
                let a = pos + rot * parry_point(&vertices[tri[0] as usize]);
                let b = pos + rot * parry_point(&vertices[tri[1] as usize]);
                let c = pos + rot * parry_point(&vertices[tri[2] as usize]);
                gizmos.line(a, b, color);
                gizmos.line(b, c, color);
                gizmos.line(c, a, color);
            }
        }
        TypedShape::ConvexPolyhedron(poly) => {
            let points = poly.points();
            for edge in poly.edges() {
                let a = pos + rot * parry_vec(points[edge.vertices[0] as usize]);
                let b = pos + rot * parry_vec(points[edge.vertices[1] as usize]);
                gizmos.line(a, b, color);
            }
        }
        TypedShape::Compound(compound) => {
            for (iso, sub_shape) in compound.shapes() {
                let sub_pos = pos + rot * Vec3::new(
                    iso.translation.x,
                    iso.translation.y,
                    iso.translation.z,
                );
                // Approximate sub-rotation
                let sub_rot = rot; // TODO: compose with iso rotation
                draw_parry_shape(gizmos, sub_shape, sub_pos, sub_rot, color);
            }
        }
        TypedShape::HalfSpace(_) => {
            // Draw a large plane indicator
            let right = rot * Vec3::X * 5.0;
            let fwd = rot * Vec3::Z * 5.0;
            gizmos.line(pos - right - fwd, pos + right - fwd, color);
            gizmos.line(pos + right - fwd, pos + right + fwd, color);
            gizmos.line(pos + right + fwd, pos - right + fwd, color);
            gizmos.line(pos - right + fwd, pos - right - fwd, color);
            // Normal arrow
            gizmos.arrow(pos, pos + rot * Vec3::Y * 2.0, color);
        }
        TypedShape::Segment(seg) => {
            let a = pos + rot * parry_point(&seg.a);
            let b = pos + rot * parry_point(&seg.b);
            gizmos.line(a, b, color);
        }
        TypedShape::Triangle(tri) => {
            let a = pos + rot * parry_point(&tri.a);
            let b = pos + rot * parry_point(&tri.b);
            let c = pos + rot * parry_point(&tri.c);
            gizmos.line(a, b, color);
            gizmos.line(b, c, color);
            gizmos.line(c, a, color);
        }
        _ => {
            // Unknown shape type — draw a small marker
            gizmos.sphere(Isometry3d::new(pos, rot), 0.1, color);
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
        (0, 1), (1, 2), (2, 3), (3, 0),
        (4, 5), (5, 6), (6, 7), (7, 4),
        (0, 4), (1, 5), (2, 6), (3, 7),
    ];
    for (a, b) in edges {
        gizmos.line(pos + rot * corners[a], pos + rot * corners[b], color);
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
