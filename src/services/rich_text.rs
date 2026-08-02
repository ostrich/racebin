use serde_json::{json, Map, Value};

const MAX_DOCUMENT_BYTES: usize = 2 * 1024 * 1024;
const MAX_DEPTH: usize = 32;
const MAX_NODES: usize = 100_000;

pub fn text_to_document(text: &str) -> Value {
    let mut blocks = Vec::new();
    for paragraph in text.split("\n\n") {
        let mut content = Vec::new();
        for (index, line) in paragraph.split('\n').enumerate() {
            if index != 0 {
                content.push(json!({"type":"hardBreak"}));
            }
            if !line.is_empty() {
                content.push(json!({"type":"text","text":line}));
            }
        }
        blocks.push(json!({"type":"paragraph","content":content}));
    }
    json!({"type":"doc","content":blocks})
}

pub fn validate_document(document: &Value) -> Result<String, String> {
    if serde_json::to_vec(document)
        .map_err(|error| error.to_string())?
        .len()
        > MAX_DOCUMENT_BYTES
    {
        return Err("Rich-text document exceeds 2 MiB".into());
    }
    let mut node_count = 0;
    validate_node(document, 0, &mut node_count, None)?;
    let text = document_to_text(document);
    if text.chars().count() > 1_000_000 {
        return Err("Rich-text content exceeds 1,000,000 characters".into());
    }
    Ok(text)
}

fn validate_node(
    node: &Value,
    depth: usize,
    node_count: &mut usize,
    parent_type: Option<&str>,
) -> Result<(), String> {
    if depth > MAX_DEPTH {
        return Err("Rich-text document is nested too deeply".into());
    }
    *node_count += 1;
    if *node_count > MAX_NODES {
        return Err("Rich-text document contains too many nodes".into());
    }
    let object = node
        .as_object()
        .ok_or("Every rich-text node must be an object")?;
    let node_type = string_field(object, "type")?;
    let allowed = matches!(
        node_type,
        "doc"
            | "paragraph"
            | "heading"
            | "bulletList"
            | "orderedList"
            | "listItem"
            | "blockquote"
            | "horizontalRule"
            | "hardBreak"
            | "text"
            | "codeBlock"
    );
    if !allowed || parent_type.is_none() != (node_type == "doc") {
        return Err(format!("Unsupported rich-text node: {node_type}"));
    }
    if let Some(parent_type) = parent_type {
        let valid_child = match parent_type {
            "doc" | "blockquote" => matches!(
                node_type,
                "paragraph"
                    | "heading"
                    | "bulletList"
                    | "orderedList"
                    | "blockquote"
                    | "horizontalRule"
                    | "codeBlock"
            ),
            "paragraph" | "heading" => matches!(node_type, "text" | "hardBreak"),
            "bulletList" | "orderedList" => node_type == "listItem",
            "listItem" => matches!(
                node_type,
                "paragraph" | "bulletList" | "orderedList" | "blockquote" | "codeBlock"
            ),
            "codeBlock" => node_type == "text",
            _ => false,
        };
        if !valid_child {
            return Err(format!("{node_type} is not valid inside {parent_type}"));
        }
    }
    validate_attributes(node_type, object.get("attrs"))?;
    if node_type == "text" {
        string_field(object, "text")?;
        if object.contains_key("content") {
            return Err("Text nodes cannot contain child nodes".into());
        }
    } else if let Some(children) = object.get("content") {
        for child in children.as_array().ok_or("Node content must be an array")? {
            validate_node(child, depth + 1, node_count, Some(node_type))?;
        }
    }
    if let Some(marks) = object.get("marks") {
        if node_type != "text" {
            return Err("Only text nodes may contain marks".into());
        }
        for mark in marks.as_array().ok_or("Marks must be an array")? {
            validate_mark(mark)?;
        }
    }
    for key in object.keys() {
        if !matches!(
            key.as_str(),
            "type" | "text" | "content" | "attrs" | "marks"
        ) {
            return Err(format!("Unsupported rich-text node property: {key}"));
        }
    }
    Ok(())
}

