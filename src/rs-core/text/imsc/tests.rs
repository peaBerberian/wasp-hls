use super::*;
use crate::text::{Document, Node, Tag};
fn wrapped(content: &str) -> String {
    format!(
        r#"<tt xmlns="http://www.w3.org/ns/ttml" xmlns:tts="http://www.w3.org/ns/ttml#styling" xmlns:ttp="http://www.w3.org/ns/ttml#parameter">{content}</tt>"#
    )
}
fn doc(s: &str) -> Document {
    parse(wrapped(s).as_bytes()).unwrap()
}
fn text<'a>(d: &'a Document, s: &str) -> &'a Node {
    d.nodes
        .iter()
        .find(|n| n.text.as_deref() == Some(s))
        .unwrap()
}
#[test]
fn time_expressions() {
    let r = Rates {
        frame_rate: 25.0,
        tick_rate: 100.0,
        explicit_frame_rate: true,
        explicit_tick_rate: true,
    };
    for (s, v) in [
        ("01:02:03.250", 3723.25),
        ("00:00:02:12", 2.48),
        ("1h", 3600.0),
        ("2.5m", 150.0),
        ("125ms", 0.125),
        ("25f", 1.0),
        ("150t", 1.5),
        ("0.5s", 0.5),
    ] {
        assert!((parse_time(s, r).unwrap() - v).abs() < 1e-8, "{s}");
    }
    for s in [
        "-1s",
        "NaNs",
        "infs",
        "1e2s",
        "00:60:00",
        "00:00:60",
        "1:00:00",
        "0",
        "00:00:01:00.5",
    ] {
        assert!(parse_time(s, r).is_err(), "{s}");
    }
    assert!(parse_time("1f", Rates::default()).is_err());
    assert!(parse_time("1t", Rates::default()).is_err());
}
#[test]
fn malformed_xml_and_references() {
    for body in [
        r#"<head><styling><style xml:id="a" style="b"/><style xml:id="b" style="a"/></styling></head>"#,
        r#"<body><p xml:id="a"/><p xml:id="a"/></body>"#,
        r#"<body region="missing"/>"#,
        r#"<body><p begin="bad"/></body>"#,
        r#"<body><p></body>"#,
        r#"<body xml:space="bad"/>"#,
    ] {
        assert!(parse(wrapped(body).as_bytes()).is_err(), "{body}");
    }
    for s in [
        "<tt/>",
        "<tt xmlns='wrong'/>",
        "<!DOCTYPE tt [<!ENTITY x 'boom'>]><tt/>",
        "<tt/><tt/>",
    ] {
        assert!(parse(s.as_bytes()).is_err(), "{s}");
    }
}
#[test]
fn unsupported_profiles_and_timebases() {
    for attrs in [
        r#"ttp:timeBase="clock""#,
        r#"ttp:profile="http://www.w3.org/ns/ttml/profile/imsc1/image""#,
        r#"ttp:subFrameRate="2""#,
    ] {
        let s = wrapped("").replace("<tt ", &format!("<tt {attrs} "));
        assert!(matches!(parse(s.as_bytes()), Err(Error::Unsupported(_))));
    }
}
#[test]
fn limits_and_foreign_subtrees() {
    let s = wrapped("<body><p><span>x</span></p></body>");
    for limits in [
        Limits {
            bytes: 5,
            ..Limits::default()
        },
        Limits {
            depth: 2,
            ..Limits::default()
        },
        Limits {
            nodes: 2,
            ..Limits::default()
        },
    ] {
        assert!(matches!(
            parse_with_limits(s.as_bytes(), limits),
            Err(Error::Limit(_))
        ));
    }
    let d = doc(
        r#"<body><foreign xmlns="urn:other"><p xmlns="http://www.w3.org/ns/ttml">ignored</p></foreign><p>kept</p></body>"#,
    );
    assert_eq!(
        d.nodes
            .iter()
            .filter_map(|n| n.text.as_deref())
            .collect::<String>(),
        "kept"
    );
    assert_eq!(d.diagnostics.len(), 1);
}

