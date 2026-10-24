use super::*;
#[test]
fn both_formats_return_plain_text_with_parents_and_seconds() {
    for d in [
        vtt("00:01.000 --> 00:03.000\nHello <b>world</b>!"),
        ttml("<body><p begin='1s' end='3s'>Hello <span>world</span>!</p></body>"),
    ] {
        assert_eq!(plain(&d), "Hello world!");
        assert!(d.nodes.iter().all(|n| n.start == 1.0 && n.end == 3.0));
        for (i, n) in d.nodes.iter().enumerate() {
            assert!(n.parent.is_none_or(|p| p < i));
        }
    }
}
#[test]
fn whitespace_crosses_spans_and_preservation_survives() {
    let d=ttml("<body><p>\n Hello <span>  world </span>! \n</p><p xml:space='preserve'> A  <span>B </span> C </p><p>A <br/> B</p></body>");
    assert_eq!(plain(&d), "Hello world ! A  B  C A\nB");
}
#[test]
fn xml_entities_cdata_namespaces_and_line_endings() {
    let d=imsc::parse(br#"<x:tt xmlns:x="http://www.w3.org/ns/ttml"><x:body><x:p><![CDATA[A < B]]> &amp; &#x1F642;</x:p></x:body></x:tt>"#).unwrap();
    assert_eq!(plain(&d), "A < B & 🙂");
    let d = ttml("<body><p xml:space='preserve'>A\r\nB\rC</p></body>");
    assert_eq!(plain(&d), "A\nB\nC");
    let d=ttml("<body><foreign xmlns='urn:other'><p xmlns='http://www.w3.org/ns/ttml'>hidden</p></foreign><p>kept</p></body>");
    assert_eq!(plain(&d), "kept");
}
#[test]
fn nested_parallel_and_sequential_times() {
    for base in 0..60 {
        for (container, b, end) in [("par", 2.0, 5.0), ("seq", 5.0, 8.0)] {
            let d=ttml(&format!("<body begin='{base}s'><div timeContainer='{container}'><p begin='1s' dur='2s'>A</p><p begin='2s' dur='3s'>B</p></div></body>"));
            assert_eq!(leaf(&d, "A").start, base as f64 + 1.0);
            assert_eq!(leaf(&d, "B").start, base as f64 + b);
            assert_eq!(leaf(&d, "B").end, base as f64 + end);
        }
    }
    let d=ttml("<body begin='10s' end='20s'><div begin='1s'><p begin='2s' dur='4s' end='5s'>A<span begin='1s' dur='10s'>B</span></p></div></body>");
    assert_eq!((leaf(&d, "A").start, leaf(&d, "A").end), (13.0, 16.0));
    assert_eq!((leaf(&d, "B").start, leaf(&d, "B").end), (14.0, 16.0));
}
#[test]
fn text_only_spans_and_cdata_do_not_sequence_text_storage() {
    for source in ["A", "A<![CDATA[B]]>C"] {
        for parent_end in ["", " end='10s'"] {
            let d=ttml(&format!("<body{parent_end}><p timeContainer='seq'><span>{source}</span><span begin='1s' dur='1s'>next</span></p></body>"));
            assert_eq!((leaf(&d, "A").start, leaf(&d, "A").end), (0.0, 0.0));
            assert_eq!((leaf(&d, "next").start, leaf(&d, "next").end), (1.0, 2.0));
        }
    }
    let d = ttml("<body><p><span timeContainer='seq'>A<![CDATA[B]]>C</span></p></body>");
    assert_eq!(leaf(&d, "A").end, f64::INFINITY);
    assert!(ttml("<body/>").nodes.is_empty());
    assert!(ttml("").nodes.is_empty());
}
#[test]
fn imsc_clock_frame_tick_and_multiplier_rules() {
    let rates = imsc::Rates {
        frame_rate: 25.0,
        tick_rate: 100.0,
        explicit_frame_rate: true,
        explicit_tick_rate: true,
    };
    for (s, n) in [
        ("01:02:03.250", 3723.25),
        ("00:00:02:12", 2.48),
        ("1h", 3600.0),
        ("2.5m", 150.0),
        ("125ms", 0.125),
        ("25f", 1.0),
        ("150t", 1.5),
    ] {
        assert!((imsc::parse_time(s, rates).unwrap() - n).abs() < 1e-8);
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
        assert!(imsc::parse_time(s, rates).is_err(), "{s}");
    }
    assert!(imsc::parse_time("1f", imsc::Rates::default()).is_err());
    assert!(imsc::parse_time("1t", imsc::Rates::default()).is_err());
    let xml = wrap("<body><p begin='30f' end='60f'>x</p></body>").replace(
        "<tt ",
        "<tt ttp:frameRate='30' ttp:frameRateMultiplier='1000 1001' ",
    );
    let d = imsc::parse(xml.as_bytes()).unwrap();
    assert!((leaf(&d, "x").start - 1.001).abs() < 1e-10);
    assert!((leaf(&d, "x").end - 2.002).abs() < 1e-10);
}
#[test]
fn vtt_recovery_and_full_intervals() {
    let d=vtt("bad\n00:00.000 -- > 00:02.000\nlost\n\n00:03.000 --> 00:01.000\nnegative\n00:00.000 --> 00:10.000\nkept");
    assert_eq!(d.diagnostics.len(), 1);
    assert_eq!(leaf(&d, "negative").end, 1.0);
    assert_eq!((leaf(&d, "kept").start, leaf(&d, "kept").end), (0.0, 10.0));
    for ending in ["\n", "\r\n", "\r"] {
        let s=format!("\u{feff}WEBVTT comment{ending}{ending}id{ending}00:01.000 --> 00:02.000{ending}hello{ending}");
        assert_eq!(plain(&vtt::parse(s.as_bytes(), None).unwrap()), "hello");
    }
    for s in ["", "WEBVTTx", "webvtt", "\nWEBVTT"] {
        assert!(vtt::parse(s.as_bytes(), None).is_err());
    }
}
#[test]
fn vtt_html_references_and_unknown_markup() {
    let d=vtt("00:00.000 --> 00:02.000\n&NotEqualTilde; &#x1F642; &amp; &#0; <unknown>kept</unknown><00:01.000>");
    assert_eq!(plain(&d), "≂̸ 🙂 & � kept");
    let d = vtt("00:00.000 --> 00:02.000\n<b>one <i>two</b> three</i>");
    assert_eq!(plain(&d), "one two three");
}
#[test]
fn timeline_mapping_initialization_and_wraparound() {
    let init =
        vtt::parse_init(b"WEBVTT\nX-TIMESTAMP-MAP=LOCAL:00:01.000,MPEGTS:900000\n\n").unwrap();
    let d = vtt::parse_with_timeline(
        b"00:02.000 --> 00:03.000\nx",
        Some(&init),
        Timeline {
            media_time: 10.0,
            playback_time: 3.0,
        },
        Limits::default(),
    )
    .unwrap();
    assert_eq!((leaf(&d, "x").start, leaf(&d, "x").end), (4.0, 5.0));
    let wrap = (1u64 << 33) as f64 / 90_000.0;
    let d = vtt::parse_with_timeline(
        b"WEBVTT\nX-TIMESTAMP-MAP=LOCAL:00:00.000,MPEGTS:90000\n\n00:00.000 --> 00:02.000\nx",
        None,
        Timeline {
            media_time: wrap + 1.0,
            playback_time: 5.0,
        },
        Limits::default(),
    )
    .unwrap();
    assert!((leaf(&d, "x").start - 5.0).abs() < 1e-8);
    for n in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(vtt::parse_with_timeline(
            b"WEBVTT",
            None,
            Timeline {
                media_time: n,
                playback_time: 0.0
            },
            Limits::default()
        )
        .is_err());
    }
}
#[test]
fn mp4_coverage_is_applied_before_rendering() {
    let bytes=br#"<tt xmlns="http://www.w3.org/ns/ttml"><body><p begin="0s" end="100s">x</p></body></tt>"#;
    let sample = mp4::Sample {
        track_id: 1,
        timescale: 1000,
        decode_time: 10000,
        presentation_time: 10000,
        duration: 1000,
        document: bytes,
    };
    let d = imsc::parse_sample(
        &sample,
        Timeline {
            media_time: 10.0,
            playback_time: 3.0,
        },
        Limits::default(),
    )
    .unwrap();
    assert!(d.nodes.iter().all(|n| n.start == 3.0 && n.end == 4.0));
}
#[test]
fn xml_restrictions_remain_explicit() {
    for s in [
        "<tt/>",
        "<tt xmlns='wrong'/>",
        "<!DOCTYPE tt [<!ENTITY x 'boom'>]><tt/>",
        "<tt/><tt/>",
    ] {
        assert!(imsc::parse(s.as_bytes()).is_err(), "{s}");
    }
    for body in [
        "<body><p></body>",
        "<body xml:space='bad'/>",
        "<body>\0</body>",
        "<body>\u{b}</body>",
        "<body xmlns:a='urn:a' xmlns:b='urn:a' a:x='1' b:x='2'/>",
        "<body a='1' a='2'/>",
        "<body xmlns:a='urn:a' xmlns:a='urn:b'/>",
    ] {
        assert!(imsc::parse(wrap(body).as_bytes()).is_err(), "{body}");
    }
    for attr in [
        "ttp:timeBase='clock'",
        "ttp:profile='http://www.w3.org/ns/ttml/profile/imsc1/image'",
        "ttp:subFrameRate='2'",
    ] {
        assert!(imsc::parse(wrap("").replace("<tt ", &format!("<tt {attr} ")).as_bytes()).is_err());
    }
}
#[test]
fn budgets_and_every_truncation() {
    let xml = wrap("<body><p><span>é &amp; x</span></p></body>");
    for limits in [
        Limits {
            bytes: 5,
            ..Limits::default()
        },
        Limits {
            nodes: 2,
            ..Limits::default()
        },
        Limits {
            depth: 2,
            ..Limits::default()
        },
        Limits {
            output_bytes: 100,
            ..Limits::default()
        },
    ] {
        assert!(matches!(
            imsc::parse_with_limits(xml.as_bytes(), limits),
            Err(Error::Limit(_))
        ));
        assert!(matches!(
            vtt::parse_with_limits(
                b"WEBVTT\n\n00:00.000 --> 00:02.000\n<b><i>x</i></b>",
                None,
                limits
            ),
            Err(Error::Limit(_))
        ));
    }
    for n in 0..xml.len() {
        let _ = imsc::parse(&xml.as_bytes()[..n]);
    }
    let v = b"WEBVTT\n\n00:00.000 --> 00:02.000\n<b>one &amp; two</b>";
    for n in 0..v.len() {
        let _ = vtt::parse(&v[..n], None);
    }
    let mut seed = 31u32;
    for size in 0..500 {
        let bytes: Vec<_> = (0..size)
            .map(|_| {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                (seed >> 24) as u8
            })
            .collect();
        let _ = vtt::parse(&bytes, None);
        let _ = imsc::parse(&bytes);
    }
}
#[test]
fn many_xml_attributes_do_not_hide_a_duplicate() {
    let attrs: String = (0..8000).map(|i| format!(" a{i:05}='x'")).collect();
    assert!(imsc::parse(wrap(&format!("<body{attrs}/>")).as_bytes()).is_ok());
    assert!(imsc::parse(wrap(&format!("<body{attrs} a00000='again'/>")).as_bytes()).is_err());
}

