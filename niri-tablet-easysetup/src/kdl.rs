//! A small, tolerant KDL reader/writer covering just the shapes that appear in
//! niri gesture configs: nodes with optional `prop=value` properties, string
//! or bare-word arguments, `//` comments, and `{ ... }` child blocks.
//!
//! This is deliberately NOT a general KDL implementation. Anything it cannot
//! understand makes `parse_document` fail; callers treat that as "show a
//! banner, refuse to rewrite the file". Unmodeled but well-formed nodes are
//! round-tripped structurally (comments inside them are not preserved).

/// A scalar value. `Str` was quoted in the source (and is re-quoted on
/// render); `Word` was bare (numbers, `true`, identifiers) and is rendered
/// verbatim.
#[derive(Clone, PartialEq, Debug)]
pub enum Arg {
    Str(String),
    Word(String),
}

impl Arg {
    /// Unquoted content, for display.
    pub fn text(&self) -> &str {
        match self {
            Arg::Str(s) | Arg::Word(s) => s,
        }
    }

    /// KDL source form: quoted+escaped, or verbatim.
    pub fn render(&self) -> String {
        match self {
            Arg::Word(w) => w.clone(),
            Arg::Str(s) => quote(s),
        }
    }

    /// Best-effort constructor for user-typed text: bare when it is a valid
    /// bare token (number/bool/plain identifier), quoted otherwise.
    pub fn from_user_text(text: &str) -> Arg {
        if is_bare_token(text) {
            Arg::Word(text.to_string())
        } else {
            Arg::Str(text.to_string())
        }
    }
}

/// True for things KDL accepts unquoted: numbers, booleans, identifiers.
fn is_bare_token(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '+' | '%' | '/'))
        && s.chars().next().is_some_and(|c| !c.is_ascii_digit() || s.parse::<f64>().is_ok() || s.chars().all(|c| c.is_ascii_digit()))
}

pub fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

/// One KDL node: `name prop=value* arg* { children }`.
#[derive(Clone, PartialEq, Debug)]
pub struct Node {
    pub name: String,
    pub props: Vec<(String, Arg)>,
    pub args: Vec<Arg>,
    pub children: Vec<Node>,
}

impl Node {
    pub fn new(name: &str) -> Node {
        Node {
            name: name.to_string(),
            props: Vec::new(),
            args: Vec::new(),
            children: Vec::new(),
        }
    }

    #[cfg(test)]
    pub fn find(&self, name: &str) -> Option<&Node> {
        self.children.iter().find(|n| n.name == name)
    }

    pub fn arg_str(mut self, s: &str) -> Node {
        self.args.push(Arg::Str(s.to_string()));
        self
    }

