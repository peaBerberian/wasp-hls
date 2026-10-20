use super::super::style::{Color, Property, Value};
use super::*;
fn wrapped(content: &str) -> String {
    format!(
        r#"<tt xmlns="http://www.w3.org/ns/ttml" xmlns:tts="http://www.w3.org/ns/ttml#styling" xmlns:ttp="http://www.w3.org/ns/ttml#parameter">{content}</tt>"#
    )
}
fn doc(s: &str) -> Document {
    parse(wrapped(s).as_bytes()).unwrap()
}
fn by_id<'a>(d: &'a Document, id: &str) -> &'a Node {
    d.nodes
        .iter()
        .find(|n| n.id.as_deref() == Some(id))
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
fn namespace_aliases_and_mixed_content() {
    let xml = r#"<x:tt xmlns:x="http://www.w3.org/ns/ttml" xmlns:s="http://www.w3.org/ns/ttml#styling"><x:body><x:div><x:p xml:id="p" begin="1s" end="2s" s:color="red">Hello <x:span>world &amp; all</x:span><x:br/>!</x:p></x:div></x:body></x:tt>"#;
    let d = parse(xml.as_bytes()).unwrap();
    let p = by_id(&d, "p");
    assert_eq!(
        p.interval,
        Interval {
            begin: 1.0,
            end: Some(2.0)
        }
    );
    assert_eq!(
        p.style.properties[&Property::Color],
        Value::Color(Color(255, 0, 0, 255))
    );
    assert_eq!(
        d.nodes
            .iter()
            .filter_map(|n| n.text.as_deref())
            .collect::<String>(),
        "Hello world & all!"
    );
    assert!(d.nodes.iter().any(|n| n.kind == Kind::Break));
}
#[test]
fn nested_timing_duration_and_clipping() {
    let d = doc(
        r#"<body begin="10s" end="20s"><div begin="1s"><p xml:id="p" begin="2s" dur="4s" end="5s">A<span xml:id="s" begin="1s" dur="10s">B</span></p></div></body>"#,
    );
    assert_eq!(
        by_id(&d, "p").interval,
        Interval {
            begin: 13.0,
            end: Some(16.0)
        }
    );
    assert_eq!(
        by_id(&d, "s").interval,
        Interval {
            begin: 14.0,
            end: Some(16.0)
        }
    );
}
#[test]
fn sequential_containers_and_implicit_end() {
    let d = doc(
        r#"<body><div xml:id="d" begin="10s" timeContainer="seq"><p xml:id="a" dur="2s">A</p><p xml:id="b" begin="1s" dur="3s">B</p></div></body>"#,
    );
    assert_eq!(
        by_id(&d, "a").interval,
        Interval {
            begin: 10.0,
            end: Some(12.0)
        }
    );
    assert_eq!(
        by_id(&d, "b").interval,
        Interval {
            begin: 13.0,
            end: Some(16.0)
        }
    );
    assert_eq!(by_id(&d, "d").interval.end, Some(16.0));
}
#[test]
fn indefinite_and_empty_documents() {
    assert!(doc("").nodes.is_empty());
    let d = doc(r#"<body><p xml:id="p">forever</p></body>"#);
    assert_eq!(by_id(&d, "p").interval.end, None);
    assert_eq!(doc("<body/>").nodes[0].interval.end, Some(0.0));
}
#[test]
fn style_chains_precedence_regions_and_extensions() {
    let d = doc(
        r##"<head><styling><style xml:id="a" tts:color="red"/><style xml:id="b" style="a" tts:fontWeight="bold"/><style xml:id="c" tts:color="blue"/></styling><layout><region xml:id="r" tts:origin="10% 20%" tts:extent="80% 70%"/></layout></head><body region="r"><p xml:id="p" style="b c" tts:color="#00ff0080" xmlns:e="urn:extension" e:effect="glow" begin="1s" end="2s">x</p></body>"##,
    );
    let p = by_id(&d, "p");
    assert_eq!(p.region.as_deref(), Some("r"));
    assert_eq!(
        p.style.properties[&Property::Color],
        Value::Color(Color(0, 255, 0, 128))
    );
    assert_eq!(
        p.style.properties[&Property::FontWeight],
        Value::Keyword("bold".into())
    );
    assert_eq!(p.style.unsupported[0].namespace.as_ref(), "urn:extension");
    assert!(matches!(
        d.regions["r"].style.properties[&Property::Origin],
        Value::Lengths(_)
    ));
}
#[test]
fn preserves_whitespace_and_inherited_metadata() {
    let d = doc(
        r#"<body xml:lang="fr" xml:space="preserve"><p xml:id="p" begin="0s" end="1s"> a  <span xml:id="s" xml:space="default"> b </span> c </p></body>"#,
    );
    assert_eq!(by_id(&d, "p").space, Space::Preserve);
    assert_eq!(by_id(&d, "s").space, Space::Default);
    assert_eq!(by_id(&d, "s").language.as_deref(), Some("fr"));
    assert_eq!(
        d.nodes
            .iter()
            .filter_map(|n| n.text.as_deref())
            .collect::<String>(),
        " a   b  c "
    );
}
#[test]
fn explicit_rates_and_multiplier() {
    let s = wrapped(r#"<body><p begin="30f" end="60f">x</p></body>"#).replace(
        "<tt ",
        r#"<tt ttp:frameRate="30" ttp:frameRateMultiplier="1000 1001" "#,
    );
    let d = parse(s.as_bytes()).unwrap();
    let p = d.nodes.iter().find(|n| n.kind == Kind::Paragraph).unwrap();
    assert!((p.interval.begin - 1.001).abs() < 1e-10);
    assert!((p.interval.end.unwrap() - 2.002).abs() < 1e-10);
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
fn missing_style_reference_is_recoverable() {
    let d = doc(r#"<body><div><p xml:id="p" style="missing" tts:color="red">x</p></div></body>"#);
    assert_eq!(d.diagnostics.len(), 1);
    assert_eq!(
        by_id(&d, "p").style.properties[&Property::Color],
        Value::Color(Color(255, 0, 0, 255))
    );
}

#[test]
fn anonymous_nested_styles_and_forward_references() {
    let d = doc(
        r#"<head><styling><style xml:id="a" style="b"><style tts:fontWeight="bold"/></style><style xml:id="b" tts:color="blue"/></styling></head><body><div><p xml:id="p" style="a">x</p></div></body>"#,
    );
    let p = by_id(&d, "p");
    assert_eq!(
        p.style.properties[&Property::Color],
        Value::Color(Color(0, 0, 255, 255))
    );
    assert_eq!(
        p.style.properties[&Property::FontWeight],
        Value::Keyword("bold".into())
    );
}
#[test]
fn xml_duplicate_expanded_names_and_forbidden_characters() {
    let s = wrapped(r#"<body xmlns:a="urn:test" xmlns:b="urn:test" a:x="1" b:x="2"/>"#);
    assert!(parse(s.as_bytes()).is_err());
    assert!(parse(wrapped("<body>\0</body>").as_bytes()).is_err());
    assert!(parse(wrapped("<body>\u{b}</body>").as_bytes()).is_err());
}
#[test]
fn cdata_and_xml_line_endings() {
    let d = doc("<body><div><p><![CDATA[A < B]]>\r\nC\rD</p></div></body>");
    assert_eq!(
        d.nodes
            .iter()
            .filter_map(|n| n.text.as_deref())
            .collect::<String>(),
        "A < B\nC\nD"
    );
}
#[test]
fn generated_parallel_and_sequential_timelines() {
    for base in 0..60 {
        for (container, end) in [("par", 5.0), ("seq", 8.0)] {
            let d = doc(&format!(
                r#"<body begin="{base}s"><div timeContainer="{container}"><p xml:id="a" begin="1s" dur="2s">A</p><p xml:id="b" begin="2s" dur="3s">B</p></div></body>"#
            ));
            assert_eq!(by_id(&d, "a").interval.begin, base as f64 + 1.0);
            assert_eq!(by_id(&d, "b").interval.end, Some(base as f64 + end));
        }
    }
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
fn repeated_style_references_share_storage() {
    let mut xml = wrapped(&format!(
        "<head><styling><style xml:id='shared' tts:fontFamily='{}'/></styling></head><body><div>",
        "A".repeat(10_000)
    ));
    xml.truncate(xml.len() - "</tt>".len());
    for _ in 0..1000 {
        xml.push_str("<p style='shared'>x</p>");
    }
    xml.push_str("</div></body></tt>");
    let d = parse_with_limits(
        xml.as_bytes(),
        Limits {
            output_bytes: 8 * 1024 * 1024,
            ..Limits::default()
        },
    )
    .unwrap();
    let named = &d.styles["shared"];
    let paragraphs: Vec<_> = d
        .nodes
        .iter()
        .filter(|n| n.kind == Kind::Paragraph)
        .collect();
    assert_eq!(paragraphs.len(), 1000);
    for node in paragraphs {
        assert!(std::sync::Arc::ptr_eq(&node.style, named));
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
fn foreign_and_metadata_subtrees_cannot_define_styles_or_regions() {
    let xml = wrapped(
        r#"<head><metadata><style xml:id="hidden-meta" tts:color="red"/><region xml:id="hidden-region"/></metadata><foreign xmlns="urn:other"><style xmlns="http://www.w3.org/ns/ttml" xml:id="hidden-foreign" tts:color="red"/></foreign><styling><style xml:id="visible" tts:color="blue"/></styling></head><body><div><p xml:id="p" style="hidden-meta hidden-foreign visible">x</p></div></body>"#,
    );
    let d = parse(xml.as_bytes()).unwrap();
    assert_eq!(d.styles.len(), 1);
    assert!(d.regions.is_empty());
    assert_eq!(
        by_id(&d, "p").style.properties[&Property::Color],
        Value::Color(Color(0, 0, 255, 255))
    );
    assert_eq!(d.diagnostics.len(), 2);
}

#[test]
fn ignored_definitions_do_not_trigger_cycles_or_duplicate_ids() {
    let d = doc(
        r#"<head><metadata><style xml:id="a" style="a"/><style xml:id="a"/></metadata><styling><style xml:id="a" tts:color="red"/></styling></head><body><div><p style="a">x</p></div></body>"#,
    );
    assert_eq!(d.styles.len(), 1);
}

#[test]
fn combined_styles_share_large_text_values() {
    let d = doc(&format!("<head><styling><style xml:id='a' tts:fontFamily='{}'/><style xml:id='b' tts:fontWeight='bold'/></styling></head><body><div><p style='a b'>x</p><p style='a b'>y</p></div></body>", "A".repeat(10_000)));
    let values: Vec<_> = d
        .nodes
        .iter()
        .filter(|n| n.kind == Kind::Paragraph)
        .map(|n| match &n.style.properties[&Property::FontFamily] {
            Value::Text(text) => text,
            _ => panic!("expected a font-family string"),
        })
        .collect();
    assert!(std::sync::Arc::ptr_eq(values[0], values[1]));
}

#[test]
fn inherited_language_uses_shared_text() {
    let d = doc("<body xml:lang='fr'><div><p>x</p><p>y</p></div></body>");
    let language = d.nodes[0].language.as_ref().unwrap();
    for node in &d.nodes {
        assert!(std::sync::Arc::ptr_eq(
            language,
            node.language.as_ref().unwrap()
        ));
    }
}

#[test]
fn invalid_inline_values_keep_source_and_do_not_replace_valid_references() {
    let d = doc(
        r#"<head><styling><style xml:id="base" tts:fontWeight="bold" tts:fontSize="12px"/></styling></head><body><div><p xml:id="p" style="base" tts:fontWeight="bogus" tts:fontSize="10px 20px">Still visible</p></div></body>"#,
    );
    let style = &by_id(&d, "p").style;
    assert_eq!(
        style.properties[&Property::FontWeight],
        Value::Keyword("bold".into())
    );
    assert_eq!(
        style.properties[&Property::FontSize],
        Value::Lengths(vec![super::super::style::Length {
            value: 12.0,
            unit: super::super::style::Unit::Pixel
        }])
    );
    assert_eq!(style.unsupported.len(), 2);
    assert_eq!(style.unsupported[0].name.as_ref(), "fontWeight");
    assert_eq!(style.unsupported[0].value.as_ref(), "bogus");
    assert_eq!(style.unsupported[1].value.as_ref(), "10px 20px");
    assert_eq!(
        style.unsupported[1].namespace.as_ref(),
        "http://www.w3.org/ns/ttml#styling"
    );
    assert!(d
        .nodes
        .iter()
        .any(|n| n.text.as_deref() == Some("Still visible")));
}

#[test]
fn text_only_spans_use_the_parent_container_for_implicit_duration() {
    for text in ["A", "A<![CDATA[B]]>C"] {
        for parent_end in ["", " end='10s'"] {
            let d = doc(&format!("<body{parent_end}><div><p xml:id='p' timeContainer='seq'><span xml:id='a'>{text}</span><span xml:id='b' begin='1s' dur='1s'>B</span></p></div></body>"));
            assert_eq!(
                by_id(&d, "a").interval,
                Interval {
                    begin: 0.0,
                    end: Some(0.0)
                }
            );
            assert_eq!(
                by_id(&d, "b").interval,
                Interval {
                    begin: 1.0,
                    end: Some(2.0)
                }
            );
            assert_eq!(by_id(&d, "p").interval.end, Some(2.0));
            for &child in &by_id(&d, "a").children {
                assert_eq!(d.nodes[child].interval.end, Some(0.0));
            }
        }
    }
    let d = doc("<body><p><span xml:id='a'>A</span></p></body>");
    assert_eq!(by_id(&d, "a").interval.end, None);
    let d = doc("<body><p timeContainer='seq'><span xml:id='a' dur='2s'>A</span><span xml:id='b' dur='1s'>B</span></p></body>");
    assert_eq!(by_id(&d, "a").interval.end, Some(2.0));
    assert_eq!(by_id(&d, "b").interval.begin, 2.0);
}

#[test]
fn region_nested_styles_keep_precedence_and_stay_local() {
    let d = doc(
        r#"<head><styling><style xml:id="base" tts:color="blue" tts:fontWeight="normal"/></styling><layout><region xml:id="r" style="base" tts:fontWeight="bold"><style xml:id="local" tts:color="red" tts:extent="80% 20%"/><style tts:color="green" tts:origin="10% 80%" tts:fontWeight="normal"/><metadata><style tts:fontStyle="italic"/></metadata></region></layout></head><body><p xml:id="p" region="r" style="local" begin="1s" end="2s">Hello</p></body>"#,
    );
    let style = &d.regions["r"].style;
    // TTML1 8.4.4.2 merges nested styles in source order. imscJS currently
    // keeps the first value instead; the expected value here follows TTML.
    assert_eq!(
        style.properties[&Property::Color],
        Value::Color(Color(0, 128, 0, 255))
    );
    assert_eq!(
        style.properties[&Property::FontWeight],
        Value::Keyword("bold".into())
    );
    for property in [Property::Origin, Property::Extent] {
        assert!(style.properties.contains_key(&property));
    }
    assert!(!style.properties.contains_key(&Property::FontStyle));
    assert!(!d.styles.contains_key("local"));
    assert!(by_id(&d, "p").style.properties.is_empty());
    assert!(d
        .diagnostics
        .iter()
        .any(|d| d.message.contains("unknown style reference local")));
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
fn text_only_span_does_not_sequence_its_internal_text_storage() {
    let d = doc("<body><p><span xml:id='a' timeContainer='seq'>A<![CDATA[B]]>C</span></p></body>");
    let span = by_id(&d, "a");
    assert_eq!(span.interval.end, None);
    for &child in &span.children {
        assert_eq!(d.nodes[child].interval, span.interval);
    }
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
