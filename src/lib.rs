pub mod attr;
pub mod fix;
pub mod marker;

use crate::marker::{LineColumn, SpanCollector};
use attr::{drain_minify_skip, is_minify_skip, ItemExt};
use fix::Visitor;
use fxhash::FxHashSet;
use marker::LinedSource;
use once_cell::sync::Lazy;
use proc_macro2::{Delimiter, Group, Ident, Literal, Punct, Spacing, Span, TokenStream, TokenTree};
use quote::ToTokens;
use std::{iter::Peekable, ops::Range, str::FromStr};
use syn::{parse2, spanned::Spanned, File};

pub fn minify(content: &str) -> Result<String, syn::Error> {
    minify_opt(content, &MinifyOption::default())
}

pub fn minify_opt(content: &str, option: &MinifyOption) -> Result<String, syn::Error> {
    let tokens = TokenStream::from_str(content)?;
    let mut sc = SpanCollector::new();
    let file = match parse2::<File>(tokens.clone()) {
        Ok(file) => file,
        Err(_) => {
            let mut state = State::new_with_capacity(
                sc,
                MinifyMode {
                    space: SpaceCollapsing::Token,
                },
                content.len(),
            );
            state.step_tokens(tokens);
            return Ok(state.buf);
        }
    };
    sc.collect(&file);
    let source = LinedSource::new(content);
    let mut state = State::new_with_capacity(
        sc,
        MinifyMode {
            space: SpaceCollapsing::Syntax,
        },
        content.len(),
    );

    for attr in file.attrs {
        state.step_tokens(attr.into_token_stream());
    }
    let mut is_newline = state.buf.is_empty();
    for mut item in file.items {
        let cond = item.get_attributes().is_some_and(is_minify_skip);
        if cond {
            if !is_newline {
                state.buf.push('\n');
                is_newline = true;
            }
            let span = item.span();
            let mut start = span.start().into();
            if option.remove_skip {
                for attr in std::mem::take(item.get_attributes_mut().unwrap()) {
                    if is_minify_skip(std::slice::from_ref(&attr)) {
                        let attr_span = attr.span();
                        let prefix = source
                            .get(&(start..attr_span.start().into()))
                            .ok_or_else(|| syn::Error::new(attr_span, "invalid source span"))?;
                        state.buf.push_str(prefix);
                        let mut attrs = vec![attr];
                        drain_minify_skip(&mut attrs);
                        for attr in attrs {
                            state.buf.push_str(&attr.into_token_stream().to_string());
                        }
                        start = attr_span.end().into();
                    }
                }
            }
            let s = source
                .get(&(start..span.end().into()))
                .ok_or_else(|| syn::Error::new(span, "invalid source span"))?;
            state.buf.push_str(s);
            state.buf.push('\n');
            let end: LineColumn = span.end().into();
            while state.tokens.peek().is_some_and(|r| r.end <= end) {
                state.tokens.next();
            }
            state.prev = PrevToken::None;
        } else {
            is_newline = false;
            Visitor::fix_item(&mut item);
            if option.add_rustfmt_skip {
                state.buf.push_str("#[cfg_attr(any(),rustfmt::skip)]");
            }
            state.step_tokens(item.into_token_stream());
        }
    }
    Ok(state.buf)
}

#[derive(Debug, Clone, Default)]
pub struct MinifyOption {
    pub remove_skip: bool,
    pub add_rustfmt_skip: bool,
}

