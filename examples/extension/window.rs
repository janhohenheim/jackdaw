//! Extends the `extend_panel` example by moving its contents into an own window.

use bevy::prelude::*;
use jackdaw::prelude::*;
use jackdaw_feathers::{
    button::{ButtonProps, button},
    tokens::{FONT_LG, FONT_MD},
};

fn main() -> AppExit {
    App::new()
        .add_plugins((
            DefaultPlugins,
            EditorPlugins::default()
                .set(ExtensionPlugin::default().with_extension::<WindowExampleExtension>()),
        ))
        .run()
}

#[derive(Default)]
pub struct WindowExampleExtension;

impl JackdawExtension for WindowExampleExtension {
    fn id(&self) -> String {
        "window_example".to_string()
    }

    fn label(&self) -> String {
        "Window Example".to_string()
    }

    fn description(&self) -> String {
        "Adds a new window that can be placed in the editor by the user.".to_string()
    }

    fn register(&self, ctx: &mut ExtensionContext) {
        ctx.register_operator::<ElapsedSecondsOp>().register_window(
            WindowDescriptor::new("window_example.window")
                .with_name("Example Window")
                .with_default_area("left")
                .with_build(|window| {
                    window.spawn((
                        Text::new("Here's your very own window!"),
                        TextFont::from_font_size(FONT_LG),
                    ));
                    window.spawn((
                        Text::new("Click the button below to log the elapsed seconds"),
                        TextFont::from_font_size(FONT_MD),
                    ));
                    window.spawn(button(ButtonProps::from_operator::<ElapsedSecondsOp>()));
                }),
        );
    }
}

#[operator(
    id = "window_example.elapsed_seconds",
    label = "Log Elapsed Seconds",
    description = "Logs the elapsed seconds since Jackdaw started."
)]
fn elapsed_seconds(_: In<OperatorParameters>, time: Res<Time>) -> OperatorResult {
    info!("Elapsed seconds: {}", time.elapsed_secs());
    OperatorResult::Finished
}
