use crate::attr::{is_minify_skip, ItemExt};
use fxhash::FxHashSet;
use proc_macro2::{Span, TokenStream};
use std::ops::Range;
use syn::{
    spanned::Spanned,
    visit::{self, Visit},
    Attribute, BinOp, Expr, File, ForeignItem, ImplItem, Item, Macro, Pat, StmtMacro, TraitItem,
    Type,
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
    // Character offsets of line starts, followed by the end of the source.
    lines: Vec<usize>,
    // Character and byte offsets immediately after each multibyte character.
    multibyte: Vec<(usize, usize)>,
}
impl<'s> LinedSource<'s> {
    pub fn new(content: &'s str) -> Self {
        let mut lines = vec![0];
        let mut multibyte = Vec::new();
        let mut len = 0;
        for (byte, ch) in content.char_indices() {
            len += 1;
            if ch == '\n' {
                lines.push(len);
            }
            if !ch.is_ascii() {
                multibyte.push((len, byte + ch.len_utf8()));
            }
        }
        lines.push(len);
        Self {
            content,
            lines,
            multibyte,
        }
    }
    fn pos(&self, lc: &LineColumn) -> Option<usize> {
        assert_ne!(lc.line, 0, "LineColumn::line is 1-indexed but {}", lc.line);
        let start = *self.lines.get(lc.line - 1)?;
        let end = *self.lines.get(lc.line)?;
        if lc.column > end - start {
            return None;
        }
        let position = start + lc.column;
        let index = self
            .multibyte
            .partition_point(|&(chars, _)| chars <= position);
        let extra = self.multibyte[..index]
            .last()
            .map_or(0, |&(chars, bytes)| bytes - chars);
        Some(position + extra)
    }
    pub fn get(&self, range: &Range<LineColumn>) -> Option<&'s str> {
        match (self.pos(&range.start), self.pos(&range.end)) {
            (Some(start), Some(end)) => self.content.get(start..end),
            _ => None,
        }
    }
}

#[derive(Default)]
pub(crate) struct SkipCollector<'ast> {
    pub items: Vec<(Span, &'ast [Attribute])>,
}

impl<'ast> SkipCollector<'ast> {
    pub fn collect(file: &'ast File) -> Self {
        let mut collector = Self::default();
        collector.visit_file(file);
        collector
            .items
            .sort_unstable_by_key(|(span, _)| span.start());
        collector
    }

    fn mark(&mut self, node: &impl Spanned, attrs: &'ast [Attribute]) {
        if is_minify_skip(attrs) {
            self.items.push((node.span(), attrs));
        }
    }
}

impl<'ast> Visit<'ast> for SkipCollector<'ast> {
    fn visit_stmt_macro(&mut self, node: &'ast StmtMacro) {
        self.mark(node, &node.attrs);
        visit::visit_stmt_macro(self, node);
    }

    fn visit_item(&mut self, node: &'ast Item) {
        if let Some(attrs) = node.get_attributes() {
            self.mark(node, attrs);
        }
        visit::visit_item(self, node);
    }

    fn visit_impl_item(&mut self, node: &'ast ImplItem) {
        let attrs = match node {
            ImplItem::Const(node) => &node.attrs,
            ImplItem::Fn(node) => &node.attrs,
            ImplItem::Type(node) => &node.attrs,
            ImplItem::Macro(node) => &node.attrs,
            _ => return,
        };
        self.mark(node, attrs);
        visit::visit_impl_item(self, node);
    }

    fn visit_trait_item(&mut self, node: &'ast TraitItem) {
        let attrs = match node {
            TraitItem::Const(node) => &node.attrs,
            TraitItem::Fn(node) => &node.attrs,
            TraitItem::Type(node) => &node.attrs,
            TraitItem::Macro(node) => &node.attrs,
            _ => return,
        };
        self.mark(node, attrs);
        visit::visit_trait_item(self, node);
    }

    fn visit_foreign_item(&mut self, node: &'ast ForeignItem) {
        let attrs = match node {
            ForeignItem::Fn(node) => &node.attrs,
            ForeignItem::Static(node) => &node.attrs,
            ForeignItem::Type(node) => &node.attrs,
            ForeignItem::Macro(node) => &node.attrs,
            _ => return,
        };
        self.mark(node, attrs);
        visit::visit_foreign_item(self, node);
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
    #[test_case("fn 日本語(){let s = \"é🦀\";} fn 次(){}"; "unicode")]
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
    fn test_lined_source_boundaries() {
        let source = LinedSource::new("a日\r\n🦀b\n");
        for (start, end, expected) in [
            ((1, 0), (1, 1), Some("a")),
            ((1, 1), (1, 2), Some("日")),
            ((1, 2), (1, 4), Some("\r\n")),
            ((2, 0), (2, 1), Some("🦀")),
            ((2, 1), (2, 2), Some("b")),
            ((1, 1), (2, 1), Some("日\r\n🦀")),
            ((3, 0), (3, 0), Some("")),
            ((2, 0), (2, 4), None),
            ((4, 0), (4, 0), None),
            ((1, usize::MAX), (1, usize::MAX), None),
        ] {
            let range = LineColumn::new(start.0, start.1)..LineColumn::new(end.0, end.1);
            assert_eq!(source.get(&range), expected, "{range:?}");
        }
        let empty = LinedSource::new("");
        assert_eq!(
            empty.get(&(LineColumn::new(1, 0)..LineColumn::new(1, 0))),
            Some("")
        );
        assert_eq!(
            empty.get(&(LineColumn::new(2, 0)..LineColumn::new(2, 0))),
            None
        );
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
