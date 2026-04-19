use bevy::prelude::*;
use jackdaw_api::prelude::*;
mod util;

#[test]
fn run_integration_tests() {
    let mut app = util::headless_app();
    app.register_extension::<IntegrationTestsExtension>();
    app.finish();
    app.update();
    app.world_mut()
        .call_operator(IntegrationTestsExtension::TEST, props![])
        .unwrap()
        .assert_finished_i_agree_to_only_use_this_in_integration_tests_and_not_production();
}

#[derive(Default)]
pub struct IntegrationTestsExtension;

impl IntegrationTestsExtension {
    const TEST: &'static str = "integration_test.run_test";
}

impl JackdawExtension for IntegrationTestsExtension {
    fn name() -> String {
        "Integration Tests".to_string()
    }

    fn register(&self, ctx: &mut ExtensionContext) {
        ctx.register_operator::<IntegrationTestOp>();
    }
}

#[derive(Component, Default)]
pub struct SampleContext;

#[operator(
    id = IntegrationTestsExtension::TEST,
)]
fn integration_test(_: In<CustomProperties>) -> OperatorResult {
    // TODO: run integration tests here, possibly using the params to select which tests to run
    OperatorResult::Finished
}