fn validate_attributes(node_type: &str, attributes: Option<&Value>) -> Result<(), String> {
    let Some(attributes) = attributes else {
        return Ok(());
    };
    let attributes = attributes
        .as_object()
        .ok_or("Node attributes must be an object")?;
    for (key, value) in attributes {
        match (node_type, key.as_str()) {
            ("heading", "level") if matches!(value.as_i64(), Some(1..=3)) => {}
            ("paragraph" | "heading", "textAlign")
                if matches!(value.as_str(), Some("left" | "center" | "right")) => {}
            ("paragraph" | "heading", "textAlign") if value.is_null() => {}
            ("orderedList", "start") if value.as_i64().is_some() => {}
            ("orderedList", "type") if value.is_null() => {}
            ("orderedList", "type")
                if matches!(value.as_str(), Some("1" | "a" | "A" | "i" | "I")) => {}
            ("codeBlock", "language") if value.is_string() || value.is_null() => {}
            _ => return Err(format!("Unsupported {node_type} attribute: {key}")),
        }
    }
    Ok(())
}

fn validate_mark(mark: &Value) -> Result<(), String> {
    let object = mark.as_object().ok_or("Every mark must be an object")?;
    let mark_type = string_field(object, "type")?;
    if !matches!(
        mark_type,
        "bold" | "italic" | "underline" | "strike" | "code" | "link"
    ) {
        return Err(format!("Unsupported rich-text mark: {mark_type}"));
    }
    if mark_type == "link" {
        let attributes = object
            .get("attrs")
            .and_then(Value::as_object)
            .ok_or("Links require attributes")?;
        let href = string_field(attributes, "href")?;
        let base = url::Url::parse("https://racebin.invalid/").expect("valid link base");
        let scheme = url::Url::options()
            .base_url(Some(&base))
            .parse(href)
            .map_err(|_| "Link URL is invalid")?
            .scheme()
            .to_string();
        if !matches!(scheme.as_str(), "http" | "https" | "mailto") {
            return Err("Links support only http, https, and mailto".into());
        }
        for key in attributes.keys() {
            if !matches!(key.as_str(), "href" | "title" | "target" | "rel" | "class") {
                return Err(format!("Unsupported link attribute: {key}"));
            }
        }
        if attributes
            .get("target")
            .is_some_and(|value| !value.is_null() && value.as_str() != Some("_blank"))
            || attributes.get("rel").is_some_and(|value| {
                !value.is_null() && value.as_str() != Some("noopener noreferrer nofollow")
            })
            || attributes
                .get("class")
                .is_some_and(|value| !value.is_null())
            || attributes
                .get("title")
                .is_some_and(|value| !value.is_null() && !value.is_string())
        {
            return Err("Link attributes are invalid".into());
        }
    } else if object.contains_key("attrs") {
        return Err(format!("{mark_type} marks do not accept attributes"));
    }
    for key in object.keys() {
        if !matches!(key.as_str(), "type" | "attrs") {
            return Err(format!("Unsupported rich-text mark property: {key}"));
        }
    }
    Ok(())
}

fn string_field<'a>(object: &'a Map<String, Value>, name: &str) -> Result<&'a str, String> {
    object
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("Rich-text {name} must be a string"))
}

pub fn document_to_text(document: &Value) -> String {
    let mut output = String::new();
    append_text(document, &mut output, 0);
    output.trim_end_matches('\n').to_string()
}

pub fn document_to_html(document: &Value) -> String {
    let mut output = String::new();
    if let Some(children) = document.get("content").and_then(Value::as_array) {
        for child in children {
            write_html_node(child, &mut output);
        }
    }
    output
}

fn write_html_node(node: &Value, output: &mut String) {
    let node_type = node.get("type").and_then(Value::as_str).unwrap_or("");
    let attrs = node.get("attrs").and_then(Value::as_object);
    let align = attrs
        .and_then(|attrs| attrs.get("textAlign"))
        .and_then(Value::as_str)
        .filter(|value| matches!(*value, "left" | "center" | "right"))
        .map(|value| format!(" style=\"text-align: {value}\""))
        .unwrap_or_default();
    match node_type {
        "text" => write_marked_text(node, output),
        "hardBreak" => output.push_str("<br>"),
        "horizontalRule" => output.push_str("<hr>"),
        "paragraph" => write_container("p", &align, node, output),
        "heading" => {
            let level = attrs
                .and_then(|attrs| attrs.get("level"))
                .and_then(Value::as_i64)
                .unwrap_or(1)
                .clamp(1, 3);
            write_container(&format!("h{level}"), &align, node, output);
        }
        "bulletList" => write_container("ul", "", node, output),
        "orderedList" => {
            let start = attrs
                .and_then(|attrs| attrs.get("start"))
                .and_then(Value::as_i64)
                .unwrap_or(1);
            let list_type = attrs
                .and_then(|attrs| attrs.get("type"))
                .and_then(Value::as_str);
            let mut html_attrs = if start != 1 {
                format!(" start=\"{start}\"")
            } else {
                String::new()
            };
            if let Some(list_type) =
                list_type.filter(|value| matches!(*value, "1" | "a" | "A" | "i" | "I"))
            {
                html_attrs.push_str(&format!(" type=\"{list_type}\""));
            }
            write_container("ol", &html_attrs, node, output);
        }
        "listItem" => write_container("li", "", node, output),
        "blockquote" => write_container("blockquote", "", node, output),
        "codeBlock" => {
            let language = attrs
                .and_then(|attrs| attrs.get("language"))
                .and_then(Value::as_str)
                .filter(|value| {
                    value.chars().all(|character| {
                        character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '+')
                    })
                });
            output.push_str("<pre><code");
            if let Some(language) = language {
                output.push_str(&format!(" class=\"language-{language}\""));
            }
            output.push('>');
            if let Some(children) = node.get("content").and_then(Value::as_array) {
                for child in children {
                    escape_html(
                        child.get("text").and_then(Value::as_str).unwrap_or(""),
                        output,
                    );
                }
            }
            output.push_str("</code></pre>");
        }
        "doc" => {
            if let Some(children) = node.get("content").and_then(Value::as_array) {
                for child in children {
                    write_html_node(child, output);
                }
            }
        }
        _ => {}
    }
}

