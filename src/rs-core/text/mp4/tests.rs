use super::*;
fn bx(name: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut b = ((data.len() + 8) as u32).to_be_bytes().to_vec();
    b.extend(name);
    b.extend(data);
    b
}
fn full(name: &[u8; 4], version: u8, flags: u32, data: &[u8]) -> Vec<u8> {
    let mut b = ((u32::from(version) << 24) | flags).to_be_bytes().to_vec();
    b.extend(data);
    bx(name, &b)
}
fn words(w: &[u32]) -> Vec<u8> {
    w.iter().flat_map(|n| n.to_be_bytes()).collect()
}
fn track() -> Track {
    Track {
        id: 1,
        timescale: 1000,
        namespace: "http://www.w3.org/ns/ttml".into(),
        schema_location: String::new(),
        sample_description_index: 1,
        default_duration: 1000,
        default_size: 0,
    }
}
fn init() -> Vec<u8> {
    let mut entry = vec![0; 6];
    entry.extend([0, 1]);
    entry.extend(b"http://www.w3.org/ns/ttml\0\0\0");
    let mut stsd = words(&[1]);
    stsd.extend(bx(b"stpp", &entry));
    let mdia = [
        full(b"mdhd", 0, 0, &words(&[0, 0, 1000, 0])),
        bx(b"minf", &bx(b"stbl", &full(b"stsd", 0, 0, &stsd))),
    ]
    .concat();
    let trak = [
        full(b"tkhd", 0, 3, &words(&[0, 0, 1, 0])),
        bx(b"mdia", &mdia),
    ]
    .concat();
    bx(
        b"moov",
        &[
            bx(b"trak", &trak),
            bx(b"mvex", &full(b"trex", 0, 0, &words(&[1, 1, 1000, 0, 0]))),
        ]
        .concat(),
    )
}
fn segment(offset: i32, version: u8) -> Vec<u8> {
    let tfhd = full(b"tfhd", 0, 0x020000, &words(&[1]));
    let tfdt = full(b"tfdt", 1, 0, &10000u64.to_be_bytes());
    let make = |data_offset: i32| {
        bx(
            b"moof",
            &bx(
                b"traf",
                &[
                    tfhd.clone(),
                    tfdt.clone(),
                    full(
                        b"trun",
                        version,
                        0xb01,
                        &words(&[2, data_offset as u32, 1000, 3, offset as u32, 2000, 4, 0]),
                    ),
                ]
                .concat(),
            ),
        )
    };
    let size = make(0).len();
    [make((size + 8) as i32), bx(b"mdat", b"oneTWO!")].concat()
}
#[test]
fn extracts_init_metadata() {
    let d = parse_init(&init()).unwrap();
    assert_eq!(d.tracks, vec![track()]);
}
#[test]
fn individual_samples_and_signed_composition_offsets() {
    let bytes = segment(-500, 1);
    let s = extract(&bytes, &track(), 0).unwrap();
    assert_eq!(s.len(), 2);
    assert_eq!(s[0].document, b"one");
    assert_eq!(s[1].document, b"TWO!");
    assert_eq!(
        (s[0].decode_time, s[0].presentation_time, s[0].duration),
        (10000, 9500, 1000)
    );
    assert_eq!(
        (s[1].decode_time, s[1].presentation_time, s[1].duration),
        (11000, 11000, 2000)
    );
    assert_eq!(s[0].timescale, 1000);
}
#[test]
fn unsigned_composition_offset() {
    let bytes = segment(-1, 0);
    let s = extract(&bytes, &track(), 0).unwrap();
    assert_eq!(s[0].presentation_time, 10000 + u32::MAX as i128);
}
#[test]
fn empty_fragment_and_unknown_boxes() {
    assert!(extract(&bx(b"free", b"unused"), &track(), 0).is_err());
    let bytes = [bx(b"free", b"unused"), segment(0, 1)].concat();
    assert_eq!(extract(&bytes, &track(), 0).unwrap().len(), 2);
}
#[test]
fn every_truncation_and_corrupt_sample_sizes() {
    let bytes = segment(0, 1);
    for n in 1..bytes.len() {
        assert!(extract(&bytes[..n], &track(), 0).is_err(), "prefix {n}");
    }
    let mut bad = bytes.clone();
    let at = bad.windows(4).position(|b| b == b"trun").unwrap();
    bad[at + 20..at + 24].copy_from_slice(&u32::MAX.to_be_bytes());
    assert!(extract(&bad, &track(), 0).is_err());
    for n in 1..init().len() {
        assert!(parse_init(&init()[..n]).is_err(), "init prefix {n}");
    }
}
#[test]
fn extended_and_zero_sized_boxes() {
    let mut extended = words(&[1]);
    extended.extend(b"free");
    extended.extend(16u64.to_be_bytes());
    assert_eq!(boxes(&extended, 0).unwrap().len(), 1);
    let mut zero = words(&[0]);
    zero.extend(b"mdat");
    zero.extend(b"data");
    assert_eq!(boxes(&zero, 0).unwrap()[0].payload, b"data");
    assert!(boxes(&words(&[4, 0]), 0).is_err());
}

