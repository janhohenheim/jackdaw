use bevy::prelude::*;
use bevy_enhanced_input::prelude::*;
use jackdaw_api::prelude::*;

pub(super) fn plugin(app: &mut App) {
    app.register_extension::<JackdawCoreExtension>();
}

#[derive(Default)]
pub struct JackdawCoreExtension;

impl JackdawExtension for JackdawCoreExtension {
    fn name() -> String {
        "Jackdaw Core Extension".to_string()
    }
    fn kind() -> ExtensionKind {
        ExtensionKind::Builtin
    }

    fn register(&self, ctx: &mut ExtensionContext) {
        ctx.entity_mut().insert((
            CoreExtensionInputContext,
            actions!(
                CoreExtensionInputContext
                    [(Action::<CancelModalOp>::new(), bindings!(KeyCode::Escape))]
            ),
        ));
        crate::draw_brush::add_to_extension(ctx);
    }

    fn register_input_context(app: &mut App) {
        app.add_input_context::<CoreExtensionInputContext>();
    }
}

#[derive(Component, Default)]
pub struct CoreExtensionInputContext;

#[operator(
    id = "modal.cancel",
    label = "Cancel Tool",
    description = "Cancels the currently active tool",
    allows_undo = false
)]
fn cancel_modal(_: In<OperatorParameters>, mut active: ActiveModalQuery) -> OperatorResult {
    active.cancel();
    OperatorResult::Finished
}
