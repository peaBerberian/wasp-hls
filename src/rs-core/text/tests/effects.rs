use super::*;
#[test]
fn group_opacity_is_multiplied_once_and_inline_opacity_stays_local() {
    let d=ttml("<body tts:opacity='0.5'><div tts:opacity='0.5'><p tts:opacity='0.5'><span tts:opacity='0.5'>x</span></p></div></body>");
    let p = d.nodes.iter().find(|n| n.kind == Kind::Paragraph).unwrap();
    assert_eq!(p.effects.opacity, 0.125);
    assert_eq!(leaf(&d, "x").effects.opacity, 1.0);
    assert_eq!(d.nodes[leaf(&d, "x").parent.unwrap()].effects.opacity, 0.5);
}
#[test]
fn display_visibility_outlines_and_invalid_blur_are_bounded() {
    let d=ttml("<body tts:display='none'><p tts:display='auto'><span tts:visibility='hidden' tts:textOutline='red 2px'>x</span></p></body>");
    let e = leaf(&d, "x").effects;
    assert!(!e.displayed);
    assert!(!e.visible);
    assert_eq!(
        e.outline,
        Some(Outline {
            color: Some([255, 0, 0, 255]),
            width: Length::Pixels(2.0)
        })
    );
    let d = ttml("<body><p tts:textOutline='red 2px 1px'>x</p></body>");
    assert!(leaf(&d, "x").effects.outline.is_none());
    let d = vtt("STYLE\n::cue {opacity:0.5;visibility:hidden}\n\n00:00.000 --> 00:02.000\nx");
    assert_eq!(d.nodes[0].effects.opacity, 0.5);
    assert!(!leaf(&d, "x").effects.visible);
}
#[test]
fn accepted_and_rejected_property_values() {
    values(include_str!("fixtures/effect-values.txt"));
}