#[test]
fn xml_duplicate_expanded_names_and_forbidden_characters() {
    let s = wrapped(r#"<body xmlns:a="urn:test" xmlns:b="urn:test" a:x="1" b:x="2"/>"#);
    assert!(parse(s.as_bytes()).is_err());
    assert!(parse(wrapped("<body>\0</body>").as_bytes()).is_err());
    assert!(parse(wrapped("<body>\u{b}</body>").as_bytes()).is_err());
}
#[test]
fn deeply_nested_style_graph_is_bounded() {
    let mut styles = String::new();
    for i in 0..20 {
        styles.push_str(&format!(r#"<style xml:id="s{i}" style="s{}"/>"#, i + 1));
    }
    styles.push_str(r#"<style xml:id="s20" tts:color="red"/>"#);
    let xml = wrapped(&format!("<head><styling>{styles}</styling></head>"));
    assert!(matches!(
        parse_with_limits(
            xml.as_bytes(),
            Limits {
                depth: 8,
                ..Limits::default()
            }
        ),
        Err(Error::Limit(_))
    ));
}
#[test]
fn no_panic_on_every_xml_truncation() {
    let source = wrapped("<body><div><p begin='0s' end='1s'>é &amp; x</p></div></body>");
    for n in 0..source.len() {
        let _ = parse(&source.as_bytes()[..n]);
    }
}

#[test]
fn style_combinations_stop_at_the_output_budget() {
    let mut body = format!("<head><styling><style xml:id='a' tts:fontFamily='{}'/><style xml:id='b' tts:fontWeight='bold'/></styling></head><body><div>", "A".repeat(10_000));
    for _ in 0..100 {
        body.push_str("<p style='a b'>x</p>");
    }
    body.push_str("</div></body>");
    assert!(matches!(
        parse_with_limits(
            wrapped(&body).as_bytes(),
            Limits {
                output_bytes: 200_000,
                ..Limits::default()
            }
        ),
        Err(Error::Limit("output bytes"))
    ));
}

#[test]
fn inherited_language_uses_shared_text() {
    let d = doc("<body xml:lang='fr'><div><p>x</p><p>y</p></div></body>");
    let language = text(&d, "x").language.as_ref().unwrap();
    for node in d.nodes.iter().filter(|n| n.tag != Tag::Div) {
        assert!(std::sync::Arc::ptr_eq(
            language,
            node.language.as_ref().unwrap()
        ));
    }
}

#[test]
fn nested_region_styles_follow_the_allocation_budget() {
    let xml = wrapped(&format!(
        "<head><layout><region xml:id='r'><style tts:fontFamily='{}'/></region></layout></head>",
        "A".repeat(10_000)
    ));
    // XML alone fits this allowance. The additional retained style text must
    // also be reserved, rather than slipping through the region reader.
    let plain = xml.replace("tts:fontFamily", "fontFamily");
    let limits = Limits {
        output_bytes: 20_000,
        ..Limits::default()
    };
    assert!(parse_with_limits(plain.as_bytes(), limits).is_ok());
    assert_eq!(
        parse_with_limits(xml.as_bytes(), limits),
        Err(Error::Limit("output bytes"))
    );
}

#[test]
fn duplicate_xml_names_and_namespace_declarations_are_rejected() {
    for attributes in [
        "a='1' a='2'",
        "xml:lang='en' xml:lang='fr'",
        "xmlns='http://www.w3.org/ns/ttml' xmlns='http://www.w3.org/ns/ttml'",
        "xmlns:a='urn:a' xmlns:a='urn:b'",
        "xmlns:a='urn:a' xmlns:a='urn:a'",
        "xmlns:a='urn:shared' xmlns:b='urn:shared' a:x='1' b:x='2'",
    ] {
        assert!(
            matches!(
                parse(wrapped(&format!("<body {attributes}/>")).as_bytes()),
                Err(Error::Invalid(_))
            ),
            "{attributes}"
        );
    }
    // The same raw names on different elements are valid, including namespace
    // rebinding. Equal local names in different namespaces are distinct too.
    assert!(parse(wrapped("<body xmlns:a='urn:a' xmlns:b='urn:b' a:x='1' b:x='2'><div xmlns:a='urn:other' a:x='3'/></body>").as_bytes()).is_ok());
}

#[test]
fn many_unique_xml_attributes_remain_supported() {
    let mut attributes = String::new();
    for i in 0..8000 {
        attributes.push_str(&format!(" a{i:05}='x'"));
    }
    assert!(parse(wrapped(&format!("<body{attributes}/>")).as_bytes()).is_ok());
    // A duplicate near the end must still be detected after all unique names.
    assert!(matches!(
        parse(wrapped(&format!("<body{attributes} a00000='again'/>")).as_bytes()),
        Err(Error::Invalid(_))
    ));
}

#[test]
fn nested_timing_duration_and_clipping() {
    let d = doc(
        r#"<body begin="10s" end="20s"><div begin="1s"><p begin="2s" dur="4s" end="5s">A<span begin="1s" dur="10s">B</span></p></div></body>"#,
    );
    assert_eq!((text(&d, "A").start, text(&d, "A").end), (13.0, 16.0));
    assert_eq!((text(&d, "B").start, text(&d, "B").end), (14.0, 16.0));
}
#[test]
fn generated_parallel_and_sequential_timelines() {
    for base in 0..60 {
        for (container, end) in [("par", 5.0), ("seq", 8.0)] {
            let d = doc(&format!(
                r#"<body begin="{base}s"><div timeContainer="{container}"><p begin="1s" dur="2s">A</p><p begin="2s" dur="3s">B</p></div></body>"#
            ));
            assert_eq!(text(&d, "A").start, base as f64 + 1.0);
            assert_eq!(text(&d, "B").end, base as f64 + end);
        }
    }
}
#[test]
fn explicit_rates_and_multiplier() {
    let xml = wrapped(r#"<body><p begin="30f" end="60f">x</p></body>"#).replace(
        "<tt ",
        r#"<tt ttp:frameRate="30" ttp:frameRateMultiplier="1000 1001" "#,
    );
    let d = parse(xml.as_bytes()).unwrap();
    assert!((text(&d, "x").start - 1.001).abs() < 1e-10);
    assert!((text(&d, "x").end - 2.002).abs() < 1e-10);
}
#[test]
fn indefinite_and_empty_documents() {
    assert!(doc("").nodes.is_empty());
    assert!(doc("<body/>").nodes.is_empty());
    assert_eq!(
        text(&doc("<body><p>forever</p></body>"), "forever").end,
        f64::INFINITY
    );
}
#[test]
fn text_only_spans_and_cdata_keep_container_timing() {
    for content in ["A", "A<![CDATA[B]]>C"] {
        for parent_end in ["", " end='10s'"] {
            let d=doc(&format!("<body{parent_end}><p timeContainer='seq'><span>{content}</span><span begin='1s' dur='1s'>next</span></p></body>"));
            assert_eq!((text(&d, "A").start, text(&d, "A").end), (0.0, 0.0));
            assert_eq!((text(&d, "next").start, text(&d, "next").end), (1.0, 2.0));
        }
    }
    let d = doc("<body><p><span timeContainer='seq'>A<![CDATA[B]]>C</span></p></body>");
    for leaf in d.nodes.iter().filter(|n| n.text.is_some()) {
        assert_eq!(leaf.end, f64::INFINITY);
    }
    let d = doc(
        "<body><p timeContainer='seq'><span dur='2s'>A</span><span dur='1s'>B</span></p></body>",
    );
    assert_eq!((text(&d, "A").end, text(&d, "B").start), (2.0, 2.0));
}
#[test]
fn repeated_and_combined_styles_share_large_strings() {
    for references in ["a", "a b"] {
        let mut body=format!("<head><styling><style xml:id='a' tts:fontFamily='{}'/><style xml:id='b' tts:fontWeight='bold'/></styling></head><body>","A".repeat(10_000));
        for _ in 0..1000 {
            body.push_str(&format!("<p style='{references}'>x</p>"));
        }
        body.push_str("</body>");
        let d = doc(&body);
        let paragraphs: Vec<_> = d.nodes.iter().filter(|n| n.tag == Tag::P).collect();
        assert_eq!(paragraphs.len(), 1000);
        let family = &paragraphs[0].style["font-family"];
        for p in paragraphs {
            assert!(Arc::ptr_eq(family, &p.style["font-family"]));
        }
    }
}
