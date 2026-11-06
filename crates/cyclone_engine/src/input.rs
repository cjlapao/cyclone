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
    /// Free-camera fly boost.
    Boost,
}

/// Compiled key bindings per action (from Settings).
#[derive(Resource, Default)]
pub struct ActionBindings {
    by_action: Vec<(Action, Vec<KeyCode>, Vec<GamepadButton>)>,
}

// Bevy 0.20 has NO ButtonInput<GamepadButton> resource feed: button state
// lives in the `Gamepad` entity component, updated from
// `GamepadButtonStateChangedEvent`, whose production source is
// `bevy_gilrs` (whose Linux backend needs pkg-config + libudev headers at
// build time, so GilrsPlugin ships behind the optional `gamepad`
// feature — DECISIONS round 49). `mirror_gamepad` below bridges those
// events into `Actions` regardless: with the feature a real pad drives
// the menus, without it the same code is driven by injected events in
// tests. Either way the mapping is live code, never a phantom resource.

/// Held buttons mirrored from `GamepadButtonStateChangedEvent`s.
#[derive(Resource, Default)]
struct GamepadState {
    held: BTreeSet<GamepadButton>,
    newly: BTreeSet<GamepadButton>,
    newly_released: BTreeSet<GamepadButton>,
}

fn mirror_gamepad(
    mut rx: MessageReader<bevy::input::gamepad::GamepadButtonStateChangedEvent>,
    mut st: ResMut<GamepadState>,
) {
    st.newly.clear();
    st.newly_released.clear();
    for e in rx.read() {
        match e.state {
            bevy::input::ButtonState::Pressed => {
                if st.held.insert(e.button) {
                    st.newly.insert(e.button);
                }
            }
            bevy::input::ButtonState::Released => {
                if st.held.remove(&e.button) {
                    st.newly_released.insert(e.button);
                }
            }
        }
    }
}

/// Spec 01 acceptance: every action also has a gamepad button, so the
/// game and menus work gamepad-only. Stick axes (view-relative fly, menu
/// scroll speed) arrive with the helicopter in spec 03; buttons cover
/// everything now.
fn gamepad_buttons(a: Action) -> Vec<GamepadButton> {
    use GamepadButton::*;
    match a {
        Action::Up => vec![DPadUp],
        Action::Down => vec![DPadDown],
        Action::Left => vec![DPadLeft],
        Action::Right => vec![DPadRight],
        Action::Confirm => vec![South],
        Action::Back => vec![East],
        // bevy 0.20's GamepadButton has no *Trigger2/stick-press aliases;
        // dual roles are per-state safe (each screen reads its own subset).
        Action::RotateCamLeft => vec![West], // preview reads rotate, not CopySeed
        Action::RotateCamRight => vec![North], // reroll-on-N also rotates: invisible
        Action::Climb => vec![RightTrigger], // R1 / RB hold
        Action::Descend => vec![LeftTrigger], // L1 / LB hold
        Action::Map => vec![Select],
        Action::Pause => vec![Start],
        Action::Reroll => vec![North],
        Action::CopySeed => vec![RightThumb],
        Action::TopDown => vec![Mode],
        Action::Boost => vec![LeftThumb], // stick press
    }
}

impl ActionBindings {
    fn rebuild(settings: &Settings) -> Self {
        let map = settings.keymap();
        let get = |name: &str| map.get(name).cloned().unwrap_or_default();
        let kb = |pairs: &[(Action, &str)]| {
            pairs
                .iter()
                .map(|(a, n)| (*a, get(n), gamepad_buttons(*a)))
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
                (Action::Boost, "boost"),
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
    gp: Res<GamepadState>,
    bindings: Res<ActionBindings>,
    mut actions: ResMut<Actions>,
) {
    actions.pressed.clear();
    actions.just_pressed.clear();
    actions.just_released.clear();
    let pad = |b: GamepadButton| {
        (
            gp.held.contains(&b),
            gp.newly.contains(&b),
            gp.newly_released.contains(&b),
        )
    };
    for (action, keys, pads) in &bindings.by_action {
        let (pd, pjp, pjr): (bool, bool, bool) = pads
            .iter()
            .map(|b| pad(*b))
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
        app.init_resource::<GamepadState>();
        app.add_systems(Update, rebuild_bindings);
        // Screens read `Actions` this frame, so refresh first.
        app.add_systems(First, mirror_gamepad.before(update_actions));
        app.add_systems(First, update_actions);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Spec 01 C2 (gamepad only): no action may lack a gamepad fallback.
    #[test]
    fn every_action_has_a_gamepad_button() {
        let all = [
            Action::Up,
            Action::Down,
            Action::Left,
            Action::Right,
            Action::Confirm,
            Action::Back,
            Action::RotateCamLeft,
            Action::RotateCamRight,
            Action::Climb,
            Action::Descend,
            Action::Map,
            Action::Pause,
            Action::Reroll,
            Action::CopySeed,
            Action::TopDown,
            Action::Boost,
        ];
        for a in all {
            assert!(!gamepad_buttons(a).is_empty(), "no gamepad for {a:?}");
        }
    }

    #[test]
    fn rebuild_keeps_keyboard_and_gamepad_together() {
        let b = ActionBindings::rebuild(&Settings::default());
        let count = |a: Action| {
            b.by_action
                .iter()
                .find(|(x, _, _)| *x == a)
                .map(|(_, k, g)| (k.len(), g.len()))
        };
        assert_eq!(count(Action::Confirm).unwrap().1, 1); // South
        assert_eq!(count(Action::Up).unwrap().0, 2); // ArrowUp + KeyW
    }
}
