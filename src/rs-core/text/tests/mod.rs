//! Each parser change has authored fixtures and tests of its public output.
mod fixture_tests;
mod text;
use super::*;
fn ttml(s: &str) -> Document {
    imsc::parse(wrap(s).as_bytes()).unwrap()
}
fn wrap(s: &str) -> String {
    format!(
        r#"<tt xmlns="http://www.w3.org/ns/ttml" xmlns:tts="http://www.w3.org/ns/ttml#styling" xmlns:ttp="http://www.w3.org/ns/ttml#parameter">{s}</tt>"#
    )
}
fn vtt(s: &str) -> Document {
    vtt::parse(format!("WEBVTT\n\n{s}").as_bytes(), None).unwrap()
}
fn leaf<'a>(d: &'a Document, text: &str) -> &'a Node {
    d.nodes
        .iter()
        .find(|n| n.text.as_deref() == Some(text))
        .unwrap()
}
fn plain(d: &Document) -> String {
    d.nodes
        .iter()
        .map(|n| {
            if n.kind == Kind::Break {
                "\n"
            } else {
                n.text.as_deref().unwrap_or("")
            }
        })
        .collect()
}
