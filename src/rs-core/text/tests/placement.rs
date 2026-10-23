use super::*;
#[test]
fn ttml_regions_use_plain_geometry_and_scale_document_pixels() {
    let xml=wrap("<head><layout><region xml:id='r' tts:origin='64px 180px' tts:extent='512px 180px' tts:displayAlign='center'/></layout></head><body><p region='r'>x</p></body>").replace("<tt ","<tt tts:extent='640px 360px' ");
    let d = imsc::parse(xml.as_bytes()).unwrap();
    let r = d
        .nodes
        .iter()
        .find(|n| n.kind == Kind::Region)
        .unwrap()
        .placement
        .as_ref()
        .unwrap();
    assert_eq!(r.x.offset, Length::Percent(10.0));
    assert_eq!(r.y.offset, Length::Percent(50.0));
    assert_eq!(r.width, Some(Length::Percent(80.0)));
    assert_eq!(r.content_align, Align::Center);
    assert_eq!(r.x.resolve(360.0, 24.0, 640.0, 100.0), 64.0);
}
#[test]
fn vtt_settings_apply_last_valid_values_and_explicit_anchors() {
    let d=vtt("00:00.000 --> 00:02.000 position:20%,line-left size:80% size:bogus line:25%,center align:left\nx");
    let p = d.nodes[0].placement.as_ref().unwrap();
    assert_eq!(p.x.offset, Length::Percent(20.0));
    assert_eq!(p.x.fraction, 0.0);
    assert_eq!(p.width, Some(Length::Percent(80.0)));
    assert_eq!(p.y.offset, Length::Percent(25.0));
    assert_eq!(p.y.fraction, 0.5);
    assert_eq!(d.nodes[0].appearance.text_align, TextAlign::Left);
    let d = vtt("00:00.000 --> 00:02.000 line:-1\nx");
    let p = d.nodes[0].placement.as_ref().unwrap();
    assert!(p.y.from_end);
    assert_eq!(p.y.fraction, 1.0);
    assert_eq!(p.y.resolve(360.0, 18.0, 360.0, 40.0), 320.0);
}
#[test]
fn regions_and_vertical_text_have_distinct_layout() {
    let d=vtt("REGION\nid:r width:60% lines:2 regionanchor:0%,100% viewportanchor:10%,90%\n\n00:00.000 --> 00:02.000 region:r\nx");
    assert_eq!(d.nodes[0].kind, Kind::Region);
    let p = d.nodes[0].placement.as_ref().unwrap();
    assert_eq!(p.width, Some(Length::Percent(60.0)));
    assert_eq!(p.height, Some(Length::Em(2.4)));
    assert_eq!(p.y.fraction, 1.0);
    for (mode, writing, far) in [
        ("lr", WritingMode::VerticalLr, false),
        ("rl", WritingMode::VerticalRl, true),
    ] {
        let d = vtt(&format!("00:00.000 --> 00:02.000 vertical:{mode}\nx"));
        assert_eq!(d.nodes[0].appearance.writing_mode, writing);
        assert_eq!(d.nodes[0].placement.as_ref().unwrap().x.from_end, far);
    }
}
#[test]
fn accepted_and_rejected_property_values() {
    values(include_str!("fixtures/placement-values.txt"));
}

#[test]
fn region_times_limit_display_and_inline_region_changes_wait() {
    let d=ttml("<head><layout><region xml:id='r' begin='2s' end='3s'/></layout></head><body><p begin='0s' end='10s' region='r'><span region='missing'>x</span></p></body>");
    let r = d.nodes.iter().find(|n| n.kind == Kind::Region).unwrap();
    assert_eq!((r.start, r.end), (2.0, 3.0));
    assert_eq!(leaf(&d, "x").end, 10.0);
}

#[test]
fn invalid_alignment_words_do_not_replace_valid_settings() {
    let d=vtt("00:00.000 --> 00:02.000 position:20%,line-left position:90%,end line:25%,center line:80%,line-left\nx");
    let p = d.nodes[0].placement.as_ref().unwrap();
    assert_eq!(p.x.offset, Length::Percent(20.0));
    assert_eq!(p.y.offset, Length::Percent(25.0));
    assert_eq!(p.y.fraction, 0.5);
}
