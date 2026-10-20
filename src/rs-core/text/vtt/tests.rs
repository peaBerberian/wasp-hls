use super::*;
use crate::text::{Error, Limits};
fn document(body: &str) -> Document {
    parse(format!("WEBVTT\n\n{body}").as_bytes(), None).unwrap()
}

#[test]
fn timestamps() {
    for (input, expected) in [
        ("00:00.000", Some(0)),
        ("01:02.345", Some(62345)),
        ("12:34:56.789", Some(45296789)),
        ("100:00:00.000", Some(360000000)),
        ("60:00.000", None),
        ("0:01.000", None),
        ("00:60.000", None),
        ("00:01.00", None),
        ("+00:01.000", None),
        ("00:00:01.0000", None),
        ("00:00:00.NaN", None),
        ("999999999999999999999999:00:00.000", None),
    ] {
        assert_eq!(timestamp(input), expected, "{input}");
    }
}
#[test]
fn headers_and_line_endings() {
    for ending in ["\n", "\r\n", "\r"] {
        let s=format!("\u{feff}WEBVTT comment{ending}{ending}id{ending}00:01.000 --> 00:02.000{ending}hello{ending}");
        let d = parse(s.as_bytes(), None).unwrap();
        assert_eq!(d.cues.len(), 1);
        assert_eq!(d.cues[0].id, "id");
        assert_eq!(d.cues[0].raw_text, "hello");
    }
    for bad in ["", "WEBVTTx\n", "webvtt\n", "\nWEBVTT"] {
        assert!(parse(bad.as_bytes(), None).is_err(), "{bad}");
    }
    assert!(parse(b"WEBVTT", None).unwrap().cues.is_empty());
}
#[test]
fn empty_segments_and_external_header() {
    let h = parse(
        b"WEBVTT\nX-TIMESTAMP-MAP=MPEGTS:900000,LOCAL:00:01.000\n\n",
        None,
    )
    .unwrap()
    .header;
    assert!(parse(b"", Some(&h)).unwrap().cues.is_empty());
    let d = parse(b"00:02.000 --> 00:03.000\nhello", Some(&h)).unwrap();
    assert_eq!(
        d.header.timestamp_map,
        TimestampMap {
            local_ms: 1000,
            mpegts: 900000
        }
    );
    assert_eq!(d.cues[0].start_ms, 2000);
    assert_eq!(d.cues[0].end_ms, 3000);
    assert_eq!(
        parse(b"WEBVTT\n\n", Some(&h)).unwrap().header.timestamp_map,
        TimestampMap::default()
    );
}
#[test]
fn maps_and_wraparound() {
    let m = TimestampMap {
        local_ms: 1000,
        mpegts: 900000,
    };
    assert_eq!(m.map_seconds(2000, 900000), 11.0);
    let m = TimestampMap {
        local_ms: 0,
        mpegts: 90_000,
    };
    assert!(
        (m.map_seconds(0, (1 << 33) + 90_000) - ((1u64 << 33) as f64 / 90000.0 + 1.0)).abs() < 1e-8
    );
    assert_eq!(
        TimestampMap {
            local_ms: 0,
            mpegts: (1 << 33) - 90000
        }
        .map_seconds(0, 0),
        -1.0
    );
    for bad in [
        "LOCAL:00:00.000",
        "LOCAL:00:00.000,MPEGTS:8589934592",
        "LOCAL:00:00.000,MPEGTS:0,MPEGTS:1",
        "LOCAL:bad,MPEGTS:0",
    ] {
        assert!(parse(
            format!("WEBVTT\nX-TIMESTAMP-MAP={bad}\n\n").as_bytes(),
            None
        )
        .is_err());
    }
}
#[test]
fn settings_and_last_valid_duplicate() {
    let d=document("00:00.000 --> 00:01.000 line:bad line:-2,center line:8 position:25%,line-right size:101% size:70% align:wrong align:end vertical:rl region:r\nx");
    let s = &d.cues[0].settings;
    assert_eq!(s.line, Line::Number(8.0));
    assert_eq!(s.line_align, LineAlign::Center);
    assert_eq!(s.position, Some(25.0));
    assert_eq!(s.position_align, PositionAlign::LineRight);
    assert_eq!(s.size, 70.0);
    assert_eq!(s.align, Align::End);
    assert_eq!(s.vertical, Vertical::Rl);
    assert_eq!(s.region, None);
    for value in ["+20%", "-2%", "NaN%", "1e1%", "101%"] {
        let d = document(&format!(
            "00:00.000 --> 00:01.000 position:{value} size:{value}\nx"
        ));
        assert_eq!(d.cues[0].settings.position, None);
        assert_eq!(d.cues[0].settings.size, 100.0);
    }
}
#[test]
fn preserves_intervals_and_order() {
    let d=document("00:08.000 --> 00:30.000\nlong\n\n00:01.000 --> 00:02.000\nearly\n\n00:04.000 --> 00:04.000\nzero\n\n00:06.000 --> 00:05.000\nreverse");
    assert_eq!(
        d.cues
            .iter()
            .map(|c| (c.start_ms, c.end_ms))
            .collect::<Vec<_>>(),
        vec![(8000, 30000), (1000, 2000), (4000, 4000), (6000, 5000)]
    );
}
#[test]
fn malformed_recovery_and_missing_separators() {
    let d=document("NOTE comment\nignored\n\nbad\n00:xx.000 --> 00:01.000\nignored\n\n00:01.000 --> 00:02.000\nfirst\n00:03.000 --> 00:04.000\nsecond");
    assert_eq!(d.cues.len(), 2);
    assert_eq!(d.diagnostics.len(), 1);
    assert_eq!(d.cues[0].raw_text, "first");
    assert_eq!(d.cues[1].raw_text, "second");
}
#[test]
fn region_blocks_and_styles() {
    let d=document("REGION\nid:r\nwidth:50%\nlines:4\nregionanchor:10%,20%\nviewportanchor:30%,40%\nscroll:up\n\nSTYLE\n::cue(.red) { color: red }\n\n00:01.000 --> 00:02.000 region:r\nx\n\nSTYLE\n::cue { color: blue }");
    assert_eq!(
        d.header.regions[0],
        Region {
            id: "r".into(),
            width: 50.0,
            lines: 4,
            region_anchor: (10.0, 20.0),
            viewport_anchor: (30.0, 40.0),
            scroll_up: true
        }
    );
    assert_eq!(d.header.style_sheets.len(), 1);
    assert_eq!(d.diagnostics.len(), 1);
}
#[test]
fn markup_is_a_portable_arena() {
    let d=document("00:00.000 --> 00:05.000\n<v.speaker Alice><b>A &amp; B</b><i>é</i></v><ruby>字<rt>ji</rt></ruby><00:02.000><lang fr>oui</lang>");
    let n = &d.cues[0].text;
    assert_eq!(
        n[1],
        Node {
            parent: Some(0),
            kind: NodeKind::Element {
                tag: Tag::Voice,
                classes: vec!["speaker".into()],
                annotation: "Alice".into()
            }
        }
    );
    assert!(n.iter().any(|n| n.kind == NodeKind::Text("A & B".into())));
    assert!(n.iter().any(|n| n.kind == NodeKind::Timestamp(2000)));
    for (i, node) in n.iter().enumerate().skip(1) {
        assert!(node.parent.unwrap() < i);
    }
}
#[test]
fn markup_recovery_entities_and_replacement() {
    let d=document("00:00.000 --> 00:01.000\n<unknown>A</unknown><rt>B</rt>&lt;&gt;&nbsp;&lrm;&rlm;&#65;&unknown;\0");
    let text = d.cues[0]
        .text
        .iter()
        .filter_map(|n| {
            if let NodeKind::Text(t) = &n.kind {
                Some(t.as_str())
            } else {
                None
            }
        })
        .collect::<String>();
    assert_eq!(text, "AB<>\u{a0}\u{200e}\u{200f}A&unknown;\u{fffd}");
    let d = parse(b"WEBVTT\n\n00:00.000 --> 00:01.000\n\xff", None).unwrap();
    assert_eq!(d.cues[0].raw_text, "\u{fffd}");
}
#[test]
fn bounded_input() {
    let bytes = b"WEBVTT\n\n00:00.000 --> 00:01.000\n<b><b>x";
    assert!(matches!(
        parse_with_limits(
            bytes,
            None,
            Limits {
                bytes: 5,
                ..Limits::default()
            }
        ),
        Err(Error::Limit(_))
    ));
    assert!(matches!(
        parse_with_limits(
            bytes,
            None,
            Limits {
                depth: 2,
                ..Limits::default()
            }
        ),
        Err(Error::Limit(_))
    ));
    assert!(matches!(
        parse_with_limits(
            bytes,
            None,
            Limits {
                nodes: 2,
                ..Limits::default()
            }
        ),
        Err(Error::Limit(_))
    ));
}

