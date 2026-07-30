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
            ("orderedList", "start") if value.as_i64().is_some_and(|start| start > 0) => {}
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
        let scheme = url::Url::parse(href)
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
}