#[test]
fn defaults_multiple_runs_and_multiple_mdats() {
    let mut track = track();
    track.default_size = 3;
    let tfhd = full(b"tfhd", 0, 0x020000, &words(&[1]));
    let tfdt = full(b"tfdt", 0, 0, &words(&[1000]));
    let make = |a: u32, b: u32| {
        bx(
            b"moof",
            &bx(
                b"traf",
                &[
                    tfhd.clone(),
                    tfdt.clone(),
                    full(b"trun", 0, 1, &words(&[1, a])),
                    full(b"trun", 0, 1, &words(&[1, b])),
                ]
                .concat(),
            ),
        )
    };
    let offset = make(0, 0).len() as u32;
    let bytes = [
        make(offset + 8, offset + 8 + 3 + 8),
        bx(b"mdat", b"one"),
        bx(b"mdat", b"two"),
    ]
    .concat();
    let samples = extract(&bytes, &track, 0).unwrap();
    assert_eq!(samples.len(), 2);
    assert_eq!(samples[0].document, b"one");
    assert_eq!(samples[1].document, b"two");
    assert_eq!(samples[1].decode_time, 2000);
}
#[test]
fn tfhd_defaults_and_absolute_byte_range_base() {
    let file_offset = 1024u64;
    let make = |base: u64| {
        let mut data = words(&[1]);
        data.extend(base.to_be_bytes());
        data.extend(words(&[250, 3]));
        bx(
            b"moof",
            &bx(
                b"traf",
                &[
                    full(b"tfhd", 0, 1 | 8 | 16, &data),
                    full(b"tfdt", 1, 0, &(u32::MAX as u64 + 1).to_be_bytes()),
                    full(b"trun", 0, 0, &words(&[1])),
                ]
                .concat(),
            ),
        )
    };
    let size = make(0).len();
    let bytes = [make(file_offset + size as u64 + 8), bx(b"mdat", b"one")].concat();
    let samples = extract(&bytes, &track(), file_offset).unwrap();
    assert_eq!(samples[0].decode_time, u32::MAX as u64 + 1);
    assert_eq!(samples[0].duration, 250);
    assert_eq!(samples[0].document, b"one");
    assert!(extract(&bytes, &track(), file_offset + size as u64 + 9).is_err());
}
#[test]
fn multiple_fragments_keep_decode_timestamps() {
    let bytes = [segment(0, 1), segment(100, 1)].concat();
    let samples = extract(&bytes, &track(), 0).unwrap();
    assert_eq!(samples.len(), 4);
    assert_eq!(samples[2].presentation_time, 10100);
}
#[test]
fn duration_empty_fragment_and_missing_decode_time() {
    let tfhd = full(b"tfhd", 0, 0x030000, &words(&[1]));
    let bytes = bx(
        b"moof",
        &bx(
            b"traf",
            &[
                tfhd.clone(),
                full(b"tfdt", 0, 0, &words(&[0])),
                full(b"trun", 0, 0, &words(&[0])),
            ]
            .concat(),
        ),
    );
    assert!(extract(&bytes, &track(), 0).unwrap().is_empty());
    assert!(extract(&bx(b"moof", &bx(b"traf", &tfhd)), &track(), 0).is_err());
}
#[test]
fn subsamples_encryption_and_description_switch_are_explicit() {
    for kind in [*b"subs", *b"senc", *b"saiz", *b"saio"] {
        let bytes = bx(
            b"moof",
            &bx(
                b"traf",
                &[full(b"tfhd", 0, 0x020000, &words(&[1])), bx(&kind, &[])].concat(),
            ),
        );
        assert!(matches!(
            extract(&bytes, &track(), 0),
            Err(Error::Unsupported(_))
        ));
    }
    let bytes = bx(
        b"moof",
        &bx(b"traf", &full(b"tfhd", 0, 0x020002, &words(&[1, 2]))),
    );
    assert!(matches!(
        extract(&bytes, &track(), 0),
        Err(Error::Unsupported(_))
    ));
}
#[test]
fn samples_do_not_consume_mdat_headers() {
    let tfhd = full(b"tfhd", 0, 0x020000, &words(&[1]));
    let tfdt = full(b"tfdt", 0, 0, &words(&[0]));
    let make = |offset: u32| {
        bx(
            b"moof",
            &bx(
                b"traf",
                &[
                    tfhd.clone(),
                    tfdt.clone(),
                    full(b"trun", 0, 0x201, &words(&[2, offset, 3, 3])),
                ]
                .concat(),
            ),
        )
    };
    let offset = make(0).len() as u32;
    let bytes = [make(offset + 8), bx(b"mdat", b"one"), bx(b"mdat", b"two")].concat();
    assert!(extract(&bytes, &track(), 0).is_err());
}
