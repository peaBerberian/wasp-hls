use super::*;
#[test]
fn parents_regions_named_styles_and_relative_fonts_are_resolved() {
    let d=ttml("<head><styling><style xml:id='red' tts:color='red'/><style xml:id='bold' style='red' tts:fontWeight='bold'/></styling><layout><region xml:id='r' tts:fontSize='20px' style='bold'/></layout></head><body tts:fontSize='200%'><p region='r' tts:fontSize='50%'><span>hello</span><span tts:color='yellow'>world</span></p></body>");
    let a = &leaf(&d, "hello").appearance;
    assert_eq!(a.color, [255, 0, 0, 255]);
    assert_eq!(a.font_size, Length::Pixels(20.0));
    assert_eq!(a.font_weight, 700.0);
    assert_eq!(leaf(&d, "world").appearance.color, [255, 255, 0, 255]);
    let d=ttml("<body tts:fontSize='20px'><p tts:lineHeight='150%'><span tts:fontSize='50%'>small</span></p></body>");
    assert_eq!(leaf(&d, "small").appearance.font_size, Length::Pixels(10.0));
    assert_eq!(
        leaf(&d, "small").appearance.line_height,
        LineHeight::Length(Length::Pixels(30.0))
    );
}
#[test]
fn invalid_styles_do_not_override_valid_declarations() {
    let d=ttml("<head><styling><style xml:id='s' tts:fontWeight='bold' tts:color='red' tts:fontSize='12px'/></styling></head><body><p style='s' tts:fontWeight='bogus' tts:color='invalid' tts:fontSize='10px 20px'>kept</p></body>");
    let a = &leaf(&d, "kept").appearance;
    assert_eq!(a.color, [255, 0, 0, 255]);
    assert_eq!(a.font_size, Length::Pixels(12.0));
    assert_eq!(a.font_weight, 700.0);
    let d=vtt("STYLE\n::cue {color:blue !important;color:red;font-weight:bold;font-weight:invalid}\n::cue(.red){color:lime}\n::cue(v[voice=\"Alice\"]){font-style:italic}\n\n00:00.000 --> 00:02.000\n<v Alice><c.red>Hello</c> world</v>");
    assert_eq!(leaf(&d, "Hello").appearance.color, [0, 255, 0, 255]);
    assert_eq!(leaf(&d, " world").appearance.color, [0, 0, 255, 255]);
    assert_eq!(leaf(&d, "Hello").appearance.font_style, FontStyle::Italic);
    assert_eq!(leaf(&d, "Hello").appearance.font_weight, 700.0);
}
#[test]
fn decoration_cancellation_and_font_families_have_small_typed_values() {
    let d=ttml("<body><p tts:textDecoration='underline'><span tts:textDecoration='noUnderline'>plain</span></p></body>");
    assert!(!leaf(&d, "plain").appearance.decoration.underline);
    let d = ttml("<body><p tts:fontFamily='proportionalSansSerif'>x</p></body>");
    let f = leaf(&d, "x").appearance.font_family.as_ref().unwrap();
    assert_eq!(f[0].name, "sans-serif");
    assert!(f[0].generic);
    let d = vtt(
        "STYLE\n::cue {font-family:proportionalSansSerif, 'a,b'}\n\n00:00.000 --> 00:02.000\nx",
    );
    let f = leaf(&d, "x").appearance.font_family.as_ref().unwrap();
    assert_eq!(f[0].name, "proportionalSansSerif");
    assert!(!f[0].generic);
    assert_eq!(f[1].name, "a,b");
}
#[test]
fn ignored_definition_paths_and_style_cycles() {
    let d=ttml("<head><metadata><style xml:id='a' style='a'/><style xml:id='a'/></metadata><styling><style xml:id='a' tts:color='red'/></styling></head><body><p style='a'>x</p></body>");
    assert_eq!(leaf(&d, "x").appearance.color, [255, 0, 0, 255]);
    for s in ["<head><styling><style xml:id='a' style='b'/><style xml:id='b' style='a'/></styling></head>","<head><styling><style xml:id='a'/><style xml:id='a'/></styling></head>"] {assert!(imsc::parse(wrap(s).as_bytes()).is_err());}
}
#[test]
fn repeated_font_lists_share_storage_and_budget_expansion_is_bounded() {
    let font = "A".repeat(10_000);
    let mut s = format!(
        "<head><styling><style xml:id='s' tts:fontFamily='{font}'/></styling></head><body>"
    );
    for _ in 0..1000 {
        s.push_str("<p style='s'>x</p>");
    }
    s.push_str("</body>");
    let d = ttml(&s);
    let first = leaf(&d, "x").appearance.font_family.as_ref().unwrap();
    for n in d.nodes.iter().filter(|n| n.text.is_some()) {
        assert!(std::sync::Arc::ptr_eq(
            first,
            n.appearance.font_family.as_ref().unwrap()
        ));
    }
    let s = format!(
        "<body><p tts:fontFamily='{}'>x</p></body>",
        "A,".repeat(10000) + "B"
    );
    assert!(matches!(
        imsc::parse_with_limits(
            wrap(&s).as_bytes(),
            Limits {
                output_bytes: 200_000,
                ..Limits::default()
            }
        ),
        Err(Error::Limit(_))
    ));
}
#[test]
fn external_css_header_copies_and_grammar_have_limits() {
    let s = format!(
        "WEBVTT\n\nSTYLE\n::cue(.{}){{color:red}}\n",
        "a".repeat(10000)
    );
    let init = vtt::parse_init(s.as_bytes()).unwrap();
    assert!(matches!(
        vtt::parse_with_limits(
            b"00:00.000 --> 00:02.000\nx",
            Some(&init),
            Limits {
                output_bytes: 1000,
                ..Limits::default()
            }
        ),
        Err(Error::Limit(_))
    ));
    let d=vtt("STYLE\n::cue { font-family:'a;/*b*/:c' } @media screen { ::cue {color:red} }\n\n00:00.000 --> 00:02.000\nx");
    assert_eq!(leaf(&d, "x").appearance.color, [255; 4]);
    assert_eq!(
        leaf(&d, "x").appearance.font_family.as_ref().unwrap()[0].name,
        "a;/*b*/:c"
    );
}
#[test]
fn accepted_and_rejected_property_values() {
    values(include_str!("fixtures/appearance-values.txt"));
}

