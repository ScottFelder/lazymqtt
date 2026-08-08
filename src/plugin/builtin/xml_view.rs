//! Built-in XML structured-view plugin.
//!
//! When the selected payload parses as reasonably well-formed XML, offers a
//! pretty-printed, syntax-colored view as an alternative to the raw text — the
//! XML counterpart to json-view. The raw view always stays available alongside.
//!
//! We hand-roll a small, display-oriented tokenizer (no XML dependency): tag
//! scanning is quote-aware so `>` inside an attribute value doesn't end a tag,
//! and comments / CDATA / processing instructions are recognized by their own
//! delimiters. Elements are re-indented by nesting depth; concatenating a
//! line's span texts reproduces that pretty rendering, so a yank stays valid.
//!
//! Coloring: angle brackets / slashes / `=` / quotes are punctuation, element
//! names are keys, attribute names are literals, attribute values and CDATA are
//! strings, and text / comments are plain.

use crate::plugin::api::{InspectMessage, InspectorSpan, InspectorStyle, InspectorView};
use crate::plugin::Plugin;
use crate::plugin::PluginMetadata;

pub struct XmlView;

impl Plugin for XmlView {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            name: "xml-view",
            version: "0.1.0",
            description: "pretty-prints XML payloads with syntax colors",
        }
    }

    fn inspect(&self, msg: &InspectMessage) -> Option<InspectorView> {
        let toks = tokenize(msg.payload.trim())?;
        // Must contain at least one element and be balanced (see `tokenize`).
        if !toks
            .iter()
            .any(|t| matches!(t, Token::Start(_) | Token::Empty(_)))
        {
            return None;
        }
        let mut fmt = Fmt::default();
        render(&mut fmt, &toks);
        fmt.flush();
        Some(InspectorView {
            label: "XML".to_string(),
            lines: fmt.lines,
        })
    }

    fn key_hints(&self) -> &'static [(&'static str, &'static str)] {
        &[("i", "view")]
    }

    fn help(&self) -> &'static [(&'static str, &'static str)] {
        &[
            (
                "i",
                "cycle the Payload pane through raw text and this XML view",
            ),
            (
                "",
                "Pretty-prints and syntax-colors well-formed XML payloads.",
            ),
        ]
    }
}

/// One lexical piece of an XML document. Tag variants carry the raw inner text
/// (between the angle brackets), parsed for coloring at render time.
#[derive(Debug, PartialEq)]
enum Token {
    Decl(String),    // <?xml ... ?> or other processing instruction
    Comment(String), // <!-- ... -->
    CData(String),   // <![CDATA[ ... ]]>
    Start(String),   // <name attrs>          (inner = "name attrs")
    Empty(String),   // <name attrs/>         (inner = "name attrs")
    End(String),     // </name>               (inner = "name")
    Text(String),    // between tags
}

/// Tokenize `s`, returning `None` when it doesn't look like well-formed XML:
/// tags must close, and start/end tags must nest correctly. Deliberately
/// lenient about content (entities, namespaces) since this only drives display.
fn tokenize(s: &str) -> Option<Vec<Token>> {
    if !s.starts_with('<') {
        return None;
    }
    let bytes = s.as_bytes();
    let mut i = 0;
    let mut out = Vec::new();
    let mut stack: Vec<String> = Vec::new();

    while i < bytes.len() {
        if bytes[i] == b'<' {
            let rest = &s[i..];
            if let Some(inner) = delimited(rest, "<!--", "-->") {
                out.push(Token::Comment(inner.to_string()));
                i += "<!--".len() + inner.len() + "-->".len();
            } else if let Some(inner) = delimited(rest, "<![CDATA[", "]]>") {
                out.push(Token::CData(inner.to_string()));
                i += "<![CDATA[".len() + inner.len() + "]]>".len();
            } else if rest.starts_with("<?") {
                let inner = delimited(rest, "<?", "?>")?;
                out.push(Token::Decl(inner.to_string()));
                i += "<?".len() + inner.len() + "?>".len();
            } else if rest.starts_with("<!") {
                // DOCTYPE and similar: treat like a declaration up to '>'.
                let end = find_tag_end(s, i)?;
                out.push(Token::Decl(s[i + 1..end].to_string()));
                i = end + 1;
            } else {
                let end = find_tag_end(s, i)?;
                let mut inner = &s[i + 1..end];
                if let Some(name) = inner.strip_prefix('/') {
                    let name = name.trim().to_string();
                    // End tag must match the most recent open element.
                    if stack.pop().as_deref() != Some(name.as_str()) {
                        return None;
                    }
                    out.push(Token::End(name));
                } else if let Some(rest_inner) = inner.strip_suffix('/') {
                    inner = rest_inner;
                    out.push(Token::Empty(inner.trim().to_string()));
                } else {
                    stack.push(element_name(inner).to_string());
                    out.push(Token::Start(inner.trim().to_string()));
                }
                i = end + 1;
            }
        } else {
            let next = s[i..].find('<').map(|p| i + p).unwrap_or(bytes.len());
            let text = &s[i..next];
            if !text.trim().is_empty() {
                out.push(Token::Text(text.trim().to_string()));
            }
            i = next;
        }
    }

    // Every opened element must have been closed.
    if stack.is_empty() {
        Some(out)
    } else {
        None
    }
}

