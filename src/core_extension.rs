use bevy::prelude::*;
use bevy_enhanced_input::prelude::*;
use jackdaw_api::{
    lifecycle::{ActiveModalOperator, OperatorEntity},
    operator::cancel_operator,
    prelude::*,
};

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
                    [(Action::<CancelModal>::new(), bindings!(KeyCode::Escape))]
            ),
        ));
        ctx.add_observer(cancel_modal);
        crate::draw_brush::add_to_extension(ctx);
    }

    fn register_input_context(app: &mut App) {
        app.add_input_context::<CoreExtensionInputContext>();
    }
}

#[derive(Component, Default)]
pub struct CoreExtensionInputContext;

#[derive(Component, InputAction)]
#[action_output(bool)]
struct CancelModal;

fn cancel_modal(
    _: On<Fire<CancelModal>>,
    op: Single<&OperatorEntity, With<ActiveModalOperator>>,
    mut commands: Commands,
) {
    let op = op.into_inner().clone();
    commands.queue(move |world: &mut World| {
        if let Err(err) = world.run_system_cached_with(cancel_operator, op) {
            error!("Failed to finalize cancel operator: {err:?}");
        }
    });
}