fn write_container(tag: &str, attributes: &str, node: &Value, output: &mut String) {
    output.push('<');
    output.push_str(tag);
    output.push_str(attributes);
    output.push('>');
    if let Some(children) = node.get("content").and_then(Value::as_array) {
        for child in children {
            write_html_node(child, output);
        }
    }
    output.push_str("</");
    output.push_str(tag);
    output.push('>');
}

fn write_marked_text(node: &Value, output: &mut String) {
    let marks = node
        .get("marks")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for mark in &marks {
        match mark.get("type").and_then(Value::as_str).unwrap_or("") {
            "bold" => output.push_str("<strong>"),
            "italic" => output.push_str("<em>"),
            "underline" => output.push_str("<u>"),
            "strike" => output.push_str("<s>"),
            "code" => output.push_str("<code>"),
            "link" => {
                let attrs = mark.get("attrs").and_then(Value::as_object);
                output.push_str("<a href=\"");
                escape_html(
                    attrs
                        .and_then(|attrs| attrs.get("href"))
                        .and_then(Value::as_str)
                        .unwrap_or(""),
                    output,
                );
                output.push_str("\" rel=\"noopener noreferrer nofollow\" target=\"_blank\"");
                if let Some(title) = attrs
                    .and_then(|attrs| attrs.get("title"))
                    .and_then(Value::as_str)
                {
                    output.push_str(" title=\"");
                    escape_html(title, output);
                    output.push('"');
                }
                output.push('>');
            }
            _ => {}
        }
    }
    escape_html(
        node.get("text").and_then(Value::as_str).unwrap_or(""),
        output,
    );
    for mark in marks.iter().rev() {
        match mark.get("type").and_then(Value::as_str).unwrap_or("") {
            "bold" => output.push_str("</strong>"),
            "italic" => output.push_str("</em>"),
            "underline" => output.push_str("</u>"),
            "strike" => output.push_str("</s>"),
            "code" => output.push_str("</code>"),
            "link" => output.push_str("</a>"),
            _ => {}
        }
    }
}

fn escape_html(value: &str, output: &mut String) {
    for character in value.chars() {
        match character {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            '"' => output.push_str("&quot;"),
            '\'' => output.push_str("&#39;"),
            _ => output.push(character),
        }
    }
}

