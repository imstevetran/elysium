// .elyx parser — TSX-like component files for Elysium
//
// An .elyx file looks like:
//
//   component Counter {
//       state count = 0
//
//       render {
//           <Column>
//               <Text>"Count: " + count</Text>
//               <Button label="Increment" onClick={count = count + 1} />
//           </Column>
//       }
//   }
//
// The XML-like `render { }` block is desugared into standard Elysium
// view-tree construction calls.

use crate::ast::*;
use crate::error::{CompileError, Result, SourceSpan};
use crate::parser::Parser;

/// An .elyx file — a component with optional XML render block.
#[derive(Debug, Clone)]
pub struct ElyxFile {
    pub component: Node<Component>,
    pub render_block: Option<ElyxRenderBlock>,
}

/// The desugared XML render tree from an .elyx file.
#[derive(Debug, Clone)]
pub struct ElyxRenderBlock {
    pub root: ElyxNode,
    pub span: SourceSpan,
}

/// An XML-like node inside a render block.
#[derive(Debug, Clone)]
pub enum ElyxNode {
    Element {
        tag: String,
        attrs: Vec<(String, ElyxAttrValue)>,
        children: Vec<ElyxNode>,
    },
    Text(String),
    Expr(Box<Node<Expr>>),
    Fragment(Vec<ElyxNode>),
}

#[derive(Debug, Clone)]
pub enum ElyxAttrValue {
    String(String),
    Expr(Box<Node<Expr>>),
    Binding(String),
}

/// Parse an .elyx file.
pub fn parse_elyx(source: &str) -> Result<ElyxFile> {
    // First, look for the render { ... } block and split the source
    let (elysium_part, xml_part_start, xml_part_end) = split_render_block(source)?;

    // Parse the Elysium part (component with body but no render block)
    let mut ely_parser = Parser::new(&elysium_part);
    let program = ely_parser.parse_program()?;

    // Extract the component
    let component = program
        .items
        .into_iter()
        .find_map(|item| {
            if let Item::Component(c) = item.value {
                Some(Node::new(c, item.span))
            } else {
                None
            }
        })
        .ok_or_else(|| {
            CompileError::new(
                ".elyx files must contain exactly one `component` declaration.",
            )
        })?;

    // Parse the render block XML if present
    let render_block = if let Some((start, end)) = xml_part_start.zip(xml_part_end) {
        let xml_source = &source[start..end];
        let root = parse_xml(xml_source, source, start)?;
        Some(ElyxRenderBlock {
            root,
            span: SourceSpan::new(start, end - start),
        })
    } else {
        None
    };

    Ok(ElyxFile {
        component,
        render_block,
    })
}

/// Split source into Elysium part (before render) and XML part (inside render block braces).
fn split_render_block(source: &str) -> Result<(String, Option<usize>, Option<usize>)> {
    // Look for "render {" in the source
    let render_pos = match source.find("render {") {
        Some(p) => p,
        None => return Ok((source.to_string(), None, None)),
    };

    // Find the matching closing brace for the render block
    let open_brace = match source[render_pos..].find('{').map(|p| render_pos + p) {
        Some(ob) => ob,
        None => return Err(CompileError::new("Expected `{` after `render`.")),
    };

    // Find matching `}` for the render block
    let mut depth = 0u32;
    let mut close_brace = None;
    for (i, ch) in source[open_brace..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    close_brace = Some(open_brace + i);
                    break;
                }
            }
            _ => {}
        }
    }

    let cb = match close_brace {
        Some(cb) => cb,
        None => return Err(CompileError::new("Unmatched `{` in render block.")),
    };

    // Elysium part: everything before `render` + `nil` + everything after render block
    let before_render = source[..render_pos].trim_end();
    let after_render = source[cb+1..].trim_end();
    let ely_part = format!("{}\n  nil\n{}\n", before_render, after_render);

    Ok((ely_part, Some(open_brace + 1), Some(cb)))
}

/// Parse XML-like content into an ElyxNode tree.
fn parse_xml(xml_source: &str, full_source: &str, offset: usize) -> Result<ElyxNode> {
    let mut parser = XmlParser::new(xml_source, full_source, offset);
    parser.parse_fragment()
}

struct XmlParser<'a> {
    source: &'a str,
    full_source: &'a str,
    offset: usize,
    pos: usize,
}

impl<'a> XmlParser<'a> {
    fn new(source: &'a str, full_source: &'a str, offset: usize) -> Self {
        XmlParser {
            source,
            full_source,
            offset,
            pos: 0,
        }
    }