#[test]
fn generated_clock_matrix() {
    for h in [0u64, 1, 12, 100] {
        for m in [0u64, 1, 59] {
            for s in [0u64, 1, 59] {
                for ms in [0u64, 1, 999] {
                    let source = format!("{h:02}:{m:02}:{s:02}.{ms:03}");
                    assert_eq!(
                        timestamp(&source),
                        Some(((h * 60 + m) * 60 + s) * 1000 + ms)
                    );
                }
            }
        }
    }
}
#[test]
fn fractional_lines_latest_settings_and_region_order() {
    let d=document("REGION\nid:r width:40%\n\n00:00.000 --> 00:01.000 region:r line:-1.5 align:start align:end\nx\n\n00:01.000 --> 00:02.000 line:4 region:r\ny");
    assert_eq!(d.header.regions[0].width, 40.0);
    assert_eq!(d.cues[0].settings.line, Line::Number(-1.5));
    assert_eq!(d.cues[0].settings.align, Align::End);
    assert_eq!(d.cues[0].settings.region, None);
    assert_eq!(d.cues[1].settings.region.as_deref(), Some("r"));
}
#[test]
fn settings_touching_end_timestamp_and_single_digit_hours() {
    let d = document("0:00:00.000 --> 0:00:01.000line:40% size:40%\nx");
    assert_eq!(d.cues.len(), 1);
    assert_eq!(d.cues[0].settings.line, Line::Percent(40.0));
    assert_eq!(d.cues[0].settings.size, 40.0);
}
#[test]
fn html_character_references_and_numeric_recovery() {
    let d=document("00:00.000 --> 00:01.000\n&copy; &NotEqualTilde; &#65; &#x1f600; &#0; &#xD800; &#x110000; &#128; &amp no-semicolon");
    let text = d.cues[0]
        .text
        .iter()
        .filter_map(|n| {
            if let NodeKind::Text(t) = &n.kind {
                Some(t.as_str())
            } else {
                None
            }
        })
        .collect::<String>();
    assert_eq!(text, "© ≂\u{338} A 😀 � � � € & no-semicolon");
}
#[test]
fn no_panic_on_arbitrary_bytes_and_truncation() {
    let bytes = b"WEBVTT\n\n00:00.000 --> 00:01.000\n<ruby>x<rt>y</rt></ruby>&amp;";
    for n in 0..=bytes.len() {
        let _ = parse(&bytes[..n], None);
    }
    for n in 0..=255 {
        let input = [n; 128];
        let _ = parse(&input, None);
    }
}

