//! BSN text loader: parse `.bsn` text into a [`SceneBsnAst`] and spawn ECS
//! entities from it.
//!
//! Uses bevy's `TopLevelPatchesParser` + `Lexer` for parsing, then adapts
//! bevy's AST types to jackdaw's AST types.

use std::cell::RefCell;

use bevy::prelude::*;
use bevy::scene2::dynamic_bsn::{
    BsnAst, BsnExpr, BsnField as BevyBsnField, BsnNameStore, BsnPatch as BevyBsnPatch,
    BsnPatches as BevyBsnPatches,
};
use bevy::scene2::dynamic_bsn_grammar::TopLevelPatchesParser;
use bevy::scene2::dynamic_bsn_lexer::Lexer;

use crate::{
    AstDirty, AstNodeRef, BsnField, BsnPatch, BsnPatches, BsnStructData, BsnStructFields,
    BsnTupleStructData, BsnValue, SceneBsnAst,
};

/// Errors that can occur when loading BSN text.
#[derive(Debug)]
pub enum BsnLoadError {
    Parse(String),
    NoAstNode,
}

impl std::fmt::Display for BsnLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BsnLoadError::Parse(msg) => write!(f, "BSN parse error: {msg}"),
            BsnLoadError::NoAstNode => write!(f, "No AST node found"),
        }
    }
}

/// Parse BSN text into a jackdaw [`SceneBsnAst`].
pub fn parse_bsn_text(text: &str) -> Result<SceneBsnAst, BsnLoadError> {
    // Parse using bevy's parser.
    let mut world = World::new();
    world.init_resource::<BsnNameStore>();
    let ast = RefCell::new(BsnAst(world));

    let lexer = Lexer::new(text);
    let patches_id = TopLevelPatchesParser::new()
        .parse(&ast, lexer)
        .map_err(|e| BsnLoadError::Parse(format!("{e:?}")))?;

    let bevy_ast = ast.into_inner();

    // Adapt bevy AST → jackdaw AST.
    let mut jackdaw_ast = SceneBsnAst::default();
    let root = adapt_patches(&bevy_ast, patches_id, &mut jackdaw_ast)?;

    // If the top-level is a single entity with only a Children relation, unwrap
    // to get the real roots (multi-root format).
    let root_patches = jackdaw_ast.get_patches(root);
    let is_children_wrapper = root_patches.is_some_and(|p| {
        p.0.len() == 1
            && jackdaw_ast
                .get_patch(p.0[0])
                .is_some_and(|patch| matches!(patch, BsnPatch::Children(_)))
    });

    if is_children_wrapper {
        // Unwrap children as roots.
        if let Some(patches) = jackdaw_ast.get_patches(root) {
            if let Some(patch) = jackdaw_ast.get_patch(patches.0[0]) {
                if let BsnPatch::Children(children) = patch {
                    let children = children.clone();
                    for child in children {
                        jackdaw_ast.add_to_roots(child);
                    }
                    return Ok(jackdaw_ast);
                }
            }
        }
    }

    jackdaw_ast.add_to_roots(root);
    Ok(jackdaw_ast)
}

/// Spawn ECS entities from the [`SceneBsnAst`] resource, linking them back to
/// AST nodes. All entities are marked [`AstDirty`] so the apply system
/// populates ECS components.
pub fn spawn_from_ast(world: &mut World) -> Vec<Entity> {
    let roots: Vec<Entity> = world.resource::<SceneBsnAst>().roots.clone();
    let mut spawned = Vec::new();

    for root in roots {
        spawn_ast_node(world, root, None, &mut spawned);
    }

    spawned
}

fn spawn_ast_node(
    world: &mut World,
    ast_entity: Entity,
    parent: Option<Entity>,
    spawned: &mut Vec<Entity>,
) {
    let ecs_entity = world
        .spawn((
            AstNodeRef { patches_entity: ast_entity },
            AstDirty,
            Visibility::default(),
        ))
        .id();

    if let Some(parent) = parent {
        world.entity_mut(ecs_entity).insert(ChildOf(parent));
    }

    // Link ECS ↔ AST in the resource.
    world
        .resource_mut::<SceneBsnAst>()
        .link(ecs_entity, ast_entity);

    spawned.push(ecs_entity);

    // Recurse into children.
    let children_ast = {
        let ast = world.resource::<SceneBsnAst>();
        let Some(patches) = ast.get_patches(ast_entity) else {
            return;
        };
        let mut children = Vec::new();
        for &pe in &patches.0 {
            if let Some(BsnPatch::Children(child_list)) = ast.get_patch(pe) {
                children.extend(child_list.iter().copied());
            }
        }
        children
    };

    for child_ast in children_ast {
        spawn_ast_node(world, child_ast, Some(ecs_entity), spawned);
    }
}

