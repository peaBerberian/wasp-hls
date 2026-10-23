use crate::text::{imsc, vtt, *};
fn ttml(body: &str) -> Document {
    imsc::parse(format!(r#"<tt xmlns="http://www.w3.org/ns/ttml" xmlns:tts="http://www.w3.org/ns/ttml#styling">{body}</tt>"#).as_bytes()).unwrap()
}
fn text<'a>(doc: &'a Document, value: &str) -> &'a Node {
    doc.nodes
        .iter()
        .find(|n| n.text.as_deref() == Some(value))
        .unwrap()
}
#[test]
fn ordinary_content_and_region_inheritance_is_prepared() {
    let doc = ttml(
        r#"<head><layout><region xml:id="r" tts:color="red" tts:fontSize="20px" tts:extent="80% 25%" tts:origin="10% 70%"/></layout></head><body tts:fontSize="200%"><div><p region="r" begin="1s" end="3s" tts:fontSize="50%"><span>Hello</span><span tts:color="yellow">world</span></p></div></body>"#,
    );
    assert_eq!(text(&doc, "Hello").style["color"].as_ref(), "#ff0000ff");
    assert_eq!(text(&doc, "world").style["color"].as_ref(), "#ffff00ff");
    assert_eq!(text(&doc, "Hello").style["font-size"].as_ref(), "20px");
    assert_eq!(doc.nodes[0].style["left"].as_ref(), "10%");
    assert_eq!(doc.nodes[0].style["top"].as_ref(), "70%");
    assert_eq!(doc.nodes[0].style["height"].as_ref(), "25%");
}
#[test]
fn relative_font_and_line_height_are_not_reapplied_on_children() {
    let doc = ttml(
        r#"<body tts:fontSize="20px"><div><p tts:lineHeight="150%"><span tts:fontSize="50%">small</span></p></div></body>"#,
    );
    assert_eq!(text(&doc, "small").style["font-size"].as_ref(), "10px");
    assert_eq!(text(&doc, "small").style["line-height"].as_ref(), "30px");
}
#[test]
fn inherited_opacity_is_applied_once_and_decoration_can_be_removed() {
    let doc = ttml(
        r#"<body tts:opacity="0.5"><div tts:opacity="0.5"><p tts:opacity="0.5" tts:textDecoration="underline"><span tts:opacity="0.5" tts:textDecoration="noUnderline">plain</span></p></div></body>"#,
    );
    let p = doc.nodes.iter().find(|n| n.tag == Tag::P).unwrap();
    assert_eq!(p.style["opacity"].as_ref(), "0.125");
    let leaf = text(&doc, "plain");
    assert!(!leaf.style.contains_key("opacity"));
    assert_eq!(leaf.style["text-decoration"].as_ref(), "");
    assert_eq!(
        doc.nodes[leaf.parent.unwrap()].style["opacity"].as_ref(),
        "0.5"
    );
}
#[test]
fn xml_whitespace_is_normalized_without_trimming_inline_boundaries() {
    let doc=ttml("<body><div><p>Hello \n <span>world</span>!</p><p xml:space='preserve'> A  <span xml:space='default'> B\t C </span> D </p></div></body>");
    assert_eq!(text(&doc, "Hello ").style["white-space"].as_ref(), "normal");
    assert_eq!(text(&doc, " A  ").style["white-space"].as_ref(), "pre-wrap");
    assert_eq!(text(&doc, " B C ").style["white-space"].as_ref(), "normal");
}
#[test]
fn set_and_extension_values_are_ignored_without_changing_text_or_styles() {
    let doc = ttml(
        r#"<head><styling><style xml:id="s" tts:color="red" tts:fontWeight="bold"/></styling></head><body><div><p style="s" tts:color="invalid" xmlns:e="urn:extension" e:effect="glow"><set begin="bad" tts:color="yellow"/>Hello</p></div></body>"#,
    );
    assert_eq!(text(&doc, "Hello").style["color"].as_ref(), "#ff0000ff");
    assert_eq!(text(&doc, "Hello").style["font-weight"].as_ref(), "bold");
    assert!(doc.nodes.iter().all(|n| n.text.as_deref() != Some("glow")));
}
#[test]
fn vtt_rules_are_matched_and_invalid_values_cannot_override_them() {
    let doc=vtt::parse(b"WEBVTT\n\nSTYLE\n::cue { color: blue !important; color: red; font-weight: bold; font-weight: invalid; }\n::cue(.red) {color:lime}\n::cue(v[voice=\"Alice\"]) {font-style:italic}\n\n00:01.000 --> 00:03.000\n<v Alice><c.red>Hello</c> world</v>",None).unwrap();
    assert_eq!(doc.nodes[1].style["color"].as_ref(), "#0000ffff");
    assert_eq!(doc.nodes[1].style["font-weight"].as_ref(), "bold");
    assert!(doc.nodes.iter().any(|n| n
        .style
        .get("color")
        .is_some_and(|s| s.as_ref() == "#00ff00ff")));
    assert!(doc.nodes.iter().any(|n| n
        .style
        .get("font-style")
        .is_some_and(|s| s.as_ref() == "italic")));
    assert!(doc.nodes.iter().all(|n| n.start == 1.0 && n.end == 3.0));
}
#[test]
fn language_ruby_and_basic_markup_need_no_vtt_specific_renderer() {
    let doc=vtt::parse(b"WEBVTT\n\n00:00.000 --> 00:03.000\n<b>B</b><i>I</i><u>U</u><lang fr>oui</lang><ruby>x<rt>y</rt></ruby><00:02.000>",None).unwrap();
    for (i, n) in doc.nodes.iter().enumerate() {
        assert!(n.parent.is_none_or(|p| p < i));
    }
    assert!(doc.nodes.iter().any(|n| n.tag == Tag::Ruby));
    assert!(doc.nodes.iter().any(|n| n.tag == Tag::Rt));
    assert!(doc
        .nodes
        .iter()
        .any(|n| n.language.as_deref() == Some("fr")));
    for (property, value) in [
        ("font-weight", "bold"),
        ("font-style", "italic"),
        ("text-decoration", "underline"),
    ] {
        assert!(doc
            .nodes
            .iter()
            .any(|n| n.style.get(property).is_some_and(|s| s.as_ref() == value)));
    }
}
#[test]
fn time_mapping_and_mp4_coverage_are_parser_responsibilities() {
    let timeline = Timeline {
        media_time: 10.0,
        playback_time: 3.0,
    };
    let doc = vtt::parse_with_timeline(
        b"WEBVTT\nX-TIMESTAMP-MAP=LOCAL:00:00.000,MPEGTS:900000\n\n00:01.000 --> 00:02.000\nx",
        None,
        timeline,
        Limits::default(),
    )
    .unwrap();
    assert_eq!((doc.nodes[0].start, doc.nodes[0].end), (4.0, 5.0));
    let bytes=br#"<tt xmlns="http://www.w3.org/ns/ttml"><body><div><p begin="0s" end="100s">x</p></div></body></tt>"#;
    let sample = mp4::Sample {
        track_id: 1,
        timescale: 1000,
        decode_time: 10000,
        presentation_time: 10000,
        duration: 1000,
        document: bytes,
    };
    let doc = imsc::parse_sample(&sample, timeline, Limits::default()).unwrap();
    assert!(doc.nodes.iter().all(|n| n.start == 3.0 && n.end == 4.0));
}
#[test]
fn common_output_can_cross_threads() {
    fn check<T: Send + Sync>() {}
    check::<Document>();
}