    /// Render this node (and children) at the given indent level.
    pub fn render(&self, indent: usize) -> String {
        let pad = "    ".repeat(indent);
        let mut line = format!("{pad}{}", self.name);
        for (k, v) in &self.props {
            line.push_str(&format!(" {k}={}", v.render()));
        }
        for a in &self.args {
            line.push(' ');
            line.push_str(&a.render());
        }
        if self.children.is_empty() {
            line.push(';');
            line
        } else if self.children.iter().all(|c| c.children.is_empty()) {
            // Leaf-only children go on one line: `tap { maximize-column; }`,
            // matching the style of the shipped config.
            let kids: Vec<String> = self
                .children
                .iter()
                .map(|c| c.render(0).trim_start().to_string())
                .collect();
            format!("{line} {{ {} }}", kids.join(" "))
        } else {
            let mut out = line;
            out.push_str(" {\n");
            for c in &self.children {
                out.push_str(&c.render(indent + 1));
                out.push('\n');
            }
            out.push_str(&format!("{pad}}}"));
            out
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    pub msg: String,
    pub line: usize,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line {}: {}", self.line, self.msg)
    }
}

impl std::error::Error for ParseError {}

#[derive(Clone, PartialEq, Debug)]
enum Tok {
    Word(String),
    Str(String),
    LBrace,
    RBrace,
    Semi,
    Equals,
    Newline,
}

fn lex(src: &str) -> Result<Vec<(Tok, usize)>, ParseError> {
    let b: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    let mut i = 0usize;
    let mut line = 1usize;

    let err = |line: usize, msg: &str| ParseError {
        msg: msg.to_string(),
        line,
    };

    while i < b.len() {
        let c = b[i];
        match c {
            ' ' | '\t' | '\r' => i += 1,
            '\n' => {
                if !matches!(out.last().map(|(t, _)| t), Some(Tok::Newline)) {
                    out.push((Tok::Newline, i));
                }
                line += 1;
                i += 1;
            }
            '/' if i + 1 < b.len() && b[i + 1] == '/' => {
                while i < b.len() && b[i] != '\n' {
                    i += 1;
                }
            }
            '{' => {
                out.push((Tok::LBrace, i));
                i += 1;
            }
            '}' => {
                out.push((Tok::RBrace, i));
                i += 1;
            }
            ';' => {
                out.push((Tok::Semi, i));
                i += 1;
            }
            '=' => {
                out.push((Tok::Equals, i));
                i += 1;
            }
            '"' => {
                let start_line = line;
                i += 1;
                let mut s = String::new();
                let mut closed = false;
                while i < b.len() {
                    let c = b[i];
                    if c == '"' {
                        closed = true;
                        i += 1;
                        break;
                    }
                    if c == '\n' {
                        return Err(err(start_line, "unterminated string"));
                    }
                    if c == '\\' {
                        i += 1;
                        let Some(&e) = b.get(i) else {
                            return Err(err(start_line, "unterminated escape in string"));
                        };
                        match e {
                            '"' => s.push('"'),
                            '\\' => s.push('\\'),
                            'n' => s.push('\n'),
                            't' => s.push('\t'),
                            'r' => s.push('\r'),
                            'u' => {
                                // \u{XXXX}: keep verbatim; re-quoted on render.
                                s.push_str("\\u");
                            }
                            other => {
                                return Err(err(
                                    start_line,
                                    &format!("unknown escape \\{other} in string"),
                                ))
                            }
                        }
                        i += 1;
                    } else {
                        s.push(c);
                        i += 1;
                    }
                }
                if !closed {
                    return Err(err(start_line, "unterminated string"));
                }
                out.push((Tok::Str(s), i));
            }
            c if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '+' | '%' | '~' | '@' | ':' | '#') => {
                let mut w = String::new();
                while i < b.len() {
                    let c = b[i];
                    if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '+' | '%' | '/' | '~' | '@' | ':' | '#') {
                        w.push(c);
                        i += 1;
                    } else {
                        break;
                    }
                }
                out.push((Tok::Word(w), i));
            }
            other => {
                return Err(err(line, &format!("unexpected character `{other}`")));
            }
        }
    }
    Ok(out)
}

/// Parse a document into its top-level nodes.
pub fn parse_document(src: &str) -> Result<Vec<Node>, ParseError> {
    let toks = lex(src)?;
    let mut pos = 0usize;
    let nodes = parse_nodes(&toks, &mut pos, false)?;
    Ok(nodes)
}

// Parse sibling nodes until EOF (top) or `}` (inside a block, which is
// consumed by the caller).
fn parse_nodes(toks: &[(Tok, usize)], pos: &mut usize, in_block: bool) -> Result<Vec<Node>, ParseError> {
    let mut nodes = Vec::new();
    loop {
        // Skip separators.
        while matches!(toks.get(*pos).map(|(t, _)| t), Some(Tok::Newline) | Some(Tok::Semi)) {
            *pos += 1;
        }
        match toks.get(*pos).map(|(t, _)| t) {
            None => {
                if in_block {
                    return Err(ParseError {
                        msg: "unexpected end of file, missing `}`".into(),
                        line: 9999,
                    });
                }
                return Ok(nodes);
            }
            Some(Tok::RBrace) if in_block => {
                *pos += 1;
                return Ok(nodes);
            }
            Some(Tok::RBrace) => {
                return Err(ParseError {
                    msg: "unexpected `}`".into(),
                    line: 1 + toks[..*pos].iter().filter(|(t, _)| matches!(t, Tok::Newline)).count(),
                });
            }
            Some(Tok::Word(_)) => {
                let node = parse_node(toks, pos)?;
                nodes.push(node);
            }
            Some(other) => {
                return Err(ParseError {
                    msg: format!("expected a node name, found {other:?}"),
                    line: 1 + toks[..*pos].iter().filter(|(t, _)| matches!(t, Tok::Newline)).count(),
                });
            }
        }
    }
}

