use super::{Color, Length};
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Outline {
    /// None means use the text color.
    pub color: Option<Color>,
    pub width: Length,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Bidi {
    #[default]
    Normal,
    Embed,
    Override,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Effects {
    /// This opacity applies to the node and its children. A child's opacity
    /// multiplies its parent's opacity.
    pub opacity: f64,
    pub visible: bool,
    pub displayed: bool,
    pub clip: bool,
    pub z_index: Option<i32>,
    /// IMSC1 does not allow blurred text outlines.
    pub outline: Option<Outline>,
    pub bidi: Bidi,
}
impl Default for Effects {
    fn default() -> Self {
        Self {
            opacity: 1.0,
            visible: true,
            displayed: true,
            clip: false,
            z_index: None,
            outline: None,
            bidi: Bidi::Normal,
        }
    }
}
impl Effects {
    pub(crate) fn child(self) -> Self {
        Self {
            visible: self.visible,
            displayed: self.displayed,
            outline: self.outline,
            ..Self::default()
        }
    }
}
// A CSS backend can use four shadows with numeric offsets for an outline.
// Native code can draw a border around each character.
// TODO: Add these fields to the arrays sent to JS.
