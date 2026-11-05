//! Regression test for crash #7: generation must never call
//! `AsyncComputeTaskPool::get()` uninitialised (it panics "has not been
//! initialized yet" when no TaskPoolPlugin ran). `gen_start` must use
//! `get_or_init`, so the OnEnter path works on a bare app.

use bevy::prelude::*;
use cyclone_engine::AppState;

#[test]
fn gen_start_works_without_task_pool_plugin() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    // States come from a bare StatesPlugin (not MinimalPlugins) so OnEnter
    // fires, and no TaskPoolPlugin exists anywhere — that is the point.
    app.add_plugins(bevy::state::app::StatesPlugin)
        .init_state::<AppState>();
    // Deliberately do NOT initialise AsyncComputeTaskPool.
    cyclone_game::register_test(&mut app);
    // Input messages that ActionsPlugin would register from bevy's InputPlugin.
    app.add_message::<bevy::input::keyboard::KeyboardInput>();
    app.add_message::<bevy::input::mouse::MouseWheel>();
    app.add_message::<cyclone_ui::Activated>();
    // Resources CycloneEnginePlugin would normally own; gen_start needs them.
    app.insert_resource(cyclone_game::screens::GameConfig(
        cyclone_worldgen::WorldGenConfig::default(),
    ));
    app.insert_resource(cyclone_engine::RigTarget::default());

    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Generating);
    // Before the fix this panics inside gen_start.
    app.update();
    app.update();
    assert_eq!(
        app.world().resource::<State<AppState>>().get().clone(),
        AppState::Generating
    );
}
