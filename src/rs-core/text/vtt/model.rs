//! WebVTT text, time and position values. All units are kept explicit.
use super::css;
use crate::text::Diagnostic;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TimestampMap {
    /// WebVTT time, in milliseconds, on the LOCAL side of X-TIMESTAMP-MAP.
    pub local_ms: u64,
    /// The matching 33-bit MPEG timestamp, in 90 kHz ticks.
    pub mpegts: u64,
}
impl TimestampMap {
    /// Unwrap relative to an actual A/V timestamp in 90 kHz ticks. A new
    /// discontinuity requires a new anchor; this parser has no playback state.
    pub fn map_seconds(&self, cue_ms: u64, anchor_ticks: i64) -> f64 {
        const WRAP: i128 = 1 << 33;
        let raw = i128::from(self.mpegts);
        let cycles = (i128::from(anchor_ticks) - raw + WRAP / 2).div_euclid(WRAP);
        (raw + cycles * WRAP) as f64 / 90_000.0 + cue_ms as f64 / 1000.0
            - self.local_ms as f64 / 1000.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Text alignment. Start and End depend on the text writing direction.
pub enum Align {
    Start,
    Center,
    End,
    Left,
    Right,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineAlign {
    Start,
    Center,
    End,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Line {
    /// Let the renderer choose the line.
    Auto,
    /// A line offset. Negative values count from the far edge.
    Number(f64),
    /// A percentage of the available space, without snapping to text lines.
    Percent(f64),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PositionAlign {
    Auto,
    LineLeft,
    Center,
    LineRight,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Vertical {
    Horizontal,
    /// Vertical text, with new lines moving to the left.
    Rl,
    /// Vertical text, with new lines moving to the right.
    Lr,
}
#[derive(Debug, Clone, PartialEq)]
/// Cue settings, before a renderer turns them into screen positions.
pub struct Settings {
    pub vertical: Vertical,
    pub line: Line,
    pub line_align: LineAlign,
    /// Percentage position, or None for WebVTT automatic positioning.
    pub position: Option<f64>,
    pub position_align: PositionAlign,
    /// Cue size as a percentage of the available width or height.
    pub size: f64,
    pub align: Align,
    pub region: Option<String>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            vertical: Vertical::Horizontal,
            line: Line::Auto,
            line_align: LineAlign::Start,
            position: None,
            position_align: PositionAlign::Auto,
            size: 100.0,
            align: Align::Center,
            region: None,
        }
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct Region {
    pub id: String,
    pub width: f64,
    pub lines: u32,
    /// Point inside the region, as x/y percentages.
    pub region_anchor: (f64, f64),
    /// Matching point inside the video viewport, as x/y percentages.
    pub viewport_anchor: (f64, f64),
    pub scroll_up: bool,
}
impl Default for Region {
    fn default() -> Self {
        Self {
            id: String::new(),
            width: 100.0,
            lines: 3,
            region_anchor: (0.0, 100.0),
            viewport_anchor: (0.0, 100.0),
            scroll_up: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tag {
    Class,
    Italic,
    Bold,
    Underline,
    Ruby,
    RubyText,
    Voice,
    Language,
}
/// A flat arena avoids recursive destruction for malicious cue markup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeKind {
    Root,
    Text(String),
    /// An inline timestamp in milliseconds, used for timed text highlighting.
    Timestamp(u64),
    Element {
        tag: Tag,
        classes: Vec<String>,
        annotation: String,
    },
}
#[derive(Debug, Clone, PartialEq, Eq)]
/// A cue-text entry. Its parent is an earlier entry in the same vector.
pub struct Node {
    pub parent: Option<usize>,
    pub kind: NodeKind,
}
#[derive(Debug, Clone, PartialEq)]
/// A complete cue. Times are not clipped to the segment that carried it.
pub struct Cue {
    pub id: String,
    pub start_ms: u64,
    pub end_ms: u64,
    pub settings: Settings,
    /// Decoded cue payload, including its original WebVTT markup.
    pub raw_text: String,
    /// Parsed markup. Entry zero is the root.
    pub text: Vec<Node>,
}
#[derive(Debug, Clone, Default, PartialEq)]
/// Header data that can also be supplied by an HLS EXT-X-MAP resource.
pub struct Header {
    pub timestamp_map: TimestampMap,
    pub regions: Vec<Region>,
    /// Source CSS is retained, never executed. CSS selector matching and cascade
    /// are deliberately separate from syntax parsing and future layout.
    pub style_sheets: Vec<css::Stylesheet>,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Document {
    pub header: Header,
    pub cues: Vec<Cue>,
    pub diagnostics: Vec<Diagnostic>,
}