#[test]
fn vtt_background_wraps_text_and_cue_times_remain_unclipped() {
    let doc = vtt::parse(b"WEBVTT\n\n00:00.000 --> 00:10.000\nHello world", None).unwrap();
    assert!(!doc.nodes[0].style.contains_key("background-color"));
    assert_eq!(
        doc.nodes[1].style["background-color"].as_ref(),
        "rgba(0, 0, 0, 0.8)"
    );
    assert!(doc.nodes.iter().all(|n| n.start == 0.0 && n.end == 10.0));
}
#[test]
fn document_pixels_and_writing_direction_become_css() {
    let doc=imsc::parse(br#"<tt xmlns="http://www.w3.org/ns/ttml" xmlns:tts="http://www.w3.org/ns/ttml#styling" tts:extent="640px 360px"><head><layout><region xml:id="r" tts:origin="64px 180px" tts:extent="512px 180px"/></layout></head><body><p region="r" tts:fontSize="36px" tts:writingMode="rltb" tts:fontFamily="sansSerif">x</p></body></tt>"#).unwrap();
    assert_eq!(doc.nodes[0].style["left"].as_ref(), "10%");
    assert_eq!(doc.nodes[0].style["top"].as_ref(), "50%");
    assert_eq!(
        text(&doc, "x").style["font-size"].as_ref(),
        "calc(var(--subtitle-height) * 0.1)"
    );
    assert_eq!(text(&doc, "x").style["direction"].as_ref(), "rtl");
    assert_eq!(text(&doc, "x").style["font-family"].as_ref(), "sans-serif");
}
#[test]
fn vtt_initialization_and_timestamp_wrap_need_no_renderer_metadata() {
    let init = vtt::parse_init(
        b"WEBVTT\nX-TIMESTAMP-MAP=LOCAL:00:00.000,MPEGTS:90000\n\nSTYLE\n::cue {color:red}\n",
    )
    .unwrap();
    let doc = vtt::parse_with_timeline(
        b"00:00.000 --> 00:02.000\nx",
        Some(&init),
        Timeline {
            media_time: ((1u64 << 33) as f64 / 90_000.0) + 1.0,
            playback_time: 5.0,
        },
        Limits::default(),
    )
    .unwrap();
    assert!((doc.nodes[0].start - 5.0).abs() < 1e-8);
    assert!((doc.nodes[0].end - 7.0).abs() < 1e-8);
    assert_eq!(doc.nodes[1].style["color"].as_ref(), "#ff0000ff");
}
#[test]
fn timeline_rejects_non_finite_references() {
    for media_time in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let timeline = Timeline {
            media_time,
            playback_time: 0.0,
        };
        assert!(vtt::parse_with_timeline(b"WEBVTT", None, timeline, Limits::default()).is_err());
        assert!(imsc::parse_with_timeline(
            br#"<tt xmlns="http://www.w3.org/ns/ttml"/>"#,
            timeline,
            Limits::default()
        )
        .is_err());
    }
}

#[test]
fn css_font_names_do_not_use_ttml_generic_font_rules() {
    let doc = vtt::parse(
        b"WEBVTT\n\nSTYLE\n::cue {font-family:proportionalSansSerif}\n\n00:00.000 --> 00:02.000\nx",
        None,
    )
    .unwrap();
    assert_eq!(
        doc.nodes[1].style["font-family"].as_ref(),
        "proportionalSansSerif"
    );
    let doc = ttml("<body><p tts:fontFamily='proportionalSansSerif'>x</p></body>");
    assert_eq!(text(&doc, "x").style["font-family"].as_ref(), "sans-serif");
}