#[test]
fn recovery_and_timestamp_differences_from_legacy_parsers() {
    let d = parse(b"WEBVTT\n00:11.000 --> 00:13.000\nTest", None).unwrap();
    assert_eq!(d.cues.len(), 1);
    for clock in ["000:00.000", "120:00.000"] {
        assert!(timestamp(clock).is_none());
    }
    let d = document("-->\n00:04.000 --> 00:05.000\nTest");
    assert_eq!(d.cues.len(), 1);
    assert!(!d.diagnostics.is_empty());
    for alignment in ["middle", "unknown"] {
        let d = document(&format!("00:00.000 --> 00:01.000 line:42,{alignment}\nx"));
        assert_eq!(d.cues[0].settings.line, Line::Auto);
    }
}

#[test]
fn invalid_region_scroll_does_not_replace_a_valid_value() {
    let d = document("REGION\nid:r scroll:up scroll:invalid\n\n");
    assert!(d.header.regions[0].scroll_up);
}

#[test]
fn external_header_copies_follow_the_output_budget() {
    let mut header = Header::default();
    header.style_sheets.push(css::Stylesheet {
        source: "x".repeat(10_000),
        ..Default::default()
    });
    let limits = Limits {
        output_bytes: 1000,
        ..Limits::default()
    };
    assert!(matches!(
        parse_with_limits(b"", Some(&header), limits),
        Err(Error::Limit("output bytes"))
    ));
    // A segment header replaces the external header, so no copy is needed.
    assert!(parse_with_limits(b"WEBVTT\n\n", Some(&header), limits).is_ok());
}

