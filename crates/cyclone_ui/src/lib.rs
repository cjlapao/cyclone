//! cyclone_ui — widget layer on bevy_ui (spec 01).
//!
//! v1 provides `MenuButton` + keyboard focus + theme; Slider/Toggle/
//! Panel/TextInput grow here as the settings panel lands.

use bevy::ecs::message::Message;
use bevy::prelude::*;
use bevy::ui::{BackgroundColor, FocusPolicy};
use cyclone_engine::{Action, Actions};

/// Fired when a menu button activates (mouse click, Enter/Space on focus).
#[derive(Message, Clone, Copy, Debug)]
pub struct Activated(pub u32);

#[derive(Component, Clone, Copy)]
pub struct MenuButton {
    pub id: u32,
}

/// Keyboard/gamepad focus bookkeeping; screens seed `buttons` at OnEnter.
#[derive(Resource, Default)]
pub struct MenuFocus {
    pub buttons: Vec<Entity>,
    pub index: usize,
}

impl MenuFocus {
    pub fn focused(&self) -> Option<Entity> {
        self.buttons.get(self.index).copied()
    }
}

/// Shared fonts, colours, sizes (spec 01 Theme).
#[derive(Resource)]
pub struct Theme {
    pub bold: Handle<Font>,
    pub regular: Handle<Font>,
    /// Window-height-relative scale (h/800), refreshed each frame.
    pub scale: f32,
}

impl Theme {
    pub fn title(&self, text: impl Into<String>) -> (Text, TextFont, TextColor) {
        self.bundle(text, &self.bold, 56.0, Color::srgb(0.98, 0.93, 0.80))
    }

    pub fn body(&self, text: impl Into<String>) -> (Text, TextFont, TextColor) {
        self.bundle(text, &self.regular, 22.0, Color::WHITE)
    }

    pub fn small(&self, text: impl Into<String>) -> (Text, TextFont, TextColor) {
        self.bundle(text, &self.regular, 15.0, Color::srgba(1.0, 1.0, 1.0, 0.75))
    }

    fn bundle(
        &self,
        text: impl Into<String>,
        font: &Handle<Font>,
        px: f32,
        color: Color,
    ) -> (Text, TextFont, TextColor) {
        (
            Text::new(text),
            TextFont {
                font: font.clone().into(),
                font_size: FontSize::Px(px * self.scale),
                ..default()
            },
            TextColor(color),
        )
    }
}

const BG_NORMAL: Color = Color::srgba(0.10, 0.13, 0.18, 0.86);
const BG_HOVER: Color = Color::srgba(0.20, 0.36, 0.52, 0.92);
const BG_FOCUS: Color = Color::srgb(0.91, 0.55, 0.13); // accent

/// Spawn a themed menu button returning the root Entity.
pub fn menu_button(
    parent: &mut ChildSpawnerCommands,
    theme: &Theme,
    id: u32,
    label: &str,
) -> Entity {
    let scale = theme.scale;
    parent
        .spawn((
            Node {
                width: Val::Px(280.0 * scale),
                height: Val::Px(54.0 * scale),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                margin: UiRect::top(Val::Px(6.0 * scale)),
                ..default()
            },
            BackgroundColor(BG_NORMAL),
            FocusPolicy::Block,
            MenuButton { id },
        ))
        .with_child((
            Text::new(label),
            TextFont {
                font: theme.bold.clone().into(),
                font_size: FontSize::Px(24.0 * scale),
                ..default()
            },
            TextColor::WHITE,
        ))
        .id()
}

fn focus_nav(
    mut focus: ResMut<MenuFocus>,
    actions: Res<Actions>,
    buttons: Query<&MenuButton>,
    mut sent: MessageWriter<Activated>,
) {
    if focus.buttons.is_empty() {
        return;
    }
    let n = focus.buttons.len();
    if actions.just_pressed(Action::Up) {
        focus.index = (focus.index + n - 1) % n;
    } else if actions.just_pressed(Action::Down) {
        focus.index = (focus.index + 1) % n;
    } else if actions.just_pressed(Action::Confirm)
        && let Some(e) = focus.focused()
        && let Ok(btn) = buttons.get(e)
    {
        sent.write(Activated(btn.id));
    }
}

#[expect(
    deprecated,
    reason = "classic Interaction until the picking::hover migration (DECISIONS.md)"
)]
fn button_visuals(
    focus: Res<MenuFocus>,
    mut buttons: Query<(&MenuButton, Entity, &mut BackgroundColor, &Interaction)>,
    mut sent: MessageWriter<Activated>,
    mut prev_pressed: Local<std::collections::HashSet<Entity>>,
) {
    let focused = focus.focused();
    for (btn, e, mut bg, interaction) in &mut buttons {
        let pressed = *interaction == Interaction::Pressed;
        if pressed && !prev_pressed.contains(&e) {
            sent.write(Activated(btn.id));
        }
        if pressed {
            prev_pressed.insert(e);
        } else {
            prev_pressed.remove(&e);
        }
        let hover = matches!(*interaction, Interaction::Pressed | Interaction::Hovered);
        **bg = if Some(e) == focused {
            BG_FOCUS
        } else if hover {
            BG_HOVER
        } else {
            BG_NORMAL
        };
    }
}

fn theme_scale(mut theme: ResMut<Theme>, windows: Query<&Window>) {
    // Query, not Single: headless tests run without a window (DECISIONS.md).
    let Ok(window) = windows.single() else {
        return;
    };
    let h = window.resolution.physical_height().max(1) as f32;
    let s = (h / 800.0).clamp(0.6, 2.5);
    if (theme.scale - s).abs() > 0.01 {
        theme.scale = s;
    }
}

pub struct CycloneUiPlugin;

impl Plugin for CycloneUiPlugin {
    fn build(&self, app: &mut App) {
        let (bold, regular) = {
            let assets = app.world().resource::<AssetServer>();
            (
                assets.load("fonts/LiberationSans-Bold.ttf"),
                assets.load("fonts/LiberationSans-Regular.ttf"),
            )
        };
        app.insert_resource(Theme {
            bold,
            regular,
            scale: 1.0,
        });
        app.init_resource::<MenuFocus>();
        app.add_message::<Activated>();
        app.add_systems(Update, (theme_scale, focus_nav, button_visuals).chain());
    }
}