fn parse_node(toks: &[(Tok, usize)], pos: &mut usize) -> Result<Node, ParseError> {
    let line = 1 + toks[..*pos].iter().filter(|(t, _)| matches!(t, Tok::Newline)).count();
    let Some(Tok::Word(name)) = toks.get(*pos).map(|(t, _)| t) else {
        return Err(ParseError {
            msg: "expected a node name".into(),
            line,
        });
    };
    let name = name.clone();
    *pos += 1;

    let mut node = Node::new(&name);
    loop {
        match toks.get(*pos).map(|(t, _)| t) {
            Some(Tok::Word(w)) => {
                // `key = value` property, or a positional argument.
                if matches!(toks.get(*pos + 1).map(|(t, _)| t), Some(Tok::Equals)) {
                    let key = w.clone();
                    *pos += 2;
                    let value = match toks.get(*pos).map(|(t, _)| t) {
                        Some(Tok::Word(w)) => {
                            let w = w.clone();
                            *pos += 1;
                            Arg::Word(w)
                        }
                        Some(Tok::Str(s)) => {
                            let s = s.clone();
                            *pos += 1;
                            Arg::Str(s)
                        }
                        _ => {
                            return Err(ParseError {
                                msg: format!("property `{key}` has no value"),
                                line,
                            })
                        }
                    };
                    node.props.push((key, value));
                } else {
                    let w = w.clone();
                    *pos += 1;
                    node.args.push(Arg::Word(w));
                }
            }
            Some(Tok::Str(s)) => {
                let s = s.clone();
                *pos += 1;
                node.args.push(Arg::Str(s));
            }
            Some(Tok::LBrace) => {
                *pos += 1;
                node.children = parse_nodes(toks, pos, true)?;
                // Optional trailing separators are skipped by the caller.
                return Ok(node);
            }
            Some(Tok::Semi) => {
                *pos += 1;
                return Ok(node);
            }
            Some(Tok::Newline) => {
                // Lenient multiline header: a bare node followed (across
                // newlines) by `{` takes the block as its children.
                let mut peek = *pos;
                while matches!(toks.get(peek).map(|(t, _)| t), Some(Tok::Newline)) {
                    peek += 1;
                }
                if matches!(toks.get(peek).map(|(t, _)| t), Some(Tok::LBrace))
                    && node.args.is_empty()
                    && node.props.is_empty()
                {
                    *pos = peek + 1;
                    node.children = parse_nodes(toks, pos, true)?;
                    return Ok(node);
                }
                *pos += 1;
                return Ok(node);
            }
            _ => {
                return Err(ParseError {
                    msg: format!("unexpected end of node `{name}`"),
                    line,
                })
            }
        }
    }
}

/// Convenience: find the first top-level node with the given name.
pub fn find_top_level<'a>(nodes: &'a [Node], name: &str) -> Option<&'a Node> {
    nodes.iter().find(|n| n.name == name)
}

