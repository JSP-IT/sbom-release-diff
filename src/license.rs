//! Declared-license normalization for change detection.
//!
//! This module only decides whether two declarations are written the same
//! way after syntactic normalization. It does not interpret licenses, look
//! up the SPDX list or judge compatibility.

/// Longest license expression that is parsed; longer ones are compared as
/// normalized text.
pub const MAX_EXPRESSION_LEN: usize = 4096;
const MAX_PAREN_DEPTH: usize = 32;

/// One declared license entry from a component's `licenses` array.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct LicenseEntry {
    /// Comparison key. Two entries with the same key are treated as the
    /// same declaration.
    pub canonical: String,
    /// The declaration as written in the SBOM, for display.
    pub display: String,
    /// False when an expression could not be parsed and is compared as
    /// whitespace-normalized text instead.
    pub parsed: bool,
}

impl LicenseEntry {
    /// `license.id`: SPDX identifiers are compared case-insensitively. An id
    /// that is not a single identifier (for example one containing spaces or
    /// operators) gets its own key, so it cannot equal a parsed expression.
    pub fn from_id(id: &str) -> Self {
        let display = id.trim().to_owned();
        let canonical = if is_valid_id(&display) {
            format!("expr:{}", display.to_ascii_lowercase())
        } else {
            format!("id:{display}")
        };
        Self {
            canonical,
            display,
            parsed: true,
        }
    }

    /// `license.name`: free text, compared exactly after trimming and
    /// collapsing whitespace.
    pub fn from_name(name: &str) -> Self {
        let display = collapse_whitespace(name);
        Self {
            canonical: format!("name:{display}"),
            display,
            parsed: true,
        }
    }

    /// `expression`: an SPDX license expression.
    pub fn from_expression(expression: &str) -> Self {
        let display = collapse_whitespace(expression);
        match canonicalize_expression(&display) {
            Some(canonical) => Self {
                canonical: format!("expr:{canonical}"),
                display,
                parsed: true,
            },
            None => Self {
                canonical: format!("text:{display}"),
                display,
                parsed: false,
            },
        }
    }
}

fn collapse_whitespace(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Returns a canonical form of an SPDX expression, or `None` if it does not
/// parse. Identifiers are lowercased and the operands of `AND` and `OR` are
/// flattened, deduplicated and sorted, so `MIT OR Apache-2.0` and
/// `(apache-2.0 OR mit)` compare equal. No other equivalence is applied.
pub fn canonicalize_expression(expression: &str) -> Option<String> {
    if expression.len() > MAX_EXPRESSION_LEN {
        return None;
    }
    let tokens = tokenize(expression)?;
    let mut parser = Parser {
        tokens: &tokens,
        position: 0,
        depth: 0,
    };
    let node = parser.parse_or()?;
    if parser.position != tokens.len() {
        return None;
    }
    Some(node.render(false))
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    Open,
    Close,
    And,
    Or,
    With,
    Id(String),
}

fn tokenize(input: &str) -> Option<Vec<Token>> {
    let mut tokens = Vec::new();
    let mut word = String::new();
    let flush = |word: &mut String, tokens: &mut Vec<Token>| -> Option<()> {
        if word.is_empty() {
            return Some(());
        }
        // Operators are case-sensitive in SPDX expressions.
        let token = match word.as_str() {
            "AND" => Token::And,
            "OR" => Token::Or,
            "WITH" => Token::With,
            id if is_valid_id(id) => Token::Id(id.to_ascii_lowercase()),
            _ => return None,
        };
        tokens.push(token);
        word.clear();
        Some(())
    };
    for c in input.chars() {
        match c {
            '(' | ')' => {
                flush(&mut word, &mut tokens)?;
                tokens.push(if c == '(' { Token::Open } else { Token::Close });
            }
            c if c.is_whitespace() => flush(&mut word, &mut tokens)?,
            c => word.push(c),
        }
    }
    flush(&mut word, &mut tokens)?;
    Some(tokens)
}

/// Identifier characters: letters, digits, '.', '-', a ':' for
/// `DocumentRef-x:LicenseRef-y`, and an optional trailing '+'.
fn is_valid_id(id: &str) -> bool {
    let body = id.strip_suffix('+').unwrap_or(id);
    !body.is_empty()
        && body
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':'))
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Node {
    Leaf(String),
    With(String, String),
    And(Vec<Node>),
    Or(Vec<Node>),
}

impl Node {
    fn render(&self, nested: bool) -> String {
        match self {
            Self::Leaf(id) => id.clone(),
            Self::With(id, exception) => format!("{id} with {exception}"),
            Self::And(children) | Self::Or(children) => {
                let operator = if matches!(self, Self::And(_)) {
                    " and "
                } else {
                    " or "
                };
                let joined = children
                    .iter()
                    .map(|c| c.render(true))
                    .collect::<Vec<_>>()
                    .join(operator);
                if nested {
                    format!("({joined})")
                } else {
                    joined
                }
            }
        }
    }

