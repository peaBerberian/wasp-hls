use super::*;
#[test]
fn language_is_inherited_and_labels_only_subtitle_elements() {
    let d = ttml("<body xml:lang='fr'><p>oui <span xml:lang='en'>yes</span></p></body>");
    assert_eq!(leaf(&d, "oui ").language.as_deref(), Some("fr"));
    assert_eq!(leaf(&d, "yes").language.as_deref(), Some("en"));
    let d = vtt("00:00.000 --> 00:02.000\n<lang ja>日本語 <lang en>English</lang></lang>");
    assert_eq!(leaf(&d, "日本語 ").language.as_deref(), Some("ja"));
    assert_eq!(leaf(&d, "English").language.as_deref(), Some("en"));
}
#[test]
fn ruby_retains_both_texts_with_distinct_roles() {
    for d in [vtt("00:00.000 --> 00:02.000\n<ruby>漢<rt>かん</rt></ruby>"),ttml("<body><p><span tts:ruby='container'><span tts:ruby='base'>漢</span><span tts:ruby='text'>かん</span></span></p></body>")] {
        assert!(d.nodes.iter().any(|n|n.kind==Kind::Ruby));assert!(d.nodes.iter().any(|n|n.kind==Kind::RubyText));assert_eq!(plain(&d),"漢かん");
    }
    let d = vtt("00:00.000 --> 00:02.000\n<rt>ordinary</rt>");
    assert!(!d.nodes.iter().any(|n| n.kind == Kind::RubyText));
    assert_eq!(plain(&d), "ordinary");
}

#[test]
fn ruby_reading_spaces_do_not_remove_main_text_spaces() {
    let d = ttml(
        "<body><p><span tts:ruby='container'>A<span tts:ruby='text'> B </span></span> C</p></body>",
    );
    assert!(d.nodes.iter().any(|n| n.text.as_deref() == Some(" C")));
    assert!(d.nodes.iter().any(|n| n.text.as_deref() == Some("B")));
    let s = format!(
        "WEBVTT\n\n00:00.000 --> 00:02.000\n<lang {}>x</lang>",
        "a".repeat(10000)
    );
    assert!(matches!(
        vtt::parse_with_limits(
            s.as_bytes(),
            None,
            Limits {
                output_bytes: 2000,
                ..Limits::default()
            }
        ),
        Err(Error::Limit(_))
    ));
}
