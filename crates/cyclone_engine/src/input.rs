//! Action-map input layer (spec 00 InputPlugin): game code reads `Actions`,
//! never raw keys. Bindings come from Settings.

use std::collections::BTreeSet;

use bevy::input::gamepad::GamepadButton;
use bevy::input::keyboard::KeyCode;
use bevy::input::keyboard::KeyboardInput;
use bevy::prelude::*;

use crate::settings::Settings;

/// Every semantic input the game understands (spec 00 table, plus the
/// preview-screen keys from spec 01).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Action {
    Up,
    Down,
    Left,
    Right,
    Confirm,
    Back,
    RotateCamLeft,
    RotateCamRight,
    Climb,
    Descend,
    Map,
    Pause,
    Reroll,
    CopySeed,
    TopDown,
}

/// Compiled key bindings per action (from Settings).
#[derive(Resource, Default)]
pub struct ActionBindings {
    by_action: Vec<(Action, Vec<KeyCode>, Vec<GamepadButton>)>,
}

impl ActionBindings {
    fn rebuild(settings: &Settings) -> Self {
        let map = settings.keymap();
        let get = |name: &str| map.get(name).cloned().unwrap_or_default();
        let kb = |pairs: &[(Action, &str)]| {
            pairs
                .iter()
                .map(|(a, n)| (*a, get(n), Vec::new()))
                .collect()
        };
        Self {
            by_action: kb(&[
                (Action::Up, "up"),
                (Action::Down, "down"),
                (Action::Left, "left"),
                (Action::Right, "right"),
                (Action::Confirm, "confirm"),
                (Action::Back, "back"),
                (Action::RotateCamLeft, "rotate_left"),
                (Action::RotateCamRight, "rotate_right"),
                (Action::Climb, "climb"),
                (Action::Descend, "descend"),
                (Action::Map, "map"),
                (Action::Pause, "pause"),
                (Action::Reroll, "reroll"),
                (Action::CopySeed, "copy_seed"),
                (Action::TopDown, "topdown"),
            ]),
        }
    }
}

/// Snapshot of action state for the current frame, read by game systems.
#[derive(Resource, Default)]
pub struct Actions {
    pressed: BTreeSet<Action>,
    just_pressed: BTreeSet<Action>,
    just_released: BTreeSet<Action>,
}

impl Actions {
    pub fn pressed(&self, a: Action) -> bool {
        self.pressed.contains(&a)
    }
    pub fn just_pressed(&self, a: Action) -> bool {
        self.just_pressed.contains(&a)
    }
    #[allow(dead_code)]
    pub fn just_released(&self, a: Action) -> bool {
        self.just_released.contains(&a)
    }
}

fn rebuild_bindings(settings: Res<Settings>, mut bindings: ResMut<ActionBindings>) {
    if settings.is_changed() {
        *bindings = ActionBindings::rebuild(&settings);
    }
}

fn update_actions(
    kb: Res<ButtonInput<KeyCode>>,
    gp: Option<Res<ButtonInput<GamepadButton>>>,
    bindings: Res<ActionBindings>,
    mut actions: ResMut<Actions>,
) {
    actions.pressed.clear();
    actions.just_pressed.clear();
    actions.just_released.clear();
    // ButtonInput<GamepadButton> only exists with gilrs; gamepad input is
    // treated as optional everywhere (spec 00).
    let pad = |g: &ButtonInput<GamepadButton>, b: GamepadButton| {
        (g.pressed(b), g.just_pressed(b), g.just_released(b))
    };
    for (action, keys, pads) in &bindings.by_action {
        let (pd, pjp, pjr): (bool, bool, bool) = pads
            .iter()
            .map(|b| gp.as_ref().map_or((false, false, false), |g| pad(g, *b)))
            .fold((false, false, false), |a, v| {
                (a.0 || v.0, a.1 || v.1, a.2 || v.2)
            });
        let down = keys.iter().any(|k| kb.pressed(*k)) || pd;
        let jp = keys.iter().any(|k| kb.just_pressed(*k)) || pjp;
        let jr = keys.iter().any(|k| kb.just_released(*k)) || pjr;
        if down {
            actions.pressed.insert(*action);
        }
        if jp {
            actions.just_pressed.insert(*action);
        }
        if jr {
            actions.just_released.insert(*action);
        }
    }
}

/// Actions layer on top of `bevy_input::InputPlugin`.
pub struct ActionsPlugin;

impl Plugin for ActionsPlugin {
    fn build(&self, app: &mut App) {
        // The Bevy plugin owns the message resources (KeyboardInput,
        // MouseWheel, …) and the ButtonInput resources the actions read;
        // DefaultPlugins normally provides it, so only add it if missing.
        use bevy::ecs::message::Messages;
        if !app.world().contains_resource::<Messages<KeyboardInput>>() {
            app.add_plugins(bevy::input::InputPlugin);
        }
        app.init_resource::<ActionBindings>();
        app.init_resource::<Actions>();
        app.add_systems(Update, rebuild_bindings);
        // Screens read `Actions` this frame, so refresh first.
        app.add_systems(First, update_actions);
    }
}