/// Find the source span (byte range) of the first top-level node with the
/// given name, so it can be cut out of the file without touching anything
/// else (comments included). The range starts at the node name and ends just
/// after the closing `}` (or after a `;`/line end for childless nodes).
///
/// This is a small independent scanner: brace-depth + string + `//` comment
/// aware, like the lexer, but offset-reporting.
pub fn find_top_level_node_span(src: &str, name: &str) -> Option<(usize, usize)> {
    let b: Vec<char> = src.chars().collect();
    // Byte offset of each char index, so we can report byte ranges.
    let mut i = 0usize;
    let mut depth: i32 = 0;

    while i < b.len() {
        let c = b[i];
        match c {
            '/' if i + 1 < b.len() && b[i + 1] == '/' => {
                while i < b.len() && b[i] != '\n' {
                    i += 1;
                }
            }
            '"' => {
                // Skip the string; strings can't contain raw newlines here.
                i += 1;
                while i < b.len() && b[i] != '"' {
                    if b[i] == '\\' {
                        i += 1;
                    }
                    i += 1;
                }
                i += 1;
            }
            '{' => {
                depth += 1;
                i += 1;
            }
            '}' => {
                depth -= 1;
                i += 1;
            }
            c if depth == 0 && (c.is_ascii_alphabetic() || c == '_') => {
                let start = i;
                let mut j = i;
                while j < b.len() && (b[j].is_ascii_alphanumeric() || b[j] == '_' || b[j] == '-') {
                    j += 1;
                }
                let word: String = b[start..j].iter().collect();
                i = j;
                if word == name {
                    // The block must open on the same line (only spaces or
                    // tabs in between), so an argument can never look like a
                    // node header for a later block.
                    let mut k = i;
                    while k < b.len() && (b[k] == ' ' || b[k] == '\t') {
                        k += 1;
                    }
                    if k < b.len() && b[k] == '{' {
                        // Find the matching close brace.
                        let mut d = 0i32;
                        let mut m = k;
                        while m < b.len() {
                            match b[m] {
                                '/' if m + 1 < b.len() && b[m + 1] == '/' => {
                                    while m < b.len() && b[m] != '\n' {
                                        m += 1;
                                    }
                                }
                                '"' => {
                                    m += 1;
                                    while m < b.len() && b[m] != '"' {
                                        if b[m] == '\\' {
                                            m += 1;
                                        }
                                        m += 1;
                                    }
                                }
                                '{' => d += 1,
                                '}' => {
                                    d -= 1;
                                    if d == 0 {
                                        // Include a trailing `;` if present.
                                        let mut end = m + 1;
                                        if end < b.len() && b[end] == ';' {
                                            end += 1;
                                        }
                                        // Expand to end of line.
                                        while end < b.len() && b[end] != '\n' {
                                            end += 1;
                                        }
                                        let char_range = start..end;
                                        let byte_start = char_to_byte(src, char_range.start);
                                        let byte_end = char_to_byte(src, char_range.end);
                                        return Some((byte_start, byte_end));
                                    }
                                }
                                _ => {}
                            }
                            m += 1;
                        }
                    }
                }
            }
            _ => i += 1,
        }
    }
    None
}

