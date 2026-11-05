//! Generation errors (spec 02 A5).

#[derive(Debug, thiserror::Error)]
pub enum GenError {
    /// Internal signal: a stage asks the driver to restart the whole world
    /// with a new attempt number (new streams). Never returned to callers.
    #[error("retry requested: {0}")]
    Retry(String),

    /// All `max_world_attempts` attempts were consumed.
    #[error("world generation failed after {attempts} attempts: {reasons:?}")]
    Exhausted { attempts: u32, reasons: Vec<String> },

    /// The `WorldGenConfig` cannot produce a world (nonsense values).
    #[error("invalid config: {0}")]
    Config(String),

    /// Seed text could not be parsed.
    #[error("invalid seed: {0}")]
    Seed(String),

    /// A finished map broke an A5.10 invariant (a generator bug).
    #[error("world invariant violated: {0}")]
    Invalid(String),

    /// A RON config/map could not be parsed.
    #[error("serialisation error: {0}")]
    Serde(String),
}

impl From<ron::de::SpannedError> for GenError {
    fn from(e: ron::de::SpannedError) -> Self {
        GenError::Serde(e.to_string())
    }
}

impl From<ron::Error> for GenError {
    fn from(e: ron::Error) -> Self {
        GenError::Serde(e.to_string())
    }
}
