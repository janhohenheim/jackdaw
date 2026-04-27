//! Extends the `minimal_operator` example by adding new content to a panel.

use std::sync::Arc;

use bevy::prelude::*;
use jackdaw::prelude::*;
use jackdaw_feathers::button::{ButtonProps, button};

fn main() -> AppExit {
    App::new()
        .add_plugins((
            DefaultPlugins,
            EditorPlugins::default()
                .set(ExtensionPlugin::default().with_extension::<PanelExampleExtension>()),
        ))
        .run()
}

#[derive(Default)]
pub struct PanelExampleExtension;

impl JackdawExtension for PanelExampleExtension {
    fn id(&self) -> String {
        "panel_example".to_string()
    }

    fn label(&self) -> String {
        "Panel Example".to_string()
    }

    fn description(&self) -> String {
        "Adds a panel to the component inspector".to_string()
    }

    fn register(&self, ctx: &mut ExtensionContext) {
        ctx.register_operator::<ElapsedSecondsOp>()
            // TODO: clean up this API
            .extend_window::<InspectorWindow>(Arc::new(|world, panel| {
                world
                    .entity_mut(panel.panel_entity)
                    .with_child(button(ButtonProps::from_operator::<ElapsedSecondsOp>()));
            }));
    }
}

#[operator(
    id = "panel_example.elapsed_seconds",
    label = "Log Elapsed Seconds",
    description = "Logs the elapsed seconds since Jackdaw started."
)]
fn elapsed_seconds(_: In<OperatorParameters>, time: Res<Time>) -> OperatorResult {
    info!("Elapsed seconds: {}", time.elapsed_secs());
    OperatorResult::Finished
}
