//! Sample extension. Registers a dock window and a `HelloOp` operator
//! bound to F9. Disabling it in File > Extensions should remove the
//! window entry and kill the keybind.

use std::sync::Arc;

use bevy::prelude::*;
use bevy_enhanced_input::prelude::*;
use jackdaw_api::prelude::*;

pub struct SampleExtension;

impl JackdawExtension for SampleExtension {
    fn name(&self) -> &str {
        "sample"
    }

    fn register_input_contexts(&self, app: &mut App) {
        app.add_input_context::<SampleContext>();
    }

    fn register(&self, ctx: &mut ExtensionContext) {
        ctx.register_window(WindowDescriptor {
            id: "sample.hello".into(),
            name: "Hello Extension".into(),
            icon: None,
            default_area: None,
            priority: None,
            build: Arc::new(build_hello_panel),
        });

        ctx.register_operator::<HelloOp>();

        ctx.spawn((
            SampleContext,
            actions!(SampleContext[
                (Action::<HelloOp>::new(), bindings![KeyCode::F9]),
            ]),
        ));
    }
}

fn build_hello_panel(world: &mut World, parent: Entity) {
    world.spawn((ChildOf(parent), Text::new("Hello from an extension!")));
}

#[derive(Component, Default)]
pub struct SampleContext;

#[operator(
    id = "sample.hello",
    label = "Hello",
    description = "Logs a hello message",
    name = "HelloOp"
)]
fn hello_op() -> OperatorResult {
    info!("Hello from the sample extension operator!");
    OperatorResult::Finished
}
