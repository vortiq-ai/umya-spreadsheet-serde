//! Just enough XML to read the chart parts Excel and umya write: elements,
//! attributes, text, namespaces and the predefined and numeric entities. What
//! it reports is what Python's ElementTree reports for the same bytes, because
//! the measured projections were taken from that.

pub const CHART: &str = "http://schemas.openxmlformats.org/drawingml/2006/chart";
pub const DRAWING: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const XML: &str = "http://www.w3.org/XML/1998/namespace";

#[derive(Debug)]
pub struct Element {
    pub namespace:  String,
    pub name:       String,
    /// A namespaced key is spelled `{uri}name`, an unprefixed one `name`.
    pub attributes: Vec<(String, String)>,
    /// The text before the first child element.
    pub text:       String,
    pub children:   Vec<Element>,
}

impl Element {
    pub fn attribute(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }

    pub fn is(&self, namespace: &str, name: &str) -> bool {
        self.namespace == namespace && self.name == name
    }

    /// The first child `c:<name>`.
    pub fn c(&self, name: &str) -> Option<&Element> {
        self.children.iter().find(|child| child.is(CHART, name))
    }

    /// The first child `a:<name>`.
    pub fn a(&self, name: &str) -> Option<&Element> {
        self.children.iter().find(|child| child.is(DRAWING, name))
    }

    /// Every child `c:<name>`, in document order.
    pub fn all_c<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Element> {
        self.children
            .iter()
            .filter(move |child| child.is(CHART, name))
    }

    /// The first descendant `c:<name>` in document order, the element itself
    /// excluded.
    pub fn descendant_c(&self, name: &str) -> Option<&Element> {
        self.children.iter().find_map(|child| {
            if child.is(CHART, name) {
                Some(child)
            } else {
                child.descendant_c(name)
            }
        })
    }

    /// The element and its descendants named `<namespace>:<name>`, in document
    /// order.
    pub fn all_named(&self, namespace: &str, name: &str) -> Vec<&Element> {
        let mut found = Vec::new();
        self.collect_named(namespace, name, &mut found);
        found
    }

    fn collect_named<'a>(&'a self, namespace: &str, name: &str, found: &mut Vec<&'a Element>) {
        if self.is(namespace, name) {
            found.push(self);
        }
        for child in &self.children {
            child.collect_named(namespace, name, found);
        }
    }
}

pub fn parse(xml: &str) -> Element {
    let mut parser = Parser {
        input:      xml,
        position:   0,
        namespaces: vec![("xml".to_owned(), XML.to_owned())],
    };
    parser.skip_whitespace();
    if parser.starts_with("<?") {
        parser.skip_past("?>");
        parser.skip_whitespace();
    }
    parser.element()
}

struct Parser<'a> {
    input:      &'a str,
    position:   usize,
    /// `(prefix, uri)` in scope, innermost last; the default namespace has the
    /// empty prefix.
    namespaces: Vec<(String, String)>,
}

