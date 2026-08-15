use comrak::nodes::{AstNode, NodeValue};
use comrak::{format_html, parse_document, Arena, Options};
use std::collections::{HashMap, HashSet};

const MAX_SOURCE_BYTES: usize = 2 * 1024 * 1024;
const MAX_DEPTH: usize = 32;
const MAX_NODES: usize = 100_000;

fn options() -> Options<'static> {
    let mut options = Options::default();
    options.extension.table = true;
    options.extension.tasklist = true;
    options.extension.strikethrough = true;
    options.extension.autolink = true;
    // Validation permits only the exact hard-break tags emitted by the visual editor;
    // Ammonia still sanitizes the formatted result below.
    options.render.r#unsafe = true;
    options
}

pub struct MarkdownOutput {
    pub html: String,
    pub plain_text: String,
}

pub fn text_to_markdown(text: &str) -> String {
    text.lines()
        .map(|line| {
            line.replace('\\', "\\\\")
                .replace('*', "\\*")
                .replace('_', "\\_")
                .replace('#', "\\#")
                .replace('`', "\\`")
                .replace('[', "\\[")
                .replace(']', "\\]")
        })
        .collect::<Vec<_>>()
        .join("  \n")
}

pub fn render_markdown(source: &str) -> Result<MarkdownOutput, String> {
    if source.len() > MAX_SOURCE_BYTES {
        return Err("Markdown content exceeds 2 MiB".into());
    }
    let arena = Arena::new();
    let options = options();
    let root = parse_document(&arena, source, &options);
    let mut count = 0;
    validate_node(root, 0, &mut count)?;
    let mut raw_html = String::new();
    format_html(root, &options, &mut raw_html).map_err(|error| error.to_string())?;
    let html = ammonia::Builder::new()
        .tags(HashSet::from([
            "p",
            "h1",
            "h2",
            "h3",
            "h4",
            "h5",
            "h6",
            "strong",
            "em",
            "s",
            "code",
            "pre",
            "blockquote",
            "ul",
            "ol",
            "li",
            "hr",
            "br",
            "a",
            "table",
            "thead",
            "tbody",
            "tr",
            "th",
            "td",
            "input",
        ]))
        .tag_attributes(HashMap::from([
            ("a", HashSet::from(["href", "title"])),
            ("code", HashSet::from(["class"])),
            ("input", HashSet::from(["type", "checked", "disabled"])),
            ("ol", HashSet::from(["start"])),
            ("th", HashSet::from(["align"])),
            ("td", HashSet::from(["align"])),
        ]))
        .url_schemes(HashSet::from(["http", "https", "mailto"]))
        .url_relative(ammonia::UrlRelative::PassThrough)
        .link_rel(Some("noopener noreferrer nofollow"))
        .clean(&raw_html)
        .to_string();
    let mut plain_text = String::new();
    append_plain_text(root, &mut plain_text);
    Ok(MarkdownOutput {
        html,
        plain_text: plain_text.trim_end().to_string(),
    })
}

fn validate_node<'a>(node: &'a AstNode<'a>, depth: usize, count: &mut usize) -> Result<(), String> {
    if depth > MAX_DEPTH {
        return Err("Markdown content is nested too deeply".into());
    }
    *count += 1;
    if *count > MAX_NODES {
        return Err("Markdown content contains too many nodes".into());
    }
    match &node.data.borrow().value {
        NodeValue::HtmlInline(html) if is_hard_break_html(html) => {}
        NodeValue::HtmlBlock(_) | NodeValue::HtmlInline(_) => {
            return Err("Raw HTML is not supported in Markdown pastes".into())
        }
        NodeValue::Image(_) => return Err("Embedded images are not supported".into()),
        NodeValue::Link(link) => validate_link(&link.url)?,
        _ => {}
    }
    for child in node.children() {
        validate_node(child, depth + 1, count)?;
    }
    Ok(())
}

fn is_hard_break_html(html: &str) -> bool {
    matches!(
        html.trim().to_ascii_lowercase().as_str(),
        "<br>" | "<br/>" | "<br />"
    )
}

fn validate_link(href: &str) -> Result<(), String> {
    if href.starts_with('#')
        || href.starts_with('/')
        || href.starts_with("./")
        || href.starts_with("../")
    {
        return Ok(());
    }
    let base = url::Url::parse("https://racebin.invalid/").expect("valid link base");
    let scheme = url::Url::options()
        .base_url(Some(&base))
        .parse(href)
        .map_err(|_| "Link URL is invalid")?
        .scheme()
        .to_string();
    matches!(scheme.as_str(), "http" | "https" | "mailto")
        .then_some(())
        .ok_or_else(|| "Links support only http, https, mailto, and relative URLs".into())
}

