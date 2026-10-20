use super::{entities::decode as entities, model::*, timestamp};
use crate::text::{budget::Budget, Error, Limits};

fn tag(s: &str) -> Option<Tag> {
    Some(match s {
        "c" => Tag::Class,
        "i" => Tag::Italic,
        "b" => Tag::Bold,
        "u" => Tag::Underline,
        "ruby" => Tag::Ruby,
        "rt" => Tag::RubyText,
        "v" => Tag::Voice,
        "lang" => Tag::Language,
        _ => return None,
    })
}
pub(super) fn cue_text(
    source: &str,
    limits: Limits,
    budget: &mut Budget,
) -> Result<Vec<Node>, Error> {
    budget.nodes(1)?;
    budget.bytes(std::mem::size_of::<Node>() * 2)?;
    let mut nodes = vec![Node {
        parent: None,
        kind: NodeKind::Root,
    }];
    let mut stack = vec![0];
    let mut tail = source;
    while !tail.is_empty() {
        if nodes.len() >= limits.nodes {
            return Err(Error::Limit("nodes"));
        }
        if let Some(after) = tail.strip_prefix('<') {
            let (token, rest) = after.split_once('>').unwrap_or((after, ""));
            tail = rest;
            if let Some(name) = token.strip_prefix('/') {
                if let Some(close) = tag(name) {
                    if let NodeKind::Element { tag: current, .. } =
                        &nodes[*stack.last().unwrap()].kind
                    {
                        if *current == close {
                            stack.pop();
                        } else if close == Tag::Ruby && *current == Tag::RubyText {
                            stack.pop();
                            if stack.len() > 1 {
                                stack.pop();
                            }
                        }
                    }
                }
                continue;
            }
            if let Some(ms) = timestamp(token) {
                budget.nodes(1)?;
                budget.bytes(std::mem::size_of::<Node>() * 2)?;
                nodes.push(Node {
                    parent: stack.last().copied(),
                    kind: NodeKind::Timestamp(ms),
                });
                continue;
            }
            let split = token
                .find([' ', '\t', '\n', '\r', '\u{c}'])
                .unwrap_or(token.len());
            let name = &token[..split];
            let mut classes = name.split('.');
            let name = classes.next().unwrap_or("");
            if let Some(t) = tag(name) {
                if t == Tag::RubyText
                    && !matches!(
                        &nodes[*stack.last().unwrap()].kind,
                        NodeKind::Element { tag: Tag::Ruby, .. }
                    )
                {
                    continue;
                }
                if stack.len() >= limits.depth {
                    return Err(Error::Limit("depth"));
                }
                budget.nodes(1)?;
                budget.bytes(
                    std::mem::size_of::<Node>() * 2
                        + token.len() * 4
                        + classes.clone().count() * std::mem::size_of::<String>() * 2,
                )?;
                let index = nodes.len();
                nodes.push(Node {
                    parent: stack.last().copied(),
                    kind: NodeKind::Element {
                        tag: t,
                        classes: classes.map(String::from).collect(),
                        annotation: annotation(&token[split..]),
                    },
                });
                stack.push(index);
            }
        } else {
            let next = tail.find('<').unwrap_or(tail.len());
            budget.nodes(1)?;
            budget.bytes(std::mem::size_of::<Node>() * 2 + next * 4)?;
            let text = entities(&tail[..next]);
            tail = &tail[next..];
            nodes.push(Node {
                parent: stack.last().copied(),
                kind: NodeKind::Text(text),
            });
        }
    }
    Ok(nodes)
}

// Normalize spaces without collecting a temporary vector for every word. Voice
// annotations can be long, and that vector used much more space than the text.
fn annotation(source: &str) -> String {
    let decoded = entities(source.trim());
    let mut output = String::with_capacity(decoded.len());
    for (index, word) in decoded.split_ascii_whitespace().enumerate() {
        if index != 0 {
            output.push(' ');
        }
        output.push_str(word);
    }
    output
}