fn char_to_byte(src: &str, char_idx: usize) -> usize {
    src.char_indices()
        .nth(char_idx)
        .map(|(b, _)| b)
        .unwrap_or(src.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_shipped_shapes() {
        let src = r#"
// comment
gestures {
    show-touch-points "gestures"
    // debug-log
    touchscreen-swipe {
        fingers 3
        horizontal-swipe "resize-column"
        tap { maximize-column; }
        hold {
            left { move-column-left; }
        }
    }
    touchscreen-edge-swipe {
        bottom { spawn-sh "~/.local/bin/niri-osk.sh"; }
    }
}
"#;
        let doc = parse_document(src).unwrap();
        let g = find_top_level(&doc, "gestures").unwrap();
        assert_eq!(
            g.find("show-touch-points").unwrap().args[0],
            Arg::Str("gestures".into())
        );
        let ts = g.find("touchscreen-swipe").unwrap();
        assert_eq!(ts.find("fingers").unwrap().args[0], Arg::Word("3".into()));
        let tap = ts.find("tap").unwrap();
        assert_eq!(tap.children.len(), 1);
        assert_eq!(tap.children[0].name, "maximize-column");
        let hold = ts.find("hold").unwrap();
        assert_eq!(hold.find("left").unwrap().children[0].name, "move-column-left");
        let edge = g.find("touchscreen-edge-swipe").unwrap();
        let bottom = edge.find("bottom").unwrap();
        assert_eq!(
            bottom.children[0].args[0],
            Arg::Str("~/.local/bin/niri-osk.sh".into())
        );
    }

    #[test]
    fn parses_props_and_multiline_nodes() {
        let src = "include \"cfg/gestures.kdl\" optional=true\ntap\n{\n  quit skip-confirmation=true\n}\n";
        let doc = parse_document(src).unwrap();
        let inc = &doc[0];
        assert_eq!(inc.name, "include");
        assert_eq!(inc.args[0], Arg::Str("cfg/gestures.kdl".into()));
        assert_eq!(inc.props[0].0, "optional");
        let tap = &doc[1];
        assert_eq!(tap.name, "tap");
        let quit = &tap.children[0];
        assert_eq!(quit.props[0], ("skip-confirmation".into(), Arg::Word("true".into())));
    }

    #[test]
    fn strings_with_escapes_round_trip() {
        let src = "bottom { spawn-sh \"echo \\\"hi\\\\there\\\"\"; }";
        let doc = parse_document(src).unwrap();
        let s = match &doc[0].children[0].args[0] {
            Arg::Str(s) => s.clone(),
            other => panic!("{other:?}"),
        };
        assert_eq!(s, "echo \"hi\\there\"");
        assert_eq!(Node::render(&doc[0], 0), src);
    }

    #[test]
    fn render_is_stable() {
        // Canonical form: leaf-only children inline, `;` after every leaf.
        let src = "gestures {\n    show-touch-points \"gestures\";\n    touchscreen-swipe {\n        fingers 3;\n        tap { maximize-column; }\n    }\n}\n";
        let doc = parse_document(src).unwrap();
        let rendered: String = doc.iter().map(|n| n.render(0)).collect::<Vec<_>>().join("\n") + "\n";
        assert_eq!(rendered, src);
        // And the shipped style (no trailing `;` on some lines) parses to
        // the same document.
        let shipped_style = "gestures {\n    show-touch-points \"gestures\"\n    touchscreen-swipe {\n        fingers 3\n        tap { maximize-column; }\n    }\n}\n";
        let doc2 = parse_document(shipped_style).unwrap();
        assert_eq!(doc, doc2);
    }

    #[test]
    fn errors_on_garbage() {
        assert!(parse_document("gestures { tap {").is_err());
        assert!(parse_document("foo \"unterminated").is_err());
        assert!(parse_document("}").is_err());
    }

    #[test]
    fn finds_top_level_span_ignoring_nested_and_strings() {
        let src = "// intro comment\nspawn-at-startup \"waybar\"\n\n// gestures live here\ngestures {\n    touchscreen-swipe {\n        tap { maximize-column; }\n    }\n    // } in comment\n    bottom { spawn-sh \"}\"; }\n}\n\ninclude \"x.kdl\"\n";
        let (start, end) = find_top_level_node_span(src, "gestures").unwrap();
        let cut = &src[start..end];
        assert!(cut.starts_with("gestures {"), "{cut:?}");
        assert!(cut.trim_end().ends_with('}'), "{cut:?}");
        // The cut is a standalone gestures node (string/comment braces like
        // "}" and "// }" must not confuse the scanner).
        let cut_doc = parse_document(cut).expect("cut parses standalone");
        assert!(find_top_level(&cut_doc, "gestures").is_some());
        // Cutting it out leaves the rest intact.
        let remainder = format!("{}{}", &src[..start], &src[end..]);
        assert!(remainder.contains("intro comment"));
        assert!(remainder.contains("include \"x.kdl\""));
        let rem_doc = parse_document(&remainder).expect("remainder parses");
        assert!(find_top_level(&rem_doc, "gestures").is_none());
    }

    #[test]
    fn span_scanner_ignores_siblings_with_same_prefix() {
        let src = "gestures-export \"x\"\ngestures { tap { maximize-column; } }\n";
        let (start, _) = find_top_level_node_span(src, "gestures").unwrap();
        assert!(src[start..].starts_with("gestures {"));
    }
}
