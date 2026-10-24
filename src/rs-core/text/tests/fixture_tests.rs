//! Regression snapshots of authored fixtures. No JavaScript or external parser
//! checkout is needed to run these tests. Snapshots cover the complete model.
use super::super::{imsc, vtt};
use std::{fs, path::Path};
fn check(kind: &str, extension: &str) -> usize {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/rs-core/text/tests/fixtures")
        .join(kind);
    let mut count = 0;
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|s| s.to_str()) != Some(extension) {
            continue;
        }
        let input = fs::read(&path).unwrap();
        let actual = if kind == "vtt" {
            format!("{:#?}\n", vtt::parse(&input, None).unwrap())
        } else {
            format!("{:#?}\n", imsc::parse(&input).unwrap())
        };
        if std::env::var_os("UPDATE_SUBTITLE_FIXTURES").is_some() {
            fs::write(path.with_extension("expected"), &actual).unwrap();
        }
        let expected = fs::read_to_string(path.with_extension("expected")).unwrap();
        assert_eq!(actual, expected, "{}", path.display());
        count += 1;
    }
    count
}
#[test]
fn webvtt_snapshots() {
    assert!(check("vtt", "vtt") > 0);
}
#[test]
fn imsc_snapshots() {
    assert!(check("imsc", "ttml") > 0);
}
