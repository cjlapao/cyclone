//! The README promise "edits apply the same frame and persist" has two
//! halves: the panel test covers live edits, this covers the FILE —
//! a changed Settings resource (from any source, panel or not) must
//! reach disk, and unchanged frames must not hammer it.
//! Unix only: directories-rust resolves config_dir from
//! XDG_CONFIG_HOME/HOME, which we redirect before touching anything;
//! Windows resolves AppData via the OS, no env override.

#![cfg(unix)]

use bevy::prelude::*;
use cyclone_engine::{Settings, apply_settings};

#[test]
fn changed_settings_reach_disk_and_idle_frames_do_not() {
    let root = std::env::temp_dir().join(format!("cyclone-cfg-{}", std::process::id()));
    // Safety: first act of a single-test binary — no other thread reads
    // the environment yet (cargo gives this file its own process).
    unsafe {
        std::env::set_var("XDG_CONFIG_HOME", &root);
        std::env::set_var("HOME", &root);
    }

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    let mut s = Settings::default();
    s.vsync = false;
    s.window_width = 1234;
    app.insert_resource(s.clone());
    app.add_systems(Update, apply_settings);
    app.update();

    // directories-rust maps config_dir to $XDG_CONFIG_HOME/rescue
    let path = root.join("rescue").join("settings.ron");
    let written = std::fs::read_to_string(&path).expect("edit reached disk");
    assert!(
        written.contains("window_width: 1234"),
        "file carries the edit: {written}"
    );
    assert_eq!(Settings::load(), s, "reload must equal the edited state");

    // Second edit: file follows (not a write-once accident).
    s.vsync = true;
    app.insert_resource(s.clone());
    app.update();
    let written = std::fs::read_to_string(&path).unwrap();
    assert!(written.contains("vsync: true"), "second edit followed");

    // Idle frames: system runs, changes nothing, touches nothing.
    let before = std::fs::metadata(&path).unwrap().modified().unwrap();
    for _ in 0..5 {
        app.update();
    }
    let after = std::fs::metadata(&path).unwrap().modified().unwrap();
    assert_eq!(
        before, after,
        "unchanged settings must not rewrite the file"
    );

    let _ = std::fs::remove_dir_all(&root);
}
