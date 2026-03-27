mod apply;
mod ast;
mod emitter;
mod loader;
mod sync;

pub use apply::*;
pub use ast::*;
pub use emitter::*;
pub use loader::*;
pub use sync::*;

use bevy::prelude::*;

pub struct JackdawBsnPlugin;

impl Plugin for JackdawBsnPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SceneBsnAst>()
            .add_systems(PostUpdate, apply_dirty_ast_patches);
    }
}
