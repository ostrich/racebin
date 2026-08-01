use super::rich_text::validate_document;
use html5ever::{local_name, ns, parse_fragment, tendril::TendrilSink, QualName};
use markup5ever_rcdom::{Handle, NodeData, RcDom};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

const MAX_DOCUMENT_BYTES: usize = 2 * 1024 * 1024;

pub fn html_to_document(input: &str) -> Result<Value, String> {
    if input.len() > MAX_DOCUMENT_BYTES {
        return Err("Rich-text HTML exceeds 2 MiB".into());
    }
    let tags = [
        "p",
        "h1",
        "h2",
        "h3",
        "strong",
        "b",
        "em",
        "i",
        "u",
        "s",
        "del",
        "code",
        "pre",
        "blockquote",
        "ul",
        "ol",
        "li",
        "hr",
        "br",
        "a",
    ]
    .into_iter()
    .collect::<HashSet<_>>();
    let attributes = HashMap::from([
        ("a", HashSet::from(["href", "title"])),
        ("p", HashSet::from(["style"])),
        ("h1", HashSet::from(["style"])),
        ("h2", HashSet::from(["style"])),
        ("h3", HashSet::from(["style"])),
        ("ol", HashSet::from(["start", "type"])),
        ("code", HashSet::from(["class"])),
    ]);
    let cleaned = ammonia::Builder::new()
        .tags(tags)
        .tag_attributes(attributes)
        .clean_content_tags(HashSet::from([
            "script", "style", "iframe", "object", "embed",
        ]))
        .url_schemes(HashSet::from(["http", "https", "mailto"]))
        .url_relative(ammonia::UrlRelative::PassThrough)
        .link_rel(Some("noopener noreferrer nofollow"))
        .attribute_filter(|element, attribute, value| match (element, attribute) {
            ("p" | "h1" | "h2" | "h3", "style") => normalize_alignment(value),
            ("code", "class") => normalize_language_class(value),
            ("ol", "start") if value.parse::<i64>().is_ok() => Some(value.into()),
            ("ol", "type") if matches!(value, "1" | "a" | "A" | "i" | "I") => Some(value.into()),
            (_, "href" | "title") => Some(value.into()),
            _ => None,
        })
        .clean(input)
        .to_string();
    let dom = parse_fragment(
        RcDom::default(),
        Default::default(),
        QualName::new(None, ns!(html), local_name!("div")),
        Vec::new(),
        false,
    )
    .one(cleaned);
    let mut content = Vec::new();
    let root = dom.document.clone();
    collect_blocks(&root, &mut content);
    if content.is_empty() {
        content.push(json!({"type":"paragraph","content":[]}));
    }
    let document = json!({"type":"doc","content":content});
    validate_document(&document)?;
    Ok(document)
}

fn normalize_alignment(value: &str) -> Option<std::borrow::Cow<'_, str>> {
    let compact = value.replace(' ', "").to_ascii_lowercase();
    let alignment = compact.strip_prefix("text-align:")?.trim_end_matches(';');
    matches!(alignment, "left" | "center" | "right")
        .then(|| std::borrow::Cow::Owned(format!("text-align: {alignment}")))
}

fn normalize_language_class(value: &str) -> Option<std::borrow::Cow<'_, str>> {
    let language = value.strip_prefix("language-")?;
    (!language.is_empty()
        && language.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '+')
        }))
    .then(|| std::borrow::Cow::Owned(format!("language-{language}")))
}

fn node_name(node: &Handle) -> Option<String> {
    match &node.data {
        NodeData::Element { name, .. } => Some(name.local.to_string()),
        _ => None,
    }
}

fn node_attributes(node: &Handle) -> HashMap<String, String> {
    match &node.data {
        NodeData::Element { attrs, .. } => attrs
            .borrow()
            .iter()
            .map(|attribute| {
                (
                    attribute.name.local.to_string(),
                    attribute.value.to_string(),
                )
            })
            .collect(),
        _ => HashMap::new(),
    }
}