#[derive(Debug, Clone)]
pub struct State {
    prev: PrevToken,
    buf: String,
    bitwise_and: FxHashSet<LineColumn>,
    tokens: Peekable<std::vec::IntoIter<Range<LineColumn>>>,
    mode: MinifyMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinifyMode {
    space: SpaceCollapsing,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpaceCollapsing {
    Syntax,
    Macro,
    Token,
}

#[derive(Debug, Clone)]
enum PrevToken {
    None,
    /// Ident or literal; whether a following dot needs a space.
    IdentOrLiteral(bool),
    Number,
    Punct(Punct),
}

const SEPARATED: [(char, char); 22] = [
    ('!', '='),
    ('%', '='),
    ('&', '&'),
    ('&', '='),
    ('*', '='),
    ('+', '='),
    ('-', '='),
    ('-', '>'),
    ('.', '.'),
    ('.', '='),
    ('/', '='),
    (':', ':'),
    ('<', '-'),
    ('<', '<'),
    ('<', '='),
    ('=', '='),
    ('=', '>'),
    ('>', '='),
    ('>', '>'),
    ('^', '='),
    ('|', '='),
    ('|', '|'),
];

static MACHER: Lazy<FxHashSet<(char, char)>> = Lazy::new(|| SEPARATED.iter().cloned().collect());

impl State {
    pub fn new(collector: SpanCollector, mode: MinifyMode) -> Self {
        Self::new_with_capacity(collector, mode, 0)
    }
    pub fn new_with_capacity(
        mut collector: SpanCollector,
        mode: MinifyMode,
        capacity: usize,
    ) -> Self {
        collector.tokens.sort_unstable_by_key(|range| range.start);
        Self {
            prev: Default::default(),
            buf: String::with_capacity(capacity),
            bitwise_and: collector.bitwise_and,
            tokens: collector.tokens.into_iter().peekable(),
            mode,
        }
    }
    pub fn step_tokens(&mut self, tokens: TokenStream) {
        let mut tokens = tokens.into_iter().peekable();
        while let Some(tt) = tokens.next() {
            self.step_token_tree_with_next(tt, tokens.peek());
        }
    }
    pub fn step_token_tree(&mut self, tt: TokenTree) {
        self.step_token_tree_with_next(tt, None);
    }
    fn step_token_tree_with_next(&mut self, tt: TokenTree, next: Option<&TokenTree>) {
        self.switch_space_mode(tt.span());
        match tt {
            TokenTree::Group(group) => self.step_group(group),
            TokenTree::Ident(ident) => self.step_ident(ident),
            TokenTree::Punct(punct) => self.step_punct(punct, next),
            TokenTree::Literal(literal) => self.step_literal(literal),
        }
    }
    fn step_group(&mut self, group: Group) {
        let (ldel, rdel) = match group.delimiter() {
            Delimiter::Parenthesis => ("(", ")"),
            Delimiter::Brace => ("{", "}"),
            Delimiter::Bracket => ("[", "]"),
            Delimiter::None => ("", ""),
        };
        self.buf.push_str(ldel);
        self.prev = PrevToken::None;
        self.step_tokens(group.stream());
        self.buf.push_str(rdel);
        self.prev = PrevToken::None;
    }
    fn step_ident(&mut self, ident: Ident) {
        if matches!(self.prev, PrevToken::IdentOrLiteral(_) | PrevToken::Number) {
            self.buf.push(' ');
        }
        self.buf.push_str(&ident.to_string());
        self.prev = PrevToken::IdentOrLiteral(false);
    }
    fn step_punct(&mut self, punct: Punct, next: Option<&TokenTree>) {
        let needs_space = match &self.prev {
            PrevToken::IdentOrLiteral(true) if punct.as_char() == '.' => true,
            PrevToken::Number if punct.as_char() == '.' => {
                !(punct.spacing() == Spacing::Joint
                    && matches!(next, Some(TokenTree::Punct(p)) if p.as_char() == '.'))
            }
            PrevToken::IdentOrLiteral(_) | PrevToken::Number
                if "#\"'".contains(punct.as_char()) =>
            {
                true
            }
            PrevToken::Punct(prev)
                if matches!(
                    (prev.as_char(), punct.as_char()),
                    ('/', '/' | '*') | ('#', '#')
                ) =>
            {
                true
            }
            PrevToken::Punct(prev) if matches!(prev.spacing(), Spacing::Alone) => {
                match self.mode.space {
                    SpaceCollapsing::Syntax => match (prev.as_char(), punct.as_char()) {
                        (':', ':') => true,
                        ('|', '|') => true,
                        ('&', '&') => self.bitwise_and.contains(&prev.span().start().into()),
                        _ => false,
                    },
                    SpaceCollapsing::Macro | SpaceCollapsing::Token => {
                        MACHER.contains(&(prev.as_char(), punct.as_char()))
                    }
                }
            }
            _ => false,
        };
        if needs_space {
            self.buf.push(' ');
        }
        self.buf.push(punct.as_char());
        self.prev = PrevToken::Punct(punct);
    }
    fn step_literal(&mut self, literal: Literal) {
        let lit = literal.to_string();
        if matches!(self.prev, PrevToken::IdentOrLiteral(_) | PrevToken::Number)
            || matches!(&self.prev, PrevToken::Punct(p) if p.as_char() == '#' && lit.starts_with('"'))
        {
            self.buf.push(' ');
        }
        let last_is_dot = lit.ends_with('.');
        let number = lit.chars().next().is_some_and(|c| c.is_ascii_digit());
        let tuple_access =
            matches!(&self.prev, PrevToken::Punct(punct) if punct.as_char() == '.') && number;
        self.buf.push_str(&lit);
        self.prev = if last_is_dot || tuple_access {
            PrevToken::IdentOrLiteral(true)
        } else if number && self.mode.space != SpaceCollapsing::Syntax {
            PrevToken::Number
        } else {
            PrevToken::IdentOrLiteral(false)
        };
    }
    fn switch_space_mode(&mut self, span: Span) {
        if self.mode.space == SpaceCollapsing::Token {
            return;
        }
        let start = span.start().into();
        while self.tokens.peek().is_some_and(|range| range.end <= start) {
            self.tokens.next();
        }
        self.mode.space = if self
            .tokens
            .peek()
            .is_some_and(|range| range.contains(&start))
        {
            SpaceCollapsing::Macro
        } else {
            SpaceCollapsing::Syntax
        };
    }
}

impl Default for PrevToken {
    fn default() -> Self {
        Self::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;
    use test_case::test_case;

    #[test_case(
        "fn f() { true & & true }",
        "fn f(){true& &true}";
        "bitwise and after and"
    )]
    #[test_case(
        "fn f() { let x: ::m::T = ::m::T::new() }",
        "fn f(){let x: ::m::T=::m::T::new()}";
        "isolated colon after colon"
    )]
    #[test_case(
        "fn f() { 1 | |_| 1 }",
        "fn f(){1| |_|1}";
        "or after or"
    )]
    #[test_case(
        indoc!(r#"
            macro_rules! f {
                (:::) => { ::: };
                (:: :) => { :: : };
                (: ::) => { : :: };
                (: : :) => { : : : };
            }
        "#),
        "macro_rules!f{(:::)=>{:::};(:: :)=>{:: :};(: ::)=>{: ::};(: : :)=>{: : :};}";
        // optimal: "macro_rules!f{(:::)=>{:::};(:::)=>{:::};(: ::)=>{: ::};(: : :)=>{: : :};}";
        "macro colon tokens"
    )]
    #[test_case(
        "fn f() { 1. ..2. }",
        "fn f(){1. ..2.}";
        "floating-point literal end with dot after dot"
    )]
    #[test_case(
        "fn f() { let x: Option<usize> = None; }",
        "fn f(){let x:Option<usize>=None;}";
        "ge in generics"
    )]
    #[test_case(
        "macro_rules! f { ( $ x : ident ) => { let $x: Option<usize> = None; }; }",
        "macro_rules!f{($x:ident)=>{let$x:Option<usize> =None;};}";
        // optimal: "macro_rules!f{($x:ident)=>{let$x:Option<usize>=None;};}";
        "ge in generics in macro"
    )]
    #[test_case(
        indoc!(r#"
            fn total(a: Vec<usize>) -> usize {
                let mut total = 0usize;
                for a in a.iter().cloned() {
                    total += a;
                }
                total
            }
        "#),
        "fn total(a:Vec<usize>)->usize{let mut total=0usize;for a in a.iter().cloned(){total+=a;}total}";
        "total"
    )]
    #[test_case(
        indoc!(r#"
            fn nested_tuple(t: ((i32,),)) -> i32 {
                t . 0 . 0 * ( t . 0 ) . 0
            }
        "#),
        "fn nested_tuple(t:((i32,),))->i32{t.0 .0*(t.0).0}";
        "nested_tuple"
    )]
    #[test_case(
        indoc!(r#"
            struct X<'a>(&'a ());
            impl<'a> X<'a> {
                fn x(&'a self) -> impl 'a + Clone {
                    match "a" {
                        _ => {
                            macro!( #a #b );
                        }
                    }
                }
            }
        "#),
        "struct X<'a>(&'a());impl<'a>X<'a>{fn x(&'a self)->impl 'a+Clone{match \"a\"{_=>{macro!(#a #b);}}}}";
        "reserving syntax for rust 2021"
    )]
    #[test_case(
        "#![no_std] #![allow(dead_code)] fn f() {}",
        "#![no_std]#![allow(dead_code)]fn f(){}";
        "crate attributes"
    )]
    #[test_case(
        "fn f() { let x = 6 / *&2; }",
        "fn f(){let x=6/ *&2;}";
        "division before dereference"
    )]
    #[test_case(
        "m!(/ /, / *, 1 . 2, 1 ., 1..2, # #, # \"ok\");",
        "m!(/ /,/ *,1 .2,1 .,1..2,# #,# \"ok\");";
        "macro token boundaries"
    )]
    #[test_case(
        "m!(1..2,1..=2,1...2,1 . . 2,1 . 2,1 .,1. ..2.);",
        "m!(1..2,1..=2,1...2,1 . .2,1 .2,1 .,1. ..2.);";
        "numeric ranges and separate dots in macros"
    )]
    #[test_case(
        "1..2,1..=2,1...2,1 . . 2,1 . 2,1 .,1. ..2.",
        "1..2,1..=2,1...2,1 . .2,1 .2,1 .,1. ..2.";
        "numeric ranges and separate dots in fallback"
    )]
    #[test_case(
        "/ / / * 1 . 2 # # # \"ok\"",
        "/ / / *1 .2 # # # \"ok\"";
        "fallback token boundaries"
    )]
    #[test_case(
        "fn f() -> m!(> =) { #![allow(unused)] 1 }",
        "fn f()->m!(> =){#![allow(unused)]1}";
        "inner attribute after signature macro"
    )]
    #[test_case(
        "fn f() { m!(x) } #[cfg_attr(any(), rust_minify::skip)] fn g() {} m!(> =);",
        "fn f(){m!(x)}\n#[cfg_attr(any(), rust_minify::skip)] fn g() {}\nm!(> =);";
        "macro after skipped item"
    )]
    #[test_case(
        "#[cfg_attr(any(), rust_minify::skip)]\nfn f() { let s = \"日本語\"; }",
        "#[cfg_attr(any(), rust_minify::skip)]\nfn f() { let s = \"日本語\"; }\n";
        "skipped unicode source"
    )]
    fn test_minify(content: &str, expected: &str) -> Result<(), syn::Error> {
        assert_eq!(minify(content)?, expected);
        Ok(())
    }

    #[test_case(
        "#[cfg_attr(any(), rust_minify::skip)] #[allow(dead_code)]",
        "#[allow(dead_code)]";
        "skip before retained attribute"
    )]
    #[test_case(
        "#[allow(dead_code)] #[rust_minify::skip]",
        "#[allow(dead_code)]";
        "skip after retained attribute"
    )]
    #[test_case(
        "#[cfg_attr(all(), cfg(any()), rust_minify::skip)]",
        "#[cfg_attr(all(), cfg(any()))]";
        "retain cfg sibling"
    )]
    #[test_case(
        "#[cfg_attr(all(), cfg_attr(any(), rust_minify::skip, allow(dead_code)))]",
        "#[cfg_attr(all(), cfg_attr(any(), allow(dead_code)))]";
        "retain nested cfg sibling"
    )]
    fn test_remove_skip(attrs: &str, expected: &str) -> Result<(), syn::Error> {
        let body = "\nfn f() { let s =  \"日本語\"; }\n";
        let output = minify_opt(
            &format!("{attrs}{body}"),
            &MinifyOption {
                remove_skip: true,
                add_rustfmt_skip: false,
            },
        )?;
        assert_eq!(
            syn::parse_file(&output)?,
            syn::parse_file(&format!("{expected}{body}"))?
        );
        assert!(output.ends_with(body));
        Ok(())
    }

    #[test]
    fn test_punct_space() {
        // https://docs.rs/syn/latest/src/syn/token.rs.html#791-838
        const TOKENS: [&str; 46] = [
            "&", "&&", "&=", "@", "^", "^=", ":", ",", "$", ".", "..", "...", "..=", "=", "==",
            "=>", ">=", ">", "<-", "<=", "<", "-", "-=", "!=", "!", "|", "|=", "||", "::", "%",
            "%=", "+", "+=", "#", "?", "->", ";", "<<", "<<=", ">>", ">>=", "/", "/=", "*", "*=",
            "~",
        ];

        let mut separated = vec![];
        for t0 in TOKENS.iter() {
            for t1 in TOKENS.iter() {
                let mut t = t0.to_string();
                t.push_str(t1);
                if TOKENS.contains(&t.as_str()) {
                    separated.push((t0.chars().next_back().unwrap(), t1.chars().next().unwrap()));
                }
            }
        }
        separated.sort_unstable();
        separated.dedup();
        assert_eq!(SEPARATED, &separated[..]);
    }
}