#[test]
fn invalid_and_unsupported_css_values_keep_their_raw_declarations() {
    let d = document("STYLE\n::cue { font-weight:bold; font-weight:Bogus; font-size:10px 20px; font-size:calc(1em + 1px); future:keep-me; }\n\n00:00.000 --> 00:01.000\nStill visible");
    let declarations = &d.header.style_sheets[0].rules[0].declarations;
    assert_eq!(declarations.len(), 5);
    assert!(declarations[0].portable.is_some());
    for declaration in &declarations[1..] {
        assert!(declaration.portable.is_none());
    }
    assert_eq!(declarations[1].value, "Bogus");
    assert_eq!(declarations[2].value, "10px 20px");
    assert_eq!(declarations[3].value, "calc(1em + 1px)");
    assert_eq!(declarations[4].name, "future");
    assert_eq!(declarations[4].value, "keep-me");
    assert_eq!(d.cues[0].raw_text, "Still visible");
}

#[test]
fn region_and_cue_ids_follow_the_allocation_budget() {
    let large = "a".repeat(10_000);
    let limits = Limits {
        output_bytes: 2000,
        ..Limits::default()
    };
    for source in [
        format!("WEBVTT\nRegion: id={large}\n\n"),
        format!("WEBVTT\n\nREGION\nid:{large}\n\n"),
        format!("WEBVTT\n\n{large}\n00:00.000 --> 00:01.000\nx"),
        format!("WEBVTT\n\nREGION\nid:{large} id:small\n\n"),
    ] {
        assert!(parse(source.as_bytes(), None).is_ok());
        assert_eq!(
            parse_with_limits(source.as_bytes(), None, limits),
            Err(Error::Limit("output bytes"))
        );
    }
}

#[test]
fn region_reference_copies_are_budgeted() {
    let header = Header {
        regions: vec![Region {
            id: "a".repeat(10_000),
            ..Region::default()
        }],
        ..Header::default()
    };
    let source = format!("00:00.000 --> 00:01.000 region:{}\nx", header.regions[0].id);
    let limits = Limits {
        output_bytes: 25_000,
        ..Limits::default()
    };
    assert!(parse_with_limits(b"", Some(&header), limits).is_ok());
    assert_eq!(
        parse_with_limits(source.as_bytes(), Some(&header), limits),
        Err(Error::Limit("output bytes"))
    );
}

#[test]
fn retained_diagnostics_follow_the_allocation_budget() {
    let source = format!("WEBVTT\n\n{}", "id\ninvalid timings\ntext\n\n".repeat(20));
    assert_eq!(
        parse(source.as_bytes(), None).unwrap().diagnostics.len(),
        20
    );
    assert_eq!(
        parse_with_limits(
            source.as_bytes(),
            None,
            Limits {
                output_bytes: 4000,
                ..Limits::default()
            }
        ),
        Err(Error::Limit("output bytes"))
    );
    let source = format!(
        "WEBVTT\n\n00:00.000 --> 00:01.000\nx\n\n{}",
        "STYLE\n::cue {}\n\n".repeat(20)
    );
    assert_eq!(
        parse(source.as_bytes(), None).unwrap().diagnostics.len(),
        20
    );
    assert_eq!(
        parse_with_limits(
            source.as_bytes(),
            None,
            Limits {
                output_bytes: 4000,
                ..Limits::default()
            }
        ),
        Err(Error::Limit("output bytes"))
    );
}

#[test]
fn region_lookup_handles_all_header_sources_and_recovery() {
    let header = Header {
        regions: vec![Region {
            id: "external".into(),
            ..Region::default()
        }],
        ..Header::default()
    };
    let d = parse(b"00:00.000 --> 00:01.000 region:external\nx", Some(&header)).unwrap();
    assert_eq!(d.cues[0].settings.region.as_deref(), Some("external"));
    let source = b"WEBVTT\nRegion: id:ignored id=legacy\n\nREGION\nid:modern\n\nbad\ninvalid timings\n\nREGION\nid:later\n\n00:00.000 --> 00:01.000 region:legacy\na\n\n00:01.000 --> 00:02.000 region:modern\nb\n\n00:02.000 --> 00:03.000 region:later\nc\n\n00:03.000 --> 00:04.000 region:external\nd\n\n00:04.000 --> 00:05.000 region:missing\ne";
    let d = parse(source, Some(&header)).unwrap();
    assert_eq!(
        d.cues
            .iter()
            .map(|c| c.settings.region.as_deref())
            .collect::<Vec<_>>(),
        vec![Some("legacy"), Some("modern"), Some("later"), None, None]
    );
}
