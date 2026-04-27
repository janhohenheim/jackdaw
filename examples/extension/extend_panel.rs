//! Extends the `minimal_operator` example by adding new content to a panel.

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
            // We extend the inspector window, which you can find by default on the right side of the screen.
            .extend_window(InspectorWindow::ID, |window| {
                // This method here is used exactly like `Commands::with_children`.
                // using `.spawn` will spawn a new entity as a child of the window.
                // While you can style your UI however you want, jackdaw comes with a set of pre-built themed widgets
                // that you can use to have a consistent look and feel. Here, we use the built-in `button` widget,
                // which is directly linked to an operator. Things like the label, tooltip, etc. are automatically
                // set up for us based on the operator definition.
                window.spawn(button(ButtonProps::from_operator::<ElapsedSecondsOp>()));
            });
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
