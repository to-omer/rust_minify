use fxhash::FxHashSet;
use proc_macro2::TokenStream;
use std::{iter::once, ops::Range};
use syn::{
    spanned::Spanned,
    visit::{self, Visit},
    BinOp, Expr, File, ForeignItem, ImplItem, Item, Macro, Pat, TraitItem, Type,
};

/// A line-column pair representing the start or end of a Span.
///
/// This type is the same type as `proc_macro2::LineColumn`,
/// and defined to implement [`Hash`](std::hash::Hash).
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LineColumn {
    /// The 1-indexed line in the source file on which the span starts or ends (inclusive).
    pub line: usize,
    /// The 0-indexed column (in UTF-8 characters) in the source file on which the span starts or ends (inclusive).
    pub column: usize,
}
impl LineColumn {
    pub fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }
}
impl From<proc_macro2::LineColumn> for LineColumn {
    fn from(lc: proc_macro2::LineColumn) -> Self {
        Self {
            line: lc.line,
            column: lc.column,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LinedSource<'s> {
    content: &'s str,
    lines: Vec<usize>,
}
impl<'s> LinedSource<'s> {
    pub fn new(content: &'s str) -> Self {
        let lines = once(0)
            .chain(
                content
                    .char_indices()
                    .filter_map(|(i, c)| if c == '\n' { Some(i + 1) } else { None }),
            )
            .collect();
        Self { content, lines }
    }
    fn pos(&self, lc: &LineColumn) -> Option<usize> {
        assert_ne!(lc.line, 0, "LineColumn::line is 1-indexed but {}", lc.line);
        self.lines.get(lc.line - 1).map(|p| p + lc.column)
    }
    pub fn get(&self, range: &Range<LineColumn>) -> Option<&'s str> {
        match (self.pos(&range.start), self.pos(&range.end)) {
            (Some(start), Some(end)) => self.content.get(start..end),
            _ => None,
        }
    }
}

#[derive(Debug, Default)]
pub struct SpanCollector {
    pub bitwise_and: FxHashSet<LineColumn>,
    pub tokens: Vec<Range<LineColumn>>,
}
impl SpanCollector {
    pub fn new() -> Self {
        Default::default()
    }
    pub fn clear(&mut self) {
        self.bitwise_and.clear();
        self.tokens.clear();
    }
    pub fn collect(&mut self, file: &File) {
        self.visit_file(file);
    }
    pub fn collect_item(&mut self, item: &Item) {
        self.visit_item(item);
    }
    fn visit_token_stream(&mut self, tokens: &TokenStream) {
        if !tokens.is_empty() {
            let span = tokens.span();
            self.tokens.push(span.start().into()..span.end().into());
        }
    }
}
impl<'ast> Visit<'ast> for SpanCollector {
    fn visit_bin_op(&mut self, node: &'ast BinOp) {
        if let BinOp::BitAnd(and) = node {
            self.bitwise_and.insert(and.span().start().into());
        }
        visit::visit_bin_op(self, node);
    }
    fn visit_expr(&mut self, node: &'ast Expr) {
        if let Expr::Verbatim(tokens) = node {
            self.visit_token_stream(tokens);
        }
        visit::visit_expr(self, node);
    }
    fn visit_foreign_item(&mut self, node: &'ast ForeignItem) {
        if let ForeignItem::Verbatim(tokens) = node {
            self.visit_token_stream(tokens);
        }
        visit::visit_foreign_item(self, node);
    }
    fn visit_impl_item(&mut self, node: &'ast ImplItem) {
        if let ImplItem::Verbatim(tokens) = node {
            self.visit_token_stream(tokens);
        }
        visit::visit_impl_item(self, node);
    }
    fn visit_item(&mut self, node: &'ast Item) {
        if let Item::Verbatim(tokens) = node {
            self.visit_token_stream(tokens);
        }
        visit::visit_item(self, node);
    }
    fn visit_macro(&mut self, node: &'ast Macro) {
        visit::visit_macro(self, node);
        self.visit_token_stream(&node.tokens);
    }
    fn visit_meta_list(&mut self, node: &'ast syn::MetaList) {
        visit::visit_meta_list(self, node);
        self.visit_token_stream(&node.tokens);
    }
    fn visit_pat(&mut self, node: &'ast Pat) {
        if let Pat::Verbatim(tokens) = node {
            self.visit_token_stream(tokens);
        }
        visit::visit_pat(self, node);
    }
    fn visit_trait_item(&mut self, node: &'ast TraitItem) {
        if let TraitItem::Verbatim(tokens) = node {
            self.visit_token_stream(tokens);
        }
        visit::visit_trait_item(self, node);
    }
    fn visit_type(&mut self, node: &'ast Type) {
        if let Type::Verbatim(tokens) = node {
            self.visit_token_stream(tokens);
        }
        visit::visit_type(self, node);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;
    use syn::{parse_file, parse_str};
    use test_case::test_case;

    #[test_case("fn main(){let x = true;println!(\"{}\",x);}"; "single line")]
    #[test_case("fn main(){\n\tlet x = true;\n\tprintln!(\"{}\",x);\n}"; "multiple line")]
    #[test_case("fn main(){\r\n\tlet x = true;\r\n\tprintln!(\"{}\",x);\r\n}"; "crlf")]
    fn test_lined_source(content: &str) -> Result<(), syn::Error> {
        let source = LinedSource::new(content);
        let file = parse_file(content)?;
        for item in file.items {
            let span = item.span();
            let item_str = source.get(&(span.start().into()..span.end().into()));
            assert!(item_str.is_some());
            let item_str = item_str.unwrap();
            assert!(!item_str.starts_with(' '));
            assert!(!item_str.ends_with(' '));
            assert_eq!(item, parse_str::<Item>(item_str)?);
        }
        Ok(())
    }

    #[test]
    fn test_span_collect() -> Result<(), syn::Error> {
        let content = indoc!(
            r#"
            #[cfg_attr(test, test)]
            //234567890123456789012345
            fn main() {
                let x = true& &true;
            //234567890123456789012345
                if x && true {
                    println!("{}", x);
            //234567890123456789012345
                }
            }
        "#
        );
        let file = parse_file(content)?;
        let mut sc = SpanCollector::new();
        sc.collect(&file);
        assert_eq!(
            sc.bitwise_and.iter().cloned().collect::<Vec<_>>(),
            vec![LineColumn::new(4, 16)]
        );
        assert_eq!(
            sc.tokens,
            vec![
                LineColumn::new(1, 11)..LineColumn::new(1, 21),
                LineColumn::new(7, 17)..LineColumn::new(7, 24)
            ]
        );
        Ok(())
    }
}