#[test]
fn many_classes_are_checked_before_allocating_their_list() {
    let s = format!(
        "WEBVTT\n\n00:00.000 --> 00:02.000\n<c{}>x</c>",
        ".x".repeat(10000)
    );
    assert!(matches!(
        vtt::parse_with_limits(
            s.as_bytes(),
            None,
            Limits {
                output_bytes: 50000,
                ..Limits::default()
            }
        ),
        Err(Error::Limit(_))
    ));
}

#[test]
fn voice_selectors_match_decoded_annotations() {
    let d=vtt("STYLE\n::cue(v[voice=\"Alice & Bob\"]) {color:red}\n\n00:00.000 --> 00:02.000\n<v Alice &amp; Bob>x</v>");
    assert_eq!(leaf(&d, "x").appearance.color, [255, 0, 0, 255]);
}

#[test]
fn relative_font_weights_use_the_correct_boundaries() {
    for (parent, bolder, lighter) in [
        (1, 400, 1),
        (100, 400, 100),
        (350, 700, 100),
        (550, 900, 400),
        (750, 900, 700),
        (900, 900, 700),
        (950, 950, 700),
    ] {
        for (word, expected) in [("bolder", bolder), ("lighter", lighter)] {
            let d=vtt(&format!("STYLE\n::cue{{font-weight:{parent}}}\n::cue(b){{font-weight:{word}}}\n\n00:00.000 --> 00:02.000\n<b>x</b>"));
            assert_eq!(
                leaf(&d, "x").appearance.font_weight,
                expected as f64,
                "{parent} {word}"
            );
        }
    }
}