fn adapt_patches(
    bevy_ast: &BsnAst,
    patches_id: bevy::prelude::Entity,
    jd_ast: &mut SceneBsnAst,
) -> Result<Entity, BsnLoadError> {
    let Some(bevy_patches) = bevy_ast.0.get::<BevyBsnPatches>(patches_id) else {
        return Err(BsnLoadError::NoAstNode);
    };

    let mut jd_patch_entities = Vec::new();
    for &patch_id in &bevy_patches.0 {
        let Some(bevy_patch) = bevy_ast.0.get::<BevyBsnPatch>(patch_id) else {
            continue;
        };
        match bevy_patch {
            BevyBsnPatch::Name(name, _index) => {
                let pe = jd_ast
                    .world
                    .spawn(BsnPatch::Name(name.clone()))
                    .id();
                jd_patch_entities.push(pe);
            }
            BevyBsnPatch::Base(path) => {
                let pe = jd_ast
                    .world
                    .spawn(BsnPatch::Base(path.clone()))
                    .id();
                jd_patch_entities.push(pe);
            }
            BevyBsnPatch::Var(var) => {
                let type_path = symbol_to_path(&var.0);
                let is_template = var.1;
                let pe = if is_template {
                    jd_ast
                        .world
                        .spawn(BsnPatch::Template(type_path, None))
                        .id()
                } else {
                    jd_ast.world.spawn(BsnPatch::Type(type_path)).id()
                };
                jd_patch_entities.push(pe);
            }
            BevyBsnPatch::Struct(bsn_struct) => {
                let type_path = symbol_to_path(&bsn_struct.0);
                let is_template = bsn_struct.2;
                let fields = adapt_struct_fields(bevy_ast, &bsn_struct.1);
                let pe = if is_template {
                    jd_ast
                        .world
                        .spawn(BsnPatch::Template(type_path, Some(fields)))
                        .id()
                } else {
                    jd_ast
                        .world
                        .spawn(BsnPatch::Struct(BsnStructData {
                            type_path,
                            fields,
                        }))
                        .id()
                };
                jd_patch_entities.push(pe);
            }
            BevyBsnPatch::NamedTuple(tuple) => {
                let type_path = symbol_to_path(&tuple.0);
                let values = adapt_tuple_values(bevy_ast, &tuple.1);
                let pe = jd_ast
                    .world
                    .spawn(BsnPatch::TupleStruct(BsnTupleStructData {
                        type_path,
                        values,
                    }))
                    .id();
                jd_patch_entities.push(pe);
            }
            BevyBsnPatch::Relation(relation) => {
                // Only Children relations are supported.
                let mut child_entities = Vec::new();
                for &child_patches_id in &relation.1 {
                    if let Ok(child) = adapt_patches(bevy_ast, child_patches_id, jd_ast) {
                        child_entities.push(child);
                    }
                }
                let pe = jd_ast
                    .world
                    .spawn(BsnPatch::Children(child_entities))
                    .id();
                jd_patch_entities.push(pe);
            }
        }
    }

    Ok(jd_ast.world.spawn(BsnPatches(jd_patch_entities)).id())
}

fn adapt_struct_fields(bevy_ast: &BsnAst, fields: &[BevyBsnField]) -> BsnStructFields {
    let mut jd_fields = Vec::new();
    for field in fields {
        let value = adapt_expr(bevy_ast, field.1);
        jd_fields.push(BsnField {
            name: field.0.clone(),
            value,
        });
    }
    BsnStructFields(jd_fields)
}

fn adapt_tuple_values(bevy_ast: &BsnAst, expr_ids: &[bevy::prelude::Entity]) -> Vec<BsnValue> {
    expr_ids.iter().map(|&id| adapt_expr(bevy_ast, id)).collect()
}

fn adapt_expr(bevy_ast: &BsnAst, expr_id: bevy::prelude::Entity) -> BsnValue {
    let Some(expr) = bevy_ast.0.get::<BsnExpr>(expr_id) else {
        return BsnValue::String("<error>".into());
    };
    match expr {
        BsnExpr::Var(var) => {
            let path = symbol_to_path(&var.0);
            BsnValue::Type(path)
        }
        BsnExpr::Struct(bsn_struct) => {
            let type_path = symbol_to_path(&bsn_struct.0);
            let fields = adapt_struct_fields(bevy_ast, &bsn_struct.1);
            BsnValue::Struct(BsnStructData { type_path, fields })
        }
        BsnExpr::NamedTuple(tuple) => {
            let type_path = symbol_to_path(&tuple.0);
            let values = adapt_tuple_values(bevy_ast, &tuple.1);
            BsnValue::TupleStruct(BsnTupleStructData { type_path, values })
        }
        BsnExpr::StringLit(s) => BsnValue::String(s.clone()),
        BsnExpr::FloatLit(f) => BsnValue::Float(*f),
        BsnExpr::BoolLit(b) => BsnValue::Bool(*b),
        BsnExpr::IntLit(i) => BsnValue::Int(*i),
        BsnExpr::List(expr_ids) => {
            let values = adapt_tuple_values(bevy_ast, expr_ids);
            BsnValue::List(values)
        }
    }
}

fn symbol_to_path(sym: &bevy::scene2::dynamic_bsn::BsnSymbol) -> String {
    let mut path = String::new();
    for segment in &sym.0 {
        path.push_str(segment);
        path.push_str("::");
    }
    path.push_str(&sym.1);
    path
}
