//! Game state machine (pure logic — no macroquad).
//! States per spec: Title, Playing, Map, Paused, Crashed, RoundComplete, GameOver.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameState {
    Title,
    Playing,
    Map,
    Paused,
    Crashed,
    RoundComplete,
    GameOver,
}

/// Edge-triggered play commands coming from input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayCmd {
    TogglePause,
    QuitToTitle,
    OpenMap,
    CloseMap,
    FlipView,
}

/// Resolve transitions involving overlays. `state` is the current state;
/// returns the new state. Playing→Crashed / →RoundComplete / →GameOver are
/// driven by the simulation, not input, and are not handled here.
pub fn transition(state: GameState, cmd: PlayCmd) -> GameState {
    match (state, cmd) {
        (GameState::Playing, PlayCmd::TogglePause) => GameState::Paused,
        (GameState::Playing, PlayCmd::OpenMap) => GameState::Map,
        (GameState::Paused, PlayCmd::TogglePause) => GameState::Playing,
        (GameState::Paused, PlayCmd::QuitToTitle) => GameState::Title,
        (GameState::Map, PlayCmd::CloseMap) => GameState::Playing,
        (GameState::Map, PlayCmd::TogglePause) => GameState::Playing,
        (s, PlayCmd::FlipView) if s == GameState::Playing => s, // handled by sim via flip timer
        (s, _) => s,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn play_pause_round_trip() {
        let s = transition(GameState::Playing, PlayCmd::TogglePause);
        assert_eq!(s, GameState::Paused);
        assert_eq!(transition(s, PlayCmd::TogglePause), GameState::Playing);
    }

    #[test]
    fn map_returns_to_playing() {
        let s = transition(GameState::Playing, PlayCmd::OpenMap);
        assert_eq!(s, GameState::Map);
        assert_eq!(transition(s, PlayCmd::CloseMap), GameState::Playing);
    }

    #[test]
    fn quit_from_pause_to_title() {
        let s = transition(GameState::Paused, PlayCmd::QuitToTitle);
        assert_eq!(s, GameState::Title);
    }
}