    fn remaining(&self) -> &str {
        &self.source[self.pos..]
    }

    fn skip_whitespace(&mut self) {
        while self.pos < self.source.len() {
            let c = self.source.as_bytes()[self.pos] as char;
            if c.is_whitespace() {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    fn expect_char(&mut self, c: char) -> Result<()> {
        self.skip_whitespace();
        if self.pos < self.source.len() && self.source.as_bytes()[self.pos] as char == c {
            self.pos += 1;
            Ok(())
        } else {
            let cur = if self.pos < self.source.len() {
                format!("`{}`", self.source.as_bytes()[self.pos] as char)
            } else {
                "end of input".to_string()
            };
            Err(CompileError::new(&format!(
                "Expected `{}` in XML render block, got {}.",
                c, cur
            )))
        }
    }

    fn peek_char(&self) -> Option<char> {
        if self.pos < self.source.len() {
            Some(self.source.as_bytes()[self.pos] as char)
        } else {
            None
        }
    }

    fn parse_fragment(&mut self) -> Result<ElyxNode> {
        let mut children = Vec::new();
        loop {
            self.skip_whitespace();
            match self.peek_char() {
                None => break,
                Some('<') => {
                    // Check if this is a closing tag
                    if self.pos + 1 < self.source.len() && self.source.as_bytes()[self.pos + 1] as char == '/' {
                        break;
                    }
                    let child = self.parse_element()?;
                    children.push(child);
                }
                Some('{') => {
                    let expr = self.parse_expr_interpolation()?;
                    children.push(expr);
                }
                Some(_) => {
                    let text = self.parse_text()?;
                    children.push(ElyxNode::Text(text));
                }
            }
        }

        if children.len() == 1 {
            Ok(children.into_iter().next().unwrap())
        } else {
            Ok(ElyxNode::Fragment(children))
        }
    }

    fn parse_text(&mut self) -> Result<String> {
        let start = self.pos;
        while self.pos < self.source.len() {
            let c = self.source.as_bytes()[self.pos] as char;
            if c == '<' || c == '{' || c == '}' {
                break;
            }
            self.pos += 1;
        }
        Ok(self.source[start..self.pos].trim().to_string())
    }

    fn parse_expr_interpolation(&mut self) -> Result<ElyxNode> {
        self.expect_char('{')?;
        // Parse an Elysium expression until matching '}'
        let expr_start = self.pos;
        let mut depth = 1u32;
        while self.pos < self.source.len() && depth > 0 {
            let c = self.source.as_bytes()[self.pos] as char;
            match c {
                '{' => depth += 1,
                '}' => depth -= 1,
                _ => {}
            }
            if depth > 0 {
                self.pos += 1;
            }
        }
        let expr_source = self.source[expr_start..self.pos].to_string();
        let span_offset = self.offset + expr_start;
        self.pos += 1; // consume '}'

        // Parse the expression using the Elysium parser
        let mut ely_parser = Parser::new(&expr_source);
        let expr = ely_parser.parse_expr().map_err(|e| {
            CompileError::new(&format!("In render block expression: {}", e.message))
        })?;

        Ok(ElyxNode::Expr(Box::new(Node::new(
            expr,
            SourceSpan::new(span_offset, self.pos - expr_start - 1),
        ))))
    }

    fn parse_element(&mut self) -> Result<ElyxNode> {
        self.expect_char('<')?;

        // Check for closing tag or self-closing
        if self.peek_char() == Some('/') {
            // This is a closing tag, not an element — should not happen at this level
            return Err(CompileError::new("Unexpected closing tag."));
        }

        // Parse tag name
        let tag = self.parse_name()?;

        // Parse attributes
        let mut attrs = Vec::new();
        loop {
            self.skip_whitespace();
            match self.peek_char() {
                Some('>') => {
                    self.pos += 1;
                    break;
                }
                Some('/') => {
                    // Self-closing tag
                    self.pos += 1;
                    self.expect_char('>')?;
                    return Ok(ElyxNode::Element {
                        tag,
                        attrs,
                        children: vec![],
                    });
                }
                Some(c) if !c.is_whitespace() && c != '/' && c != '>' => {
                    let attr = self.parse_attr()?;
                    attrs.push(attr);
                }
                _ => break,
            }
        }

        // Parse children
        let mut children = Vec::new();
        loop {
            self.skip_whitespace();
            match self.peek_char() {
                None | Some('<') => {
                    // Could be closing tag or nested element
                    if self.pos + 1 < self.source.len() {
                        let next_two = &self.source[self.pos..self.pos + 2];
                        if next_two == "</" {
                            break;
                        }
                    }
                    let child = self.parse_fragment()?;
                    match child {
                        ElyxNode::Fragment(c) => children.extend(c),
                        other => children.push(other),
                    }
                }
                Some('}') => break,
                Some(_) => {
                    let text = self.parse_text()?;
                    if !text.is_empty() {
                        children.push(ElyxNode::Text(text));
                    }
                }
            }
        }

        // Parse closing tag
        self.expect_char('<')?;
        self.expect_char('/')?;
        let close_tag = self.parse_name()?;
        if close_tag != tag {
            return Err(CompileError::new(&format!(
                "Mismatched closing tag: `</{}>` does not match `<{}>`.",
                close_tag, tag
            )));
        }
        self.expect_char('>')?;

        Ok(ElyxNode::Element {
            tag,
            attrs,
            children,
        })
    }

    fn parse_name(&mut self) -> Result<String> {
        self.skip_whitespace();
        let start = self.pos;
        while self.pos < self.source.len() {
            let c = self.source.as_bytes()[self.pos] as char;
            if c.is_alphanumeric() || c == '_' || c == '-' || c == ':' {
                self.pos += 1;
            } else {
                break;
            }
        }
        if self.pos == start {
            return Err(CompileError::new("Expected tag or attribute name in XML."));
        }
        Ok(self.source[start..self.pos].to_string())
    }

    fn parse_attr(&mut self) -> Result<(String, ElyxAttrValue)> {
        let name = self.parse_name()?;
        self.skip_whitespace();
        self.expect_char('=')?;

        self.skip_whitespace();
        match self.peek_char() {
            Some('"') | Some('\'') => {
                // String attribute value
                let quote = if self.peek_char() == Some('"') { '"' } else { '\'' };
                self.pos += 1;
                let start = self.pos;
                while self.pos < self.source.len() && self.source.as_bytes()[self.pos] as char != quote {
                    self.pos += 1;
                }
                let val = self.source[start..self.pos].to_string();
                self.pos += 1; // consume quote
                Ok((name, ElyxAttrValue::String(val)))
            }
            Some('{') => {
                // Expression attribute value
                self.pos += 1; // consume '{'
                let expr_start = self.pos;
                let mut depth = 1u32;
                while self.pos < self.source.len() && depth > 0 {
                    let c = self.source.as_bytes()[self.pos] as char;
                    match c {
                        '{' => depth += 1,
                        '}' => depth -= 1,
                        _ => {}
                    }
                    if depth > 0 {
                        self.pos += 1;
                    }
                }
                let expr_source = self.source[expr_start..self.pos].to_string();
                let span_offset = self.offset + expr_start;
                let span_len = self.pos - expr_start;
                self.pos += 1; // consume '}'

                let mut ely_parser = Parser::new(&expr_source);
                let expr = ely_parser
                    .parse_expr()
                    .map_err(|e| CompileError::new(&format!("In attribute expression: {}", e.message)))?;

                Ok((
                    name,
                    ElyxAttrValue::Expr(Box::new(Node::new(
                        expr,
                        SourceSpan::new(span_offset, span_len),
                    ))),
                ))
            }
            Some(c) if c.is_alphabetic() || c == '_' => {
                // Binding shorthand (e.g., value = count)
                let start = self.pos;
                while self.pos < self.source.len() {
                    let c = self.source.as_bytes()[self.pos] as char;
                    if c.is_alphanumeric() || c == '_' || c == '.' {
                        self.pos += 1;
                    } else {
                        break;
                    }
                }
                Ok((name, ElyxAttrValue::Binding(self.source[start..self.pos].to_string())))
            }
            other => Err(CompileError::new(&format!(
                "Expected attribute value (string, expression, or binding), got {:?}",
                other
            ))),
        }
    }
}

/// Desugar an ElyxNode tree into Elysium expression AST nodes.
/// Converts e.g. `<Column><Text>"hello"</Text></Column>` into
/// `Column { children: [Text { content: "hello" }] }`.
pub fn desugar_render_block(block: &ElyxRenderBlock) -> Result<Expr> {
    desugar_node(&block.root)
}

fn desugar_node(node: &ElyxNode) -> Result<Expr> {
    match node {
        ElyxNode::Text(content) => Ok(Expr::Literal(Node::new(
            Literal::String(content.clone()),
            SourceSpan::new(0, 0),
        ))),
        ElyxNode::Expr(expr) => Ok(expr.value.clone()),
        ElyxNode::Fragment(children) => {
            // Flatten fragments: if single child, return it directly
            if children.len() == 1 {
                return desugar_node(&children[0]);
            }
            // Multiple children: wrap in Column
            let mut child_exprs = Vec::new();
            for child in children {
                child_exprs.push(Node::new(
                    desugar_node(child)?,
                    SourceSpan::new(0, 0),
                ));
            }
            Ok(Expr::Call {
                callee: Box::new(Node::new(
                    Expr::Identifier("Column".to_string()),
                    SourceSpan::new(0, 0),
                )),
                args: vec![],
            })
        }
        ElyxNode::Element {
            tag: _,
            attrs,
            children,
        } => {
            // Convert the element to a named constructor call
            let mut child_exprs = Vec::new();
            for child in children {
                child_exprs.push(Node::new(
                    desugar_node(child)?,
                    SourceSpan::new(0, 0),
                ));
            }

            // Build attribute pairs
            let mut named_args = Vec::new();
            for (name, val) in attrs {
                let attr_expr = match val {
                    ElyxAttrValue::String(s) => Expr::Literal(Node::new(
                        Literal::String(s.clone()),
                        SourceSpan::new(0, 0),
                    )),
                    ElyxAttrValue::Expr(expr) => expr.value.clone(),
                    ElyxAttrValue::Binding(b) => Expr::Identifier(b.clone()),
                };
                named_args.push((name.clone(), Node::new(attr_expr, SourceSpan::new(0, 0))));
            }

            if !child_exprs.is_empty() {
                named_args.push((
                    "children".to_string(),
                    Node::new(
                        Expr::Array(child_exprs),
                        SourceSpan::new(0, 0),
                    ),
                ));
            }

            // Use a record-like constructor
            let mut fields = Vec::new();
            for (name, val_node) in &named_args {
                fields.push((name.clone(), val_node.clone()));
            }

            Ok(Expr::Record(fields))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_element() {
        let source = "<Text>Hello</Text>";
        let (_, xml_start, xml_end) = split_render_block(&format!("render {{{}}}", source))
            .unwrap();
        let (start, end) = (xml_start.unwrap(), xml_end.unwrap());
        let xml = &source[..];
        let node = parse_xml(xml, source, 0).unwrap();
        match node {
            ElyxNode::Element { tag, children, .. } => {
                assert_eq!(tag, "Text");
                assert_eq!(children.len(), 1);
                match &children[0] {
                    ElyxNode::Text(t) => assert_eq!(t, "Hello"),
                    _ => panic!("expected Text child"),
                }
            }
            _ => panic!("expected Element"),
        }
    }

    #[test]
    fn test_nested_elements() {
        let source = "<Column><Text>Hi</Text><Text>There</Text></Column>";
        let node = parse_xml(source, source, 0).unwrap();
        match node {
            ElyxNode::Element { tag, children, .. } => {
                assert_eq!(tag, "Column");
                assert_eq!(children.len(), 2);
            }
            _ => panic!("expected Element"),
        }
    }

    #[test]
    fn test_self_closing() {
        let source = "<Button label=\"Click\" onClick={count + 1} />";
        let node = parse_xml(source, source, 0).unwrap();
        match node {
            ElyxNode::Element { tag, attrs, children } => {
                assert_eq!(tag, "Button");
                assert_eq!(attrs.len(), 2);
                assert!(children.is_empty());
            }
            _ => panic!("expected Element"),
        }
    }

    #[test]
    fn test_name_parsing() {
        let source = "my-element";
        let mut parser = XmlParser::new(source, source, 0);
        let name = parser.parse_name().unwrap();
        assert_eq!(name, "my-element");
    }

    #[test]
    fn test_text_content() {
        let source = "  Hello World  ";
        let mut parser = XmlParser::new(source, source, 0);
        let text = parser.parse_text();
        assert_eq!(text.unwrap(), "Hello World");
    }

    #[test]
    fn test_split_render_block_present() {
        let source = "component Foo {\n    state x = 1\n    render {\n        <Text>Hello</Text>\n    }\n}";
        let (ely_part, xml_start, xml_end) = split_render_block(source).unwrap();
        assert!(xml_start.is_some());
        assert!(!ely_part.contains("render"));
    }

    #[test]
    fn test_split_render_block_absent() {
        let source = "component Foo { state x = 1 }";
        let (ely_part, xml_start, _) = split_render_block(source).unwrap();
        assert!(xml_start.is_none());
        assert_eq!(ely_part, source);
    }
}
