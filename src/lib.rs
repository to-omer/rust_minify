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

    let mut is_newline = state.buf.is_empty();
    for mut item in file.items {
        let cond = if option.remove_skip {
            item.get_attributes_mut().is_some_and(drain_minify_skip)
        } else {
            item.get_attributes().is_some_and(is_minify_skip)
        };
        if cond {
            if !is_newline {
                state.buf.push('\n');
                is_newline = true;
            }
            let span = item.span();
            if let Some(s) = source.get(&(span.start().into()..span.end().into())) {
                state.buf.push_str(s);
                state.buf.push('\n');
            };
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
    /// Ident or Lit, ends with `.`
    IdentOrLiteral(bool),
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
        Self {
            prev: Default::default(),
            buf: Default::default(),
            bitwise_and: collector.bitwise_and,
            tokens: collector.tokens.into_iter().peekable(),
            mode,
        }
    }
    pub fn new_with_capacity(collector: SpanCollector, mode: MinifyMode, capacity: usize) -> Self {
        Self {
            prev: Default::default(),
            buf: String::with_capacity(capacity),
            bitwise_and: collector.bitwise_and,
            tokens: collector.tokens.into_iter().peekable(),
            mode,
        }
    }
    pub fn step_tokens(&mut self, tokens: TokenStream) {
        for tt in tokens {
            self.step_token_tree(tt);
        }
    }
    pub fn step_token_tree(&mut self, tt: TokenTree) {
        self.switch_space_mode(tt.span());
        match tt {
            TokenTree::Group(group) => self.step_group(group),
            TokenTree::Ident(ident) => self.step_ident(ident),
            TokenTree::Punct(punct) => self.step_punct(punct),
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
        if matches!(self.prev, PrevToken::IdentOrLiteral(_)) {
            self.buf.push(' ');
        }
        self.buf.push_str(&ident.to_string());
        self.prev = PrevToken::IdentOrLiteral(false);
    }
    fn step_punct(&mut self, punct: Punct) {
        let needs_space = match &self.prev {
            PrevToken::IdentOrLiteral(true) if punct.as_char() == '.' => true,
            PrevToken::IdentOrLiteral(_) if "#\"'".contains(punct.as_char()) => true,
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
        self.buf.push_str(&punct.to_string());
        self.prev = PrevToken::Punct(punct);
    }
    fn step_literal(&mut self, literal: Literal) {
        if matches!(self.prev, PrevToken::IdentOrLiteral(_)) {
            self.buf.push(' ');
        }
        let lit = literal.to_string();
        let last_is_dot = lit.ends_with('.');
        let tuple_access = matches!(&self.prev, PrevToken::Punct(punct) if punct.as_char() == '.')
            && lit.chars().next().is_some_and(|c| c.is_ascii_digit());
        self.buf.push_str(&lit);
        self.prev = PrevToken::IdentOrLiteral(last_is_dot | tuple_access);
    }
    fn switch_space_mode(&mut self, span: Span) {
        match self.mode.space {
            SpaceCollapsing::Syntax => {
                if let Some(range) = self.tokens.peek() {
                    if range.contains(&span.start().into()) {
                        self.mode.space = SpaceCollapsing::Macro;
                    }
                }
            }
            SpaceCollapsing::Macro => {
                if let Some(range) = self.tokens.peek() {
                    if !range.contains(&span.start().into()) {
                        self.mode.space = SpaceCollapsing::Syntax;
                        self.tokens.next();
                    }
                }
            }
            SpaceCollapsing::Token => {}
        }
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
    fn test_minify(content: &str, expected: &str) -> Result<(), syn::Error> {
        assert_eq!(minify(content)?, expected);
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
