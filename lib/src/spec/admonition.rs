use serde::Serialize;

/// The significance of a structured explanatory block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SpecAdmonitionKind {
    Note,
    Warning,
}

/// A note or warning whose presentation is chosen by the renderer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SpecAdmonition {
    pub kind: SpecAdmonitionKind,
    pub text: String,
}

impl SpecAdmonition {
    pub fn note(text: impl Into<String>) -> Self {
        Self {
            kind: SpecAdmonitionKind::Note,
            text: text.into(),
        }
    }

    pub fn warning(text: impl Into<String>) -> Self {
        Self {
            kind: SpecAdmonitionKind::Warning,
            text: text.into(),
        }
    }
}