#[test]
fn line_breaks_keep_the_paragraphs_time_range() {
    let d = ttml("<body><p begin='1s' end='3s'>A<br/>B</p></body>");
    let br = d.nodes.iter().find(|n| n.kind == Kind::Break).unwrap();
    assert_eq!((br.start, br.end), (1.0, 3.0));
}

#[test]
fn timestamp_wrap_uses_the_cue_time_and_keeps_the_whole_duration() {
    let d = vtt::parse_with_timeline(
        b"WEBVTT\n\n27:46:41.000 --> 27:46:42.000\nx",
        None,
        Timeline {
            media_time: 100000.0,
            playback_time: 0.0,
        },
        Limits::default(),
    )
    .unwrap();
    assert!((leaf(&d, "x").start - 1.0).abs() < 1e-8);
    let d = vtt("00:00.000 --> 100:00:00.000\nx");
    assert_eq!(leaf(&d, "x").end, 360000.0);
    let huge = "1".to_string() + &"0".repeat(308) + "s";
    assert!(imsc::parse_with_timeline(
        wrap(&format!("<body><p begin='{huge}'>x</p></body>")).as_bytes(),
        Timeline {
            media_time: 0.0,
            playback_time: 1e308
        },
        Limits::default()
    )
    .is_err());
}

#[test]
fn empty_preserved_text_does_not_change_spaces_around_it() {
    assert_eq!(
        plain(&ttml(
            "<body><p><span xml:space='preserve'><![CDATA[]]></span> A</p></body>"
        )),
        "A"
    );
    assert!(imsc::parse(format!("\u{feff}{}", wrap("<body><p>x</p></body>")).as_bytes()).is_ok());
}
