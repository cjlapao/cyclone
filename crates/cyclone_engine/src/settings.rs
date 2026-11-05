//! Settings resource: RON file in the OS config dir (spec 00 SettingsPlugin).
//! Deleting the file restores defaults; unparseable files are replaced.

use std::collections::BTreeMap;
use std::path::PathBuf;

use bevy::input::keyboard::KeyCode;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Default keyboard bindings, stored as key names so Settings can serialise
/// (KeyCode itself is not serde-friendly).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct KeyBindings {
    pub up: Vec<String>,
    pub down: Vec<String>,
    pub left: Vec<String>,
    pub right: Vec<String>,
    pub confirm: Vec<String>,
    pub back: Vec<String>,
    pub rotate_left: Vec<String>,
    pub rotate_right: Vec<String>,
    pub climb: Vec<String>,
    pub descend: Vec<String>,
    pub map: Vec<String>,
    pub pause: Vec<String>,
    pub reroll: Vec<String>,
    pub copy_seed: Vec<String>,
    pub topdown: Vec<String>,
    pub boost: Vec<String>,
}

impl Default for KeyBindings {
    fn default() -> Self {
        let k = |names: &[&str]| names.iter().map(|s| s.to_string()).collect();
        Self {
            up: k(&["ArrowUp", "KeyW"]),
            down: k(&["ArrowDown", "KeyS"]),
            left: k(&["ArrowLeft", "KeyA"]),
            right: k(&["ArrowRight", "KeyD"]),
            confirm: k(&["Enter", "Space"]),
            back: k(&["Escape"]),
            rotate_left: k(&["KeyQ"]),
            rotate_right: k(&["KeyE"]),
            climb: k(&["Space"]),
            descend: k(&["ShiftLeft", "ShiftRight"]),
            map: k(&["KeyM"]),
            pause: k(&["KeyP"]),
            reroll: k(&["KeyR"]),
            copy_seed: k(&["KeyC"]),
            topdown: k(&["Tab"]),
            boost: k(&["ControlLeft", "ControlRight"]),
        }
    }
}

/// Resolve a key name (as saved) into a KeyCode; unknown names drop out.
pub fn parse_key(name: &str) -> Option<KeyCode> {
    Some(match name {
        "KeyW" => KeyCode::KeyW,
        "KeyA" => KeyCode::KeyA,
        "KeyS" => KeyCode::KeyS,
        "KeyD" => KeyCode::KeyD,
        "KeyQ" => KeyCode::KeyQ,
        "KeyE" => KeyCode::KeyE,
        "KeyR" => KeyCode::KeyR,
        "KeyC" => KeyCode::KeyC,
        "KeyM" => KeyCode::KeyM,
        "KeyP" => KeyCode::KeyP,
        "KeyT" => KeyCode::KeyT,
        "ArrowUp" => KeyCode::ArrowUp,
        "ArrowDown" => KeyCode::ArrowDown,
        "ArrowLeft" => KeyCode::ArrowLeft,
        "ArrowRight" => KeyCode::ArrowRight,
        "Enter" => KeyCode::Enter,
        "Space" => KeyCode::Space,
        "Escape" => KeyCode::Escape,
        "Tab" => KeyCode::Tab,
        "ShiftLeft" => KeyCode::ShiftLeft,
        "ShiftRight" => KeyCode::ShiftRight,
        "ControlLeft" => KeyCode::ControlLeft,
        "ControlRight" => KeyCode::ControlRight,
        _ => return None,
    })
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Resource)]
#[serde(default)]
pub struct Settings {
    pub fullscreen: bool,
    pub vsync: bool,
    pub window_width: u32,
    pub window_height: u32,
    /// Multiplier on the camera heading tween speed (spec 01 settings panel).
    pub camera_rotation_speed: f32,
    pub keys: KeyBindings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            fullscreen: false,
            vsync: true,
            window_width: 1280,
            window_height: 800,
            camera_rotation_speed: 1.0,
            keys: KeyBindings::default(),
        }
    }
}

impl Settings {
    fn file_path() -> Option<PathBuf> {
        let dirs = directories::ProjectDirs::from("com", "cyclone", "rescue")?;
        let dir = dirs.config_dir();
        let _ = std::fs::create_dir_all(dir);
        Some(dir.join("settings.ron"))
    }

    /// Load from disk; any problem falls back to defaults (spec 00).
    pub fn load() -> Settings {
        let Some(path) = Self::file_path() else {
            return Settings::default();
        };
        match std::fs::read_to_string(&path) {
            Ok(text) => ron::from_str(&text).unwrap_or_default(),
            Err(_) => Settings::default(),
        }
    }

    pub fn save_to_disk(&self) {
        let Some(path) = Self::file_path() else {
            return;
        };
        if let Ok(text) = ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default()) {
            let _ = std::fs::write(path, text);
        }
    }

    /// BTreeMap keeps binding lookup deterministic for tests.
    pub fn keymap(&self) -> BTreeMap<String, Vec<KeyCode>> {
        let mut out = BTreeMap::new();
        for (name, keys) in [
            ("up", &self.keys.up),
            ("down", &self.keys.down),
            ("left", &self.keys.left),
            ("right", &self.keys.right),
            ("confirm", &self.keys.confirm),
            ("back", &self.keys.back),
            ("rotate_left", &self.keys.rotate_left),
            ("rotate_right", &self.keys.rotate_right),
            ("climb", &self.keys.climb),
            ("descend", &self.keys.descend),
            ("map", &self.keys.map),
            ("pause", &self.keys.pause),
            ("reroll", &self.keys.reroll),
            ("copy_seed", &self.keys.copy_seed),
            ("topdown", &self.keys.topdown),
            ("boost", &self.keys.boost),
        ] {
            out.insert(
                name.to_string(),
                keys.iter().filter_map(|k| parse_key(k)).collect(),
            );
        }
        out
    }
}

pub struct SettingsPlugin;

impl Plugin for SettingsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Settings>();
        // Replace the resource value with the on-disk one (defaults if any).
        let loaded = Settings::load();
        app.insert_resource(loaded);
        app.add_systems(Update, crate::apply_settings);
    }
}