fn append_text(node: &Value, output: &mut String, list_depth: usize) {
    let Some(object) = node.as_object() else {
        return;
    };
    let node_type = object.get("type").and_then(Value::as_str).unwrap_or("");
    if node_type == "text" {
        output.push_str(object.get("text").and_then(Value::as_str).unwrap_or(""));
        return;
    }
    if node_type == "hardBreak" {
        output.push('\n');
        return;
    }
    if node_type == "horizontalRule" {
        output.push_str("---\n\n");
        return;
    }
    let children = object.get("content").and_then(Value::as_array);
    if let Some(children) = children {
        for (index, child) in children.iter().enumerate() {
            if node_type == "listItem" && index == 0 {
                output.push_str(&"  ".repeat(list_depth.saturating_sub(1)));
                output.push_str("- ");
            }
            append_text(
                child,
                output,
                list_depth + usize::from(matches!(node_type, "bulletList" | "orderedList")),
            );
        }
    }
    if matches!(
        node_type,
        "paragraph" | "heading" | "blockquote" | "codeBlock" | "listItem"
    ) {
        output.push('\n');
    }
    if matches!(
        node_type,
        "paragraph" | "heading" | "blockquote" | "codeBlock"
    ) {
        output.push('\n');
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::rich_text_import::html_to_document;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn arbitrary_html_imports_to_a_valid_serializable_document(input in ".{0,4096}") {
            let document = html_to_document(&input).expect("HTML import should be total");
            prop_assert!(validate_document(&document).is_ok());
            let rendered = document_to_html(&document);
            let reparsed = html_to_document(&rendered).expect("rendered HTML should be importable");
            prop_assert!(validate_document(&reparsed).is_ok());
        }
    }

    #[test]
    fn text_conversion_preserves_script_line_breaks() {
        let source = "INT. HOUSE - NIGHT\n\nSAM\n(quietly)\nWe should go.";
        let document = text_to_document(source);
        assert_eq!(validate_document(&document).unwrap(), source);
    }

    #[test]
    fn validation_rejects_scripts_and_unsafe_links() {
        assert!(validate_document(&json!({"type":"doc","content":[{"type":"script"}]})).is_err());
        assert!(validate_document(&json!({"type":"doc","content":[{
            "type":"paragraph","content":[{"type":"text","text":"bad","marks":[{
                "type":"link","attrs":{"href":"javascript:alert(1)"}
            }]}]
        }]}))
        .is_err());
    }

    #[test]
    fn validation_accepts_ordered_list_attributes_emitted_by_tiptap() {
        let document = json!({"type":"doc","content":[{
            "type":"orderedList","attrs":{"start":1,"type":null},"content":[{
                "type":"listItem","content":[{
                    "type":"paragraph","content":[{"type":"text","text":"First"}]
                }]
            }]
        }]});
        assert_eq!(validate_document(&document).unwrap(), "- First");

        let mut styled = document.clone();
        styled["content"][0]["attrs"]["type"] = json!("A");
        assert!(validate_document(&styled).is_ok());
        styled["content"][0]["attrs"]["type"] = json!("unsupported");
        assert!(validate_document(&styled).is_err());

        let mut reverse_start = document;
        reverse_start["content"][0]["attrs"]["start"] = json!(-2);
        assert!(validate_document(&reverse_start).is_ok());
    }

    #[test]
    fn validation_accepts_safe_relative_links() {
        let linked = |href| {
            json!({"type":"doc","content":[{
                "type":"paragraph","content":[{"type":"text","text":"link","marks":[{
                    "type":"link","attrs":{
                        "href":href,"target":"_blank",
                        "rel":"noopener noreferrer nofollow","class":null,"title":null
                    }
                }]}]
            }]})
        };
        for href in [
            "/help",
            "../pastes",
            "#section",
            "https://example.com",
            "mailto:user@example.com",
        ] {
            assert!(validate_document(&linked(href)).is_ok(), "rejected {href}");
        }
        for href in [
            "javascript:alert(1)",
            "data:text/html,bad",
            "tel:+15551212",
            "sms:+15551212",
            "ftp://example.com",
        ] {
            assert!(validate_document(&linked(href)).is_err(), "accepted {href}");
        }
    }

    #[test]
    fn html_round_trip_preserves_supported_script_formatting() {
        let html = "<h1>Episode title</h1><p><strong>INT. LAB - NIGHT</strong></p><p style=\"text-align:center\"><em>Quietly</em><br>We should go.</p><ol start=\"0\"><li><p>First beat</p></li></ol>";
        let document = html_to_document(html).unwrap();
        let output = document_to_html(&document);
        assert!(output.contains("<h1>Episode title</h1>"), "{output}");
        assert!(output.contains("<strong>INT. LAB - NIGHT</strong>"));
        assert!(output.contains("text-align: center"));
        assert!(output.contains("<ol start=\"0\">"));
        assert!(validate_document(&document).is_ok());
    }

    #[test]
    fn html_import_removes_active_content_and_normalizes_links() {
        let document = html_to_document(
            "<script>alert(1)</script><p onclick=\"bad()\"><a href=\"javascript:alert(2)\">bad</a><a href=\"/help\" target=\"popup\" class=\"foreign\">good</a></p>",
        )
        .unwrap();
        let output = document_to_html(&document);
        assert!(!output.contains("script"));
        assert!(!output.contains("onclick"));
        assert!(!output.contains("javascript:"));
        assert!(!output.contains("foreign"));
        assert!(output.contains("href=\"/help\""));
        assert!(output.contains("noopener noreferrer nofollow"));
    }
}