fn collect_blocks(node: &Handle, output: &mut Vec<Value>) {
    for child in node.children.borrow().iter() {
        let Some(name) = node_name(child) else {
            if matches!(&child.data, NodeData::Document) {
                collect_blocks(child, output);
            } else if matches!(&child.data, NodeData::Text { contents } if !contents.borrow().trim().is_empty())
            {
                let mut inline = Vec::new();
                collect_inline(child, &[], &mut inline);
                output.push(json!({"type":"paragraph","content":inline}));
            }
            continue;
        };
        match name.as_str() {
            "p" | "h1" | "h2" | "h3" => {
                let mut inline = Vec::new();
                collect_inline_children(child, &[], &mut inline);
                let attrs = node_attributes(child);
                let align = attrs
                    .get("style")
                    .and_then(|style| style.strip_prefix("text-align: "));
                if name == "p" {
                    output.push(json!({
                        "type":"paragraph",
                        "attrs":{"textAlign":align},
                        "content":inline
                    }));
                } else {
                    output.push(json!({
                        "type":"heading",
                        "attrs":{"level":name[1..].parse::<i64>().unwrap_or(1),"textAlign":align},
                        "content":inline
                    }));
                }
            }
            "ul" | "ol" => output.push(list_node(child, name == "ol")),
            "blockquote" => {
                let mut content = Vec::new();
                collect_blocks(child, &mut content);
                output.push(json!({"type":"blockquote","content":content}));
            }
            "pre" => {
                let mut text = String::new();
                collect_text(child, &mut text);
                let language = child
                    .children
                    .borrow()
                    .iter()
                    .find(|item| node_name(item).as_deref() == Some("code"))
                    .and_then(|code| node_attributes(code).get("class").cloned())
                    .and_then(|class| class.strip_prefix("language-").map(str::to_string));
                output.push(json!({
                    "type":"codeBlock","attrs":{"language":language},
                    "content": if text.is_empty() { Vec::<Value>::new() } else { vec![json!({"type":"text","text":text})] }
                }));
            }
            "hr" => output.push(json!({"type":"horizontalRule"})),
            "html" | "body" | "div" | "section" | "article" | "main" => {
                collect_blocks(child, output);
            }
            _ => {
                let mut inline = Vec::new();
                collect_inline(child, &[], &mut inline);
                if !inline.is_empty() {
                    output.push(json!({"type":"paragraph","content":inline}));
                }
            }
        }
    }
}

fn list_node(node: &Handle, ordered: bool) -> Value {
    let attrs = node_attributes(node);
    let mut items = Vec::new();
    for child in node.children.borrow().iter() {
        if node_name(child).as_deref() != Some("li") {
            continue;
        }
        let mut blocks = Vec::new();
        let mut direct_inline = Vec::new();
        for item in child.children.borrow().iter() {
            if node_name(item).is_some_and(|name| {
                matches!(
                    name.as_str(),
                    "p" | "h1" | "h2" | "h3" | "ul" | "ol" | "blockquote" | "pre"
                )
            }) {
                if !direct_inline.is_empty() {
                    blocks.push(
                        json!({"type":"paragraph","content":std::mem::take(&mut direct_inline)}),
                    );
                }
                let temporary = RcDom::default();
                temporary.document.children.borrow_mut().push(item.clone());
                collect_blocks(&temporary.document, &mut blocks);
            } else {
                collect_inline(item, &[], &mut direct_inline);
            }
        }
        if !direct_inline.is_empty() || blocks.is_empty() {
            blocks.insert(0, json!({"type":"paragraph","content":direct_inline}));
        }
        items.push(json!({"type":"listItem","content":blocks}));
    }
    if ordered {
        json!({"type":"orderedList","attrs":{
            "start":attrs.get("start").and_then(|value| value.parse::<i64>().ok()).unwrap_or(1),
            "type":attrs.get("type")
        },"content":items})
    } else {
        json!({"type":"bulletList","content":items})
    }
}

fn collect_inline_children(node: &Handle, marks: &[Value], output: &mut Vec<Value>) {
    for child in node.children.borrow().iter() {
        collect_inline(child, marks, output);
    }
}

fn collect_inline(node: &Handle, marks: &[Value], output: &mut Vec<Value>) {
    match &node.data {
        NodeData::Text { contents } => {
            let text = contents.borrow().to_string();
            if !text.is_empty() {
                let mut value = json!({"type":"text","text":text});
                if !marks.is_empty() {
                    value["marks"] = Value::Array(marks.to_vec());
                }
                output.push(value);
            }
        }
        NodeData::Element { name, .. } if name.local.as_ref() == "br" => {
            output.push(json!({"type":"hardBreak"}));
        }
        NodeData::Element { name, .. } => {
            let name = name.local.as_ref();
            let mut next_marks = marks.to_vec();
            let mark = match name {
                "strong" | "b" => Some(json!({"type":"bold"})),
                "em" | "i" => Some(json!({"type":"italic"})),
                "u" => Some(json!({"type":"underline"})),
                "s" | "del" => Some(json!({"type":"strike"})),
                "code" => Some(json!({"type":"code"})),
                "a" => {
                    let attrs = node_attributes(node);
                    attrs.get("href").map(|href| {
                        json!({"type":"link","attrs":{
                            "href":href,"title":attrs.get("title"),"target":"_blank",
                            "rel":"noopener noreferrer nofollow","class":null
                        }})
                    })
                }
                _ => None,
            };
            if let Some(mark) = mark {
                next_marks.push(mark);
            }
            collect_inline_children(node, &next_marks, output);
        }
        _ => collect_inline_children(node, marks, output),
    }
}

fn collect_text(node: &Handle, output: &mut String) {
    if let NodeData::Text { contents } = &node.data {
        output.push_str(&contents.borrow());
    }
    for child in node.children.borrow().iter() {
        collect_text(child, output);
    }
}