    fn combine(is_and: bool, items: Vec<Self>) -> Self {
        let mut flat = Vec::new();
        for item in items {
            match item {
                Self::And(children) if is_and => flat.extend(children),
                Self::Or(children) if !is_and => flat.extend(children),
                other => flat.push(other),
            }
        }
        flat.sort_by_key(|n| n.render(true));
        flat.dedup();
        if flat.len() == 1 {
            return flat.pop().expect("one item");
        }
        if is_and {
            Self::And(flat)
        } else {
            Self::Or(flat)
        }
    }
}

struct Parser<'a> {
    tokens: &'a [Token],
    position: usize,
    depth: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.position)
    }

    fn next(&mut self) -> Option<&Token> {
        let token = self.tokens.get(self.position);
        self.position += 1;
        token
    }

    fn parse_or(&mut self) -> Option<Node> {
        let mut items = vec![self.parse_and()?];
        while self.peek() == Some(&Token::Or) {
            self.position += 1;
            items.push(self.parse_and()?);
        }
        Some(if items.len() == 1 {
            items.pop()?
        } else {
            Node::combine(false, items)
        })
    }

    fn parse_and(&mut self) -> Option<Node> {
        let mut items = vec![self.parse_with()?];
        while self.peek() == Some(&Token::And) {
            self.position += 1;
            items.push(self.parse_with()?);
        }
        Some(if items.len() == 1 {
            items.pop()?
        } else {
            Node::combine(true, items)
        })
    }

    fn parse_with(&mut self) -> Option<Node> {
        let atom = self.parse_atom()?;
        if self.peek() == Some(&Token::With) {
            self.position += 1;
            let Node::Leaf(id) = atom else {
                return None;
            };
            let Some(Token::Id(exception)) = self.next() else {
                return None;
            };
            return Some(Node::With(id, exception.clone()));
        }
        Some(atom)
    }

    fn parse_atom(&mut self) -> Option<Node> {
        match self.next()? {
            Token::Id(id) => Some(Node::Leaf(id.clone())),
            Token::Open => {
                self.depth += 1;
                if self.depth > MAX_PAREN_DEPTH {
                    return None;
                }
                let inner = self.parse_or()?;
                if self.next()? != &Token::Close {
                    return None;
                }
                self.depth -= 1;
                Some(inner)
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canon(s: &str) -> Option<String> {
        canonicalize_expression(s)
    }

    #[test]
    fn operand_order_and_case_are_ignored() {
        assert_eq!(canon("MIT OR Apache-2.0"), canon("(apache-2.0 OR mit)"));
        assert_eq!(
            canon("A AND (B AND C)"),
            canon("C AND B AND A"),
            "nested AND is flattened"
        );
    }

    #[test]
    fn operator_meaning_is_kept() {
        assert_ne!(canon("MIT OR Apache-2.0"), canon("MIT AND Apache-2.0"));
        assert_ne!(canon("(A OR B) AND C"), canon("A OR (B AND C)"));
        assert_eq!(canon("A OR B AND C"), canon("A OR (B AND C)"));
    }

    #[test]
    fn with_exceptions() {
        assert_eq!(
            canon("GPL-2.0-only WITH Classpath-exception-2.0").as_deref(),
            Some("gpl-2.0-only with classpath-exception-2.0")
        );
        assert_ne!(
            canon("GPL-2.0-only WITH Classpath-exception-2.0"),
            canon("GPL-2.0-only")
        );
    }

    #[test]
    fn license_refs_and_plus() {
        assert!(canon("DocumentRef-spdx:LicenseRef-custom OR GPL-2.0+").is_some());
    }

    #[test]
    fn rejects_invalid_expressions() {
        for bad in [
            "",
            "MIT OR",
            "(MIT",
            "MIT)",
            "MIT and Apache-2.0",
            "MIT WITH",
            "(A OR B) WITH C",
            "MIT, Apache-2.0",
            "MIT Apache-2.0",
        ] {
            assert_eq!(canon(bad), None, "{bad:?} should not parse");
        }
        let deep = format!("{}MIT{}", "(".repeat(40), ")".repeat(40));
        assert_eq!(canon(&deep), None);
    }

    #[test]
    fn unparsed_expressions_fall_back_to_text() {
        let a = LicenseEntry::from_expression("MIT  and   Apache-2.0");
        assert!(!a.parsed);
        assert_eq!(a.canonical, "text:MIT and Apache-2.0");
    }

    #[test]
    fn malformed_id_does_not_equal_an_expression() {
        assert_ne!(
            LicenseEntry::from_id("apache-2.0 or mit").canonical,
            LicenseEntry::from_expression("MIT OR Apache-2.0").canonical
        );
    }

    #[test]
    fn id_and_expression_of_same_license_compare_equal() {
        assert_eq!(
            LicenseEntry::from_id("MIT").canonical,
            LicenseEntry::from_expression("mit").canonical
        );
        assert_ne!(
            LicenseEntry::from_id("MIT").canonical,
            LicenseEntry::from_name("MIT").canonical
        );
    }
}
