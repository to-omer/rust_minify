pub mod attr;
pub mod fix;
pub mod marker;

use crate::marker::{LineColumn, SpanCollector};
use attr::{ItemExt, drain_minify_skip, is_minify_skip};
use fix::Visitor;
use marker::{LinedSource, SkipCollector};
use proc_macro2::{Delimiter, Group, Ident, Literal, Punct, Spacing, Span, TokenStream, TokenTree};
use quote::ToTokens;
use rustc_hash::FxHashSet;
use std::{iter::Peekable, ops::Range, str::FromStr, sync::LazyLock};
use syn::{File, parse_file, spanned::Spanned};

pub fn minify(content: &str) -> Result<String, syn::Error> {
    minify_opt(content, &MinifyOption::default())
}

pub fn minify_opt(content: &str, option: &MinifyOption) -> Result<String, syn::Error> {
    let content = content.strip_prefix('\u{feff}').unwrap_or(content);
    let mut sc = SpanCollector::new();
    let file = match parse_file(content) {
        Ok(file) => file,
        Err(_) => {
            let mut state = State::new_with_capacity(
                sc,
                MinifyMode {
                    space: SpaceCollapsing::Token,
                },
                content.len(),
            );
            state.step_tokens(TokenStream::from_str(content)?);
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
    state.skipped = skipped_sources(&file, &source, option.remove_skip)?
        .into_iter()
        .peekable();

    if let Some(shebang) = file.shebang {
        state.buf.push_str(&shebang);
        state.buf.push('\n');
    }
    for attr in file.attrs {
        state.step_tokens(attr.into_token_stream());
    }
    for mut item in file.items {
        if option.add_rustfmt_skip && !item.get_attributes().is_some_and(is_minify_skip) {
            state.buf.push_str("#[cfg_attr(any(),rustfmt::skip)]");
        }
        Visitor::fix_item(&mut item);
        state.step_tokens(item.into_token_stream());
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
    skipped: Peekable<std::vec::IntoIter<Skipped>>,
    skip_until: Option<LineColumn>,
    mode: MinifyMode,
}

#[derive(Debug, Clone)]
struct Skipped {
    range: Range<LineColumn>,
    source: String,
}

fn skipped_sources(
    file: &File,
    source: &LinedSource<'_>,
    remove_skip: bool,
) -> Result<Vec<Skipped>, syn::Error> {
    let mut items = SkipCollector::collect(file).items.into_iter().peekable();
    let mut skipped = Vec::new();
    while let Some((span, attrs)) = items.next() {
        let range = span.start().into()..span.end().into();
        let mut attrs: Vec<_> = attrs.iter().collect();
        // A skipped parent owns its source; nested markers still need removal.
        while items
            .peek()
            .is_some_and(|(nested, _)| nested.end() <= span.end())
        {
            attrs.extend(items.next().unwrap().1);
        }
        let mut text = String::new();
        let mut start = range.start;
        if remove_skip {
            attrs.sort_unstable_by_key(|attr| attr.span().start());
            for attr in attrs {
                if is_minify_skip(std::slice::from_ref(attr)) {
                    let attr_span = attr.span();
                    text.push_str(
                        source
                            .get(&(start..attr_span.start().into()))
                            .ok_or_else(|| syn::Error::new(attr_span, "invalid source span"))?,
                    );
                    let mut retained = vec![attr.clone()];
                    drain_minify_skip(&mut retained);
                    for attr in retained {
                        text.push_str(&attr.into_token_stream().to_string());
                    }
                    start = attr_span.end().into();
                }
            }
        }
        text.push_str(
            source
                .get(&(start..range.end))
                .ok_or_else(|| syn::Error::new(span, "invalid source span"))?,
        );
        skipped.push(Skipped {
            range,
            source: text,
        });
    }
    Ok(skipped)
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

#[derive(Debug, Clone, Default)]
enum PrevToken {
    #[default]
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

static MACHER: LazyLock<FxHashSet<(char, char)>> =
    LazyLock::new(|| SEPARATED.iter().cloned().collect());

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
            skipped: Vec::new().into_iter().peekable(),
            skip_until: None,
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
        let start = tt.span().start().into();
        if self.skip_until.is_some_and(|end| start < end) {
            return;
        }
        self.skip_until = None;
        if self
            .skipped
            .peek()
            .is_some_and(|item| item.range.start == start)
        {
            let item = self.skipped.next().unwrap();
            if !self.buf.is_empty() && !self.buf.ends_with('\n') {
                self.buf.push('\n');
            }
            self.buf.push_str(&item.source);
            self.buf.push('\n');
            self.prev = PrevToken::None;
            self.skip_until = Some(item.range.end);
            return;
        }
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
                prev.as_char() != '#' || prev.spacing() == Spacing::Alone
            }
            PrevToken::Punct(prev) if matches!(prev.spacing(), Spacing::Alone) => {
                match self.mode.space {
                    SpaceCollapsing::Syntax => match (prev.as_char(), punct.as_char()) {
                        ('<', '-') => true,
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
        "type F = fn(i32,) -> i32;",
        "type F=fn(i32)->i32;";
        "function pointer trailing comma"
    )]
    #[test_case(
        "type F = unsafe extern \"C\" fn(i32, ...);",
        "type F=unsafe extern \"C\" fn(i32,...);";
        "variadic function pointer comma"
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
        "fn f(x: i32) -> bool { x < -1 }",
        "fn f(x:i32)->bool{x< -1}";
        "comparison with negative literal"
    )]
    #[test_case(
        "fn f(x: i32) -> bool { x < - *&1 }",
        "fn f(x:i32)->bool{x< -*&1}";
        "comparison with negated expression"
    )]
    #[test_case(
        "#!/usr/bin/env rust-script\nfn main() {}",
        "#!/usr/bin/env rust-script\nfn main(){}";
        "shebang"
    )]
    #[test_case(
        "#!/path/it's-a-script\r\nfn main() {}",
        "#!/path/it's-a-script\r\nfn main(){}";
        "shebang with non rust tokens and crlf"
    )]
    #[test_case(
        "#!/usr/bin/env rust-script",
        "#!/usr/bin/env rust-script\n";
        "shebang without trailing newline"
    )]
    #[test_case(
        "#! /* comment */ [allow(dead_code)] fn f() {}",
        "#![allow(dead_code)]fn f(){}";
        "inner attribute is not a shebang"
    )]
    #[test_case(
        "\u{feff}#!/usr/bin/env rust-script\n#[cfg_attr(any(),rust_minify::skip)] fn 日本語() {}",
        "#!/usr/bin/env rust-script\n#[cfg_attr(any(),rust_minify::skip)] fn 日本語() {}\n";
        "shebang and skipped unicode source"
    )]
    #[test_case(
        "m!(/ /, / *, 1 . 2, 1 ., 1..2, # #, # \"ok\");",
        "m!(/ /,/ *,1 .2,1 .,1..2,# #,# \"ok\");";
        "macro token boundaries"
    )]
    #[test_case(
        "m!(##, # #, ###, ## #, # ##);",
        "m!(##,# #,###,## #,# ##);";
        "hash spacing in macros"
    )]
    #[test_case(
        "##, # #, ###, ## #, # ##",
        "##,# #,###,## #,# ##";
        "hash spacing in fallback"
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
        "const trait T<const N: usize> {} const fn f<A: [const] T<{m!(> =)}>>() {}",
        "const trait T<const N:usize>{}const fn f<A:[const]T<{m!(> =)}>>(){}";
        "macro tokens in const trait bound"
    )]
    #[test_case(
        "const trait T<const N: usize> {} const fn f<A: const T<{2 & &2}>>() {}",
        "const trait T<const N:usize>{}const fn f<A:const T<{2& &2}>>(){}";
        "bitwise and in const trait bound"
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
        "#!/usr/bin/env rust-script\n#[rust_minify::skip] #[allow(dead_code)]",
        "#!/usr/bin/env rust-script\n#[allow(dead_code)]";
        "skip after shebang"
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

    #[test_case("mod m", "fn f() { let s =  \"日本語\"; }"; "module")]
    #[test_case("impl S", "fn f() { let s =  \"日本語\"; }"; "impl method")]
    #[test_case("impl S", "const N: [i32; 2] = [1,  2,];"; "associated constant")]
    #[test_case("trait T", "fn f(x:  i32,);"; "trait method")]
    #[test_case("unsafe extern \"C\"", "fn f(x:  i32,);"; "foreign function")]
    #[test_case("fn outer()", "fn f() { let s =  \"日本語\"; }"; "local item")]
    #[test_case("fn outer()", "m!( a  b );"; "local macro")]
    fn test_nested_skip(container: &str, item: &str) -> Result<(), syn::Error> {
        let attr = "#[cfg_attr(any(), rust_minify::skip)]";
        let input = format!("{container} {{ {attr}{item} }} fn compact() {{ let x = 1; }}");
        for remove_skip in [false, true] {
            for add_rustfmt_skip in [false, true] {
                let output = minify_opt(
                    &input,
                    &MinifyOption {
                        remove_skip,
                        add_rustfmt_skip,
                    },
                )?;
                let attr = if remove_skip { "" } else { attr };
                let rustfmt = if add_rustfmt_skip {
                    "#[cfg_attr(any(),rustfmt::skip)]"
                } else {
                    ""
                };
                assert_eq!(
                    output,
                    format!(
                        "{rustfmt}{container}{{\n{attr}{item}\n}}{rustfmt}fn compact(){{let x=1;}}"
                    )
                );
                syn::parse_file(&output)?;
            }
        }
        Ok(())
    }

    #[test]
    fn test_nested_skip_with_parent() -> Result<(), syn::Error> {
        let attr = "#[cfg_attr(any(), rust_minify::skip)]";
        let body = "mod m {\n    #[cfg_attr(all(), cfg(any()), rust_minify::skip)]\n    fn f() { let s =  \"日本語\"; }\n}";
        let input = format!("{attr}{body} fn compact() {{}}");
        assert_eq!(minify(&input)?, format!("{attr}{body}\nfn compact(){{}}"));
        let output = minify_opt(
            &input,
            &MinifyOption {
                remove_skip: true,
                add_rustfmt_skip: true,
            },
        )?;
        assert!(output.starts_with("mod m {\n    #"));
        assert!(output.contains("\n    fn f() { let s =  \"日本語\"; }\n}"));
        assert_eq!(
            syn::parse_file(&output)?,
            syn::parse_file(
                "mod m { #[cfg_attr(all(), cfg(any()))] fn f() { let s = \"日本語\"; } } #[cfg_attr(any(),rustfmt::skip)] fn compact() {}"
            )?
        );
        Ok(())
    }

    #[test]
    fn test_nested_skip_preserves_following_tokens() -> Result<(), syn::Error> {
        let input = "mod m { mod n { m!(x); #[cfg_attr(any(),rust_minify::skip)]fn f() { let x =  1; } m!(> =); fn g(x: i32) -> bool { x < -1 } } }";
        assert_eq!(
            minify(input)?,
            "mod m{mod n{m!(x);\n#[cfg_attr(any(),rust_minify::skip)]fn f() { let x =  1; }\nm!(> =);fn g(x:i32)->bool{x< -1}}}"
        );
        Ok(())
    }

    #[test]
    fn test_nested_skip_with_doc_and_inner_attribute() -> Result<(), syn::Error> {
        let input = "mod m { /// doc\nfn f() { #![cfg_attr(any(),rust_minify::skip)] let s =  \"日本語\"; } fn g() {} }";
        assert_eq!(
            minify(input)?,
            "mod m{\n/// doc\nfn f() { #![cfg_attr(any(),rust_minify::skip)] let s =  \"日本語\"; }\nfn g(){}}"
        );
        let output = minify_opt(
            input,
            &MinifyOption {
                remove_skip: true,
                add_rustfmt_skip: false,
            },
        )?;
        assert_eq!(
            output,
            "mod m{\n/// doc\nfn f() {  let s =  \"日本語\"; }\nfn g(){}}"
        );
        Ok(())
    }

    #[test]
    fn test_adjacent_nested_skips() -> Result<(), syn::Error> {
        let first = "#[cfg_attr(any(),rust_minify::skip)]\r\nfn first() { // 日本語\r\n    let x = [1,  2,];\r\n}";
        let second = "#[cfg_attr(any(),rust_minify::skip)]fn second() { let x =  2; }";
        let input = format!("mod m {{ fn before() {{}} {first} {second} fn after() {{}} }}");
        assert_eq!(
            minify(&input)?,
            format!("mod m{{fn before(){{}}\n{first}\n{second}\nfn after(){{}}}}")
        );
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
