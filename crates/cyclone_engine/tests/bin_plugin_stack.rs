//! The binary's plugin stack — DefaultPlugins *then* CycloneEnginePlugin —
//! is assembled NOWHERE in the headless tests (they skip DefaultPlugins
//! because this box has no display). That blind spot let the engine's
//! unconditional `add_plugins(StatesPlugin)` panic the user's first run:
//! DefaultPlugins already carries StatesPlugin, and duplicate adds are a
//! hard panic. Regression: the engine must tolerate a preexisting one.

use bevy::app::App;
use bevy::state::app::StatesPlugin;
use cyclone_engine::{AppState, CycloneEnginePlugin};

#[test]
fn engine_tolerates_preexisting_states_plugin() {
    let mut app = App::new();
    app.add_plugins(StatesPlugin); // the DefaultPlugins member that collided
    app.add_plugins(CycloneEnginePlugin); // panicked here pre-fix
    // Construction is the regression; running needs plugins the bin
    // carries (Time et al.) that this skeleton omits on purpose.
    assert!(
        app.world()
            .contains_resource::<bevy::prelude::State<AppState>>(),
        "state plumbing survived the tolerated plugin"
    );
}