impl<'a> Parser<'a> {
    fn rest(&self) -> &'a str {
        &self.input[self.position..]
    }

    fn starts_with(&self, prefix: &str) -> bool {
        self.rest().starts_with(prefix)
    }

    fn skip_whitespace(&mut self) {
        self.position = self.input.len() - self.rest().trim_start().len();
    }

    fn skip_past(&mut self, terminator: &str) {
        let end = self
            .rest()
            .find(terminator)
            .unwrap_or_else(|| panic!("no {terminator} after byte {}", self.position));
        self.position += end + terminator.len();
    }

    fn expect(&mut self, token: &str) {
        assert!(
            self.starts_with(token),
            "expected {token} at byte {}",
            self.position
        );
        self.position += token.len();
    }

    fn name(&mut self) -> &'a str {
        let rest = self.rest();
        let end = rest
            .find(|c: char| c.is_whitespace() || matches!(c, '/' | '>' | '='))
            .unwrap_or(rest.len());
        self.position += end;
        &rest[..end]
    }

    fn attribute_value(&mut self) -> String {
        let rest = self.rest();
        let quote = rest
            .chars()
            .next()
            .filter(|c| matches!(c, '"' | '\''))
            .unwrap_or_else(|| panic!("unquoted attribute at byte {}", self.position));
        let end = rest[1..]
            .find(quote)
            .unwrap_or_else(|| panic!("unterminated attribute at byte {}", self.position));
        self.position += end + 2;
        decode(&rest[1..=end], true)
    }

    fn element(&mut self) -> Element {
        self.expect("<");
        let qualified = self.name();
        let mut raw = Vec::new();
        let empty = loop {
            self.skip_whitespace();
            if self.starts_with("/>") {
                self.position += 2;
                break true;
            }
            if self.starts_with(">") {
                self.position += 1;
                break false;
            }
            let key = self.name();
            self.skip_whitespace();
            self.expect("=");
            self.skip_whitespace();
            raw.push((key, self.attribute_value()));
        };

        let scope = self.namespaces.len();
        for (key, value) in &raw {
            if *key == "xmlns" {
                self.namespaces.push((String::new(), value.clone()));
            } else if let Some(prefix) = key.strip_prefix("xmlns:") {
                self.namespaces.push((prefix.to_owned(), value.clone()));
            }
        }
        let (namespace, name) = self.resolve(qualified, true);
        let attributes = raw
            .iter()
            .filter(|(key, _)| *key != "xmlns" && !key.starts_with("xmlns:"))
            .map(|(key, value)| {
                let (namespace, name) = self.resolve(key, false);
                let key = if namespace.is_empty() {
                    name
                } else {
                    format!("{{{namespace}}}{name}")
                };
                (key, value.clone())
            })
            .collect();

        let mut text = String::new();
        let mut children = Vec::new();
        if !empty {
            loop {
                if self.starts_with("</") {
                    self.position += 2;
                    let closing = self.name();
                    assert_eq!(
                        closing, qualified,
                        "mismatched end tag at byte {}",
                        self.position
                    );
                    self.skip_whitespace();
                    self.expect(">");
                    break;
                } else if self.starts_with("<") {
                    children.push(self.element());
                } else {
                    let rest = self.rest();
                    let end = rest.find('<').unwrap_or_else(|| {
                        panic!("unterminated {qualified} at byte {}", self.position)
                    });
                    self.position += end;
                    if children.is_empty() {
                        text.push_str(&decode(&rest[..end], false));
                    }
                }
            }
        }
        self.namespaces.truncate(scope);
        Element {
            namespace,
            name,
            attributes,
            text,
            children,
        }
    }

    /// The namespace and local name of a qualified name; an unprefixed
    /// attribute has no namespace, an unprefixed element has the default one.
    fn resolve(&self, qualified: &str, default_applies: bool) -> (String, String) {
        let (prefix, name) = qualified.split_once(':').unwrap_or(("", qualified));
        if prefix.is_empty() && !default_applies {
            return (String::new(), name.to_owned());
        }
        let namespace = self
            .namespaces
            .iter()
            .rev()
            .find(|(declared, _)| declared == prefix)
            .map(|(_, uri)| uri.clone());
        match namespace {
            Some(namespace) => (namespace, name.to_owned()),
            None if prefix.is_empty() => (String::new(), name.to_owned()),
            None => panic!("unbound prefix {prefix}"),
        }
    }
}

/// Line ends become `\n`; in an attribute value every literal line end or tab
/// becomes a space; entity and character references are replaced.
fn decode(raw: &str, attribute: bool) -> String {
    let mut decoded = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '&' => {
                let reference: String = chars.by_ref().take_while(|c| *c != ';').collect();
                decoded.push(entity(&reference));
            }
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                decoded.push(if attribute { ' ' } else { '\n' });
            }
            '\n' | '\t' if attribute => decoded.push(' '),
            c => decoded.push(c),
        }
    }
    decoded
}

fn entity(reference: &str) -> char {
    let code = match reference {
        "amp" => return '&',
        "lt" => return '<',
        "gt" => return '>',
        "quot" => return '"',
        "apos" => return '\'',
        _ => {
            if let Some(hex) = reference.strip_prefix("#x") {
                u32::from_str_radix(hex, 16)
            } else if let Some(decimal) = reference.strip_prefix('#') {
                decimal.parse::<u32>()
            } else {
                panic!("unknown entity &{reference};")
            }
        }
    };
    code.ok()
        .and_then(char::from_u32)
        .unwrap_or_else(|| panic!("bad character reference &{reference};"))
}