/// If `s` starts with `open`, return the slice up to the next `close` (not
/// including either delimiter). `None` if it doesn't start with `open` or has no
/// matching `close`.
fn delimited<'a>(s: &'a str, open: &str, close: &str) -> Option<&'a str> {
    let after = s.strip_prefix(open)?;
    let end = after.find(close)?;
    Some(&after[..end])
}

/// Index of the `>` that closes the tag starting at `from` (a `<`), skipping any
/// `>` inside single- or double-quoted attribute values.
fn find_tag_end(s: &str, from: usize) -> Option<usize> {
    let bytes = s.as_bytes();
    let mut i = from + 1;
    let mut quote = 0u8;
    while i < bytes.len() {
        let c = bytes[i];
        if quote != 0 {
            if c == quote {
                quote = 0;
            }
        } else if c == b'"' || c == b'\'' {
            quote = c;
        } else if c == b'>' {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// The element name at the start of a tag's inner text (up to whitespace).
fn element_name(inner: &str) -> &str {
    inner
        .trim_start()
        .split([' ', '\t', '\n', '\r'])
        .next()
        .unwrap_or("")
}

/// Render tokens into styled, depth-indented lines.
fn render(f: &mut Fmt, toks: &[Token]) {
    let mut depth: usize = 0;
    for tok in toks {
        match tok {
            Token::Start(inner) => {
                f.indent(depth);
                write_tag(f, inner, "<", ">");
                f.newline();
                depth += 1;
            }
            Token::End(name) => {
                depth = depth.saturating_sub(1);
                f.indent(depth);
                f.push("</", InspectorStyle::Punctuation);
                f.push(name.clone(), InspectorStyle::Key);
                f.push(">", InspectorStyle::Punctuation);
                f.newline();
            }
            Token::Empty(inner) => {
                f.indent(depth);
                write_tag(f, inner, "<", "/>");
                f.newline();
            }
            Token::Text(text) => {
                f.indent(depth);
                f.push(text.clone(), InspectorStyle::Plain);
                f.newline();
            }
            Token::Comment(inner) => {
                f.indent(depth);
                f.push(format!("<!--{inner}-->"), InspectorStyle::Plain);
                f.newline();
            }
            Token::CData(inner) => {
                f.indent(depth);
                f.push("<![CDATA[", InspectorStyle::Punctuation);
                f.push(inner.clone(), InspectorStyle::Str);
                f.push("]]>", InspectorStyle::Punctuation);
                f.newline();
            }
            Token::Decl(inner) => {
                f.indent(depth);
                f.push(format!("<?{inner}?>"), InspectorStyle::Plain);
                f.newline();
            }
        }
    }
}

/// Write a start/empty tag with `open`/`close` brackets, coloring the element
/// name and its attributes.
fn write_tag(f: &mut Fmt, inner: &str, open: &str, close: &str) {
    f.push(open, InspectorStyle::Punctuation);
    let name = element_name(inner);
    f.push(name.to_string(), InspectorStyle::Key);
    write_attrs(f, inner[name.len()..].trim_start());
    f.push(close, InspectorStyle::Punctuation);
}

/// Color the attribute list `name="value" other='v'` of a tag. Lenient: any
/// leftover fragment is emitted as plain text rather than dropped.
fn write_attrs(f: &mut Fmt, mut rest: &str) {
    while !rest.is_empty() {
        // Attribute name up to '=' or whitespace.
        let name_end = rest
            .find(['=', ' ', '\t', '\n', '\r'])
            .unwrap_or(rest.len());
        let name = &rest[..name_end];
        if !name.is_empty() {
            f.push(" ", InspectorStyle::Punctuation);
            f.push(name.to_string(), InspectorStyle::Literal);
        }
        rest = rest[name_end..].trim_start();
        if let Some(after_eq) = rest.strip_prefix('=') {
            f.push("=", InspectorStyle::Punctuation);
            rest = after_eq.trim_start();
            if let Some(quote) = rest.chars().next().filter(|c| *c == '"' || *c == '\'') {
                if let Some(close) = rest[1..].find(quote) {
                    let value = &rest[..close + 2]; // include both quotes
                    f.push(value.to_string(), InspectorStyle::Str);
                    rest = rest[close + 2..].trim_start();
                    continue;
                }
            }
            // Unquoted or malformed value: take the next token.
            let end = rest.find([' ', '\t', '\n', '\r']).unwrap_or(rest.len());
            if end > 0 {
                f.push(rest[..end].to_string(), InspectorStyle::Str);
            }
            rest = rest[end..].trim_start();
        } else if name.is_empty() {
            // No progress possible; emit the remainder plainly and stop.
            f.push(format!(" {rest}"), InspectorStyle::Plain);
            break;
        }
    }
}

/// Accumulates styled spans into lines (mirrors json-view's `Fmt`).
#[derive(Default)]
struct Fmt {
    lines: Vec<Vec<InspectorSpan>>,
    current: Vec<InspectorSpan>,
}

impl Fmt {
    fn push(&mut self, text: impl Into<String>, style: InspectorStyle) {
        self.current.push(InspectorSpan::new(text, style));
    }

    fn newline(&mut self) {
        self.lines.push(std::mem::take(&mut self.current));
    }

    fn flush(&mut self) {
        if !self.current.is_empty() {
            self.newline();
        }
    }

    fn indent(&mut self, level: usize) {
        if level > 0 {
            self.push("  ".repeat(level), InspectorStyle::Punctuation);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inspect(payload: &str) -> Option<InspectorView> {
        XmlView.inspect(&InspectMessage {
            topic: "t".into(),
            payload: payload.into(),
            payload_raw: payload.as_bytes().to_vec(),
            qos: 0,
            retained: false,
        })
    }

    fn line_text(line: &[InspectorSpan]) -> String {
        line.iter().map(|s| s.text.as_str()).collect()
    }

    #[test]
    fn pretty_prints_nested_xml() {
        let view = inspect("<a><b>hi</b><c x=\"1\"/></a>").expect("valid xml yields a view");
        assert_eq!(view.label, "XML");
        let text: Vec<String> = view.lines.iter().map(|l| line_text(l)).collect();
        assert_eq!(text[0], "<a>");
        assert_eq!(text[1], "  <b>");
        assert_eq!(text[2], "    hi");
        assert_eq!(text[3], "  </b>");
        assert_eq!(text[4], "  <c x=\"1\"/>");
        assert_eq!(text[5], "</a>");
    }

    #[test]
    fn colors_tokens() {
        let view = inspect("<root attr=\"v\">text</root>").unwrap();
        let spans: Vec<&InspectorSpan> = view.lines.iter().flatten().collect();
        let has = |t: &str, s: InspectorStyle| spans.iter().any(|sp| sp.text == t && sp.style == s);
        assert!(has("<", InspectorStyle::Punctuation));
        assert!(has("root", InspectorStyle::Key));
        assert!(has("attr", InspectorStyle::Literal));
        assert!(has("\"v\"", InspectorStyle::Str));
        assert!(has("text", InspectorStyle::Plain));
        assert!(has("</", InspectorStyle::Punctuation));
    }

    #[test]
    fn quote_aware_tag_scan() {
        // A '>' inside an attribute value must not end the tag early.
        let view = inspect("<a b=\"x>y\">t</a>").expect("gt in attribute is fine");
        let text: Vec<String> = view.lines.iter().map(|l| line_text(l)).collect();
        assert_eq!(text[0], "<a b=\"x>y\">");
    }

    #[test]
    fn rejects_non_xml_and_unbalanced() {
        assert!(inspect("just text").is_none());
        assert!(inspect("").is_none());
        assert!(inspect(r#"{"a":1}"#).is_none()); // JSON is not XML
        assert!(inspect("<a><b></a>").is_none()); // mismatched close
        assert!(inspect("<a>").is_none()); // never closed
    }
}
