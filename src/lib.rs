use once_cell::sync::Lazy;
use proc_macro2::{Delimiter, Punct, Spacing, TokenStream, TokenTree};
use quote::ToTokens;
use std::collections::HashSet;
use syn::{parse_file, spanned::Spanned, Item};

// Whitespace needed:
//   1. between (Ident, Literal) and (Ident, Literal).
//   2. between Puncts that become another Punct when combined.

enum PrevToken {
    None,
    IdentOrLiteral,
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

static MACHER: Lazy<HashSet<(char, char)>> = Lazy::new(|| SEPARATED.iter().cloned().collect());

pub fn minify(content: &str) -> Result<String, syn::Error> {
    let mut buf = String::with_capacity(content.len());
    minify_token_stream(parse_file(content)?.to_token_stream(), &mut buf);
    Ok(buf)
}

pub fn minify_token_stream(token_stream: TokenStream, buf: &mut String) {
    let mut prev = PrevToken::None;
    for tt in token_stream {
        match tt {
            TokenTree::Group(group) => {
                let (ldel, rdel) = match group.delimiter() {
                    Delimiter::Parenthesis => ('(', ')'),
                    Delimiter::Brace => ('{', '}'),
                    Delimiter::Bracket => ('[', ']'),
                    Delimiter::None => {
                        // HELP: What this?
                        eprintln!("warning: Implicit Delimiter");
                        (' ', ' ')
                    }
                };
                buf.push(ldel);
                minify_token_stream(group.stream(), buf);
                buf.push(rdel);
                prev = PrevToken::None;
            }
            TokenTree::Ident(ident) => {
                if matches!(prev, PrevToken::IdentOrLiteral) {
                    buf.push(' ');
                }
                buf.push_str(&ident.to_string());
                prev = PrevToken::IdentOrLiteral;
            }
            TokenTree::Punct(punct) => {
                if let PrevToken::Punct(prev) = prev {
                    if matches!(prev.spacing(), Spacing::Alone)
                        && MACHER.contains(&(prev.as_char(), punct.as_char()))
                    {
                        buf.push(' ');
                    }
                }
                buf.push_str(&punct.to_string());
                prev = PrevToken::Punct(punct);
            }
            TokenTree::Literal(literal) => {
                if matches!(prev, PrevToken::IdentOrLiteral) {
                    buf.push(' ');
                }
                buf.push_str(&literal.to_string());
                prev = PrevToken::IdentOrLiteral;
            }
        }
    }
}

pub fn minify_selected<S>(content: &str, mut select: S) -> Result<String, syn::Error>
where
    S: FnMut(&Item) -> bool,
{
    let mut buf = String::with_capacity(content.len());
    let mut is_newline = true;
    let lines: Vec<&str> = content.split_inclusive('\n').collect();
    for item in parse_file(content)?.items {
        if select(&item) {
            is_newline = false;
            minify_token_stream(item.to_token_stream(), &mut buf);
        } else {
            if !is_newline {
                buf.push('\n');
                is_newline = true;
            }
            let span = item.span();
            let start = span.start();
            let end = span.end();
            if start.line == end.line {
                buf.push_str(&lines[start.line - 1][start.column..=end.column]);
            } else {
                buf.push_str(&lines[start.line - 1][start.column..]);
                for line in start.line + 1..end.line.saturating_sub(1) {
                    buf.push_str(&lines[line - 1]);
                }
                buf.push_str(&lines[end.line - 1][..=end.column]);
            }
        }
    }
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_minify() {
        let ts = parse_file(
            r#"fn f(a: Vec<usize>) -> usize {
    let mut total = 0usize;
    for a in a.iter().cloned() {
        total += a;
    }
    total
}
"#,
        )
        .unwrap()
        .to_token_stream();
        let mut buf = String::new();
        minify_token_stream(ts, &mut buf);
        assert_eq!(
            buf,
            "fn f(a:Vec<usize>)->usize{let mut total=0usize;for a in a.iter().cloned(){total+=a;}total}"
        );
    }

    #[test]
    fn test_punct_space() {
        // https://docs.rs/syn/1.0.72/src/syn/token.rs.html#707-754
        let tokens = [
            "+", "+=", "&", "&&", "&=", "@", "!", "^", "^=", ":", "::", ",", "/", "/=", "$", ".",
            "..", "...", "..=", "=", "==", ">=", ">", "<=", "<", "*=", "!=", "|", "|=", "||", "#",
            "?", "->", "<-", "%", "%=", "=>", ";", "<<", "<<=", ">>", ">>=", "*", "-", "-=", "~",
        ];
        let mut separated = vec![];
        for t0 in tokens.iter().cloned() {
            for t1 in tokens.iter().cloned() {
                let mut t = t0.to_string();
                t.push_str(t1);
                if tokens.contains(&t.as_str()) {
                    separated.push((t0.chars().next_back().unwrap(), t1.chars().next().unwrap()));
                }
            }
        }
        separated.sort();
        separated.dedup();
        assert_eq!(SEPARATED, &separated[..]);
    }
}