fn append_plain_text<'a>(node: &'a AstNode<'a>, output: &mut String) {
    match &node.data.borrow().value {
        NodeValue::Text(text) => output.push_str(text),
        NodeValue::Code(code) => output.push_str(&code.literal),
        NodeValue::CodeBlock(code) => output.push_str(&code.literal),
        NodeValue::SoftBreak | NodeValue::LineBreak => output.push('\n'),
        NodeValue::ThematicBreak => output.push_str("---\n"),
        NodeValue::TaskItem(value) => output.push_str(if value.symbol.is_some() {
            "[x] "
        } else {
            "[ ] "
        }),
        _ => {}
    }
    for child in node.children() {
        append_plain_text(child, output);
    }
    match node.data.borrow().value {
        NodeValue::TableCell => output.push('\t'),
        NodeValue::TableRow(_) => {
            if output.ends_with('\t') {
                output.pop();
            }
            push_newline(output);
        }
        NodeValue::Paragraph | NodeValue::Heading(_) | NodeValue::Item(_) => push_newline(output),
        _ => {}
    }
}

fn push_newline(output: &mut String) {
    if !output.ends_with('\n') {
        output.push('\n');
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn arbitrary_markdown_never_panics(source in ".{0,20000}") {
            if let Ok(output) = render_markdown(&source) {
                prop_assert!(!output.html.to_ascii_lowercase().contains("<script"));
                prop_assert!(!output.html.to_ascii_lowercase().contains("javascript:"));
            }
        }
    }

    #[test]
    fn renders_gfm_and_rejects_unsafe_constructs() {
        let output =
            render_markdown("# Title\n\n| A | B |\n|---|---|\n| 1 | 2 |\n\n- [x] Done").unwrap();
        assert!(output.html.contains("<table>"));
        assert!(output.html.contains("type=\"checkbox\""));
        assert!(render_markdown("<script>alert(1)</script>").is_err());
        assert!(render_markdown("![alt](https://example.com/a.png)").is_err());
        assert!(render_markdown("[bad](javascript:alert(1))").is_err());
    }

    #[test]
    fn preserves_supported_structural_details() {
        let ordered = render_markdown("3. Third\n4. Fourth").unwrap();
        assert!(ordered.html.contains("<ol start=\"3\">"));

        let aligned_table =
            render_markdown("| Left | Center | Right |\n| :--- | :---: | ---: |\n| A | B | C |")
                .unwrap();
        assert!(aligned_table.html.contains("align=\"left\""));
        assert!(aligned_table.html.contains("align=\"center\""));
        assert!(aligned_table.html.contains("align=\"right\""));

        let table_break = render_markdown("| Line |\n| --- |\n| first<br>second |").unwrap();
        assert!(table_break.html.contains("first<br"));
        assert!(table_break.html.contains("second"));
        assert!(render_markdown("text <br class=\"unsafe\"> more").is_err());
        assert!(render_markdown("text <span>unsafe</span>").is_err());
    }

    #[test]
    fn plain_text_projection_preserves_structure_without_spurious_blank_lines() {
        let list = render_markdown("- first\n- second\n\n- [x] done\n- [ ] pending").unwrap();
        assert_eq!(list.plain_text, "first\nsecond\n[x] done\n[ ] pending");

        let table = render_markdown("| A | B |\n| --- | --- |\n| one | two |").unwrap();
        assert_eq!(table.plain_text, "A\tB\none\ttwo");
    }

    #[test]
    fn nested_task_lists_retain_normal_list_structure() {
        let output = render_markdown(
            "- [ ] Parent with **formatting**\n  - [x] Nested task\n  - [ ] Nested multiline  \n    continuation",
        )
        .unwrap();

        assert!(output.html.contains("Parent with <strong>formatting</strong>"));
        assert!(output.html.contains("<ul>\n<li><input type=\"checkbox\" checked=\"\" disabled=\"\"> Nested task</li>"));
        assert!(output.html.contains("Nested multiline<br>\ncontinuation"));
        assert_eq!(
            output.plain_text,
            "[ ] Parent with formatting\n[x] Nested task\n[ ] Nested multiline\ncontinuation"
        );
    }
}
