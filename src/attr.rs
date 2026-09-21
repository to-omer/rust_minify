use quote::ToTokens;
use syn::{parse_str, punctuated::Punctuated, Attribute, Item, Meta, Path, Token};

thread_local! {
    static RUST_MINIFY_SKIP: Path = parse_str::<Path>("rust_minify::skip").unwrap();
}

fn is_minify_skip_meta(meta: &Meta) -> bool {
    match meta {
        Meta::Path(path) => RUST_MINIFY_SKIP.with(|p| p == path),
        Meta::List(list) if list.path.is_ident("cfg_attr") => list
            .parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
            .map(|punct| punct.iter().skip(1).any(is_minify_skip_meta))
            .unwrap_or_default(),
        _ => false,
    }
}

pub fn is_minify_skip(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| is_minify_skip_meta(&attr.meta))
}

pub fn drain_minify_skip(attrs: &mut Vec<Attribute>) -> bool {
    let mut removed = false;
    attrs.retain_mut(|attr| retain_meta(&mut attr.meta, &mut removed));
    removed
}

fn retain_meta(meta: &mut Meta, removed: &mut bool) -> bool {
    match meta {
        Meta::Path(path) if RUST_MINIFY_SKIP.with(|p| p == path) => {
            *removed = true;
            false
        }
        Meta::List(list) if list.path.is_ident("cfg_attr") => {
            let Ok(args) = list.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
            else {
                return true;
            };
            let mut args = args.into_iter();
            let Some(predicate) = args.next() else {
                return true;
            };
            let mut nested_removed = false;
            let mut retained = Punctuated::<Meta, Token![,]>::new();
            retained.push(predicate);
            for mut arg in args {
                if retain_meta(&mut arg, &mut nested_removed) {
                    retained.push(arg);
                }
            }
            if nested_removed {
                *removed = true;
                if retained.len() == 1 {
                    return false;
                }
                list.tokens = retained.into_token_stream();
            }
            true
        }
        _ => true,
    }
}

pub trait ItemExt {
    fn get_attributes(&self) -> Option<&[Attribute]>;
    fn get_attributes_mut(&mut self) -> Option<&mut Vec<Attribute>>;
}

impl ItemExt for Item {
    fn get_attributes(&self) -> Option<&[Attribute]> {
        Some(match self {
            Item::Const(it) => &it.attrs,
            Item::Enum(it) => &it.attrs,
            Item::ExternCrate(it) => &it.attrs,
            Item::Fn(it) => &it.attrs,
            Item::ForeignMod(it) => &it.attrs,
            Item::Impl(it) => &it.attrs,
            Item::Macro(it) => &it.attrs,
            Item::Mod(it) => &it.attrs,
            Item::Static(it) => &it.attrs,
            Item::Struct(it) => &it.attrs,
            Item::Trait(it) => &it.attrs,
            Item::TraitAlias(it) => &it.attrs,
            Item::Type(it) => &it.attrs,
            Item::Union(it) => &it.attrs,
            Item::Use(it) => &it.attrs,
            _ => return None,
        })
    }

    fn get_attributes_mut(&mut self) -> Option<&mut Vec<Attribute>> {
        Some(match self {
            Item::Const(it) => &mut it.attrs,
            Item::Enum(it) => &mut it.attrs,
            Item::ExternCrate(it) => &mut it.attrs,
            Item::Fn(it) => &mut it.attrs,
            Item::ForeignMod(it) => &mut it.attrs,
            Item::Impl(it) => &mut it.attrs,
            Item::Macro(it) => &mut it.attrs,
            Item::Mod(it) => &mut it.attrs,
            Item::Static(it) => &mut it.attrs,
            Item::Struct(it) => &mut it.attrs,
            Item::Trait(it) => &mut it.attrs,
            Item::TraitAlias(it) => &mut it.attrs,
            Item::Type(it) => &mut it.attrs,
            Item::Union(it) => &mut it.attrs,
            Item::Use(it) => &mut it.attrs,
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    #[test_case("#[rust_minify::skip]mod a;", true; "rust_minify::skip")]
    #[test_case("#[cfg_attr(foo, rust_minify::skip)]mod a;", true; "cfg_attr(foo, rust_minify::skip)")]
    #[test_case("#[rustfmt::skip]mod a;", false; "rustfmt::skip")]
    #[test_case("#[cfg_attr(foo, rustfmt::skip)]mod a;", false; "cfg_attr(foo, rustfmt::skip)")]
    fn test_is_minify_skip(content: &str, expected: bool) {
        let item = parse_str::<Item>(content).unwrap();
        let attrs = item.get_attributes().unwrap();
        assert_eq!(is_minify_skip(attrs), expected);
    }

    #[test_case(
        "#[rust_minify::skip] #[allow(dead_code)] fn f() {}",
        "#[allow(dead_code)] fn f() {}"
    )]
    #[test_case(
        "#[allow(dead_code)] #[rust_minify::skip] fn f() {}",
        "#[allow(dead_code)] fn f() {}"
    )]
    #[test_case(
        "#[cfg_attr(all(), cfg(any()), rust_minify::skip)] fn f() {}",
        "#[cfg_attr(all(), cfg(any()))] fn f() {}"
    )]
    #[test_case(
        "#[cfg_attr(all(), cfg_attr(any(), rust_minify::skip), allow(dead_code))] fn f() {}",
        "#[cfg_attr(all(), allow(dead_code))] fn f() {}"
    )]
    #[test_case(
        "#[cfg_attr(all(), cfg_attr(any(), rust_minify::skip, allow(dead_code)))] fn f() {}",
        "#[cfg_attr(all(), cfg_attr(any(), allow(dead_code)))] fn f() {}"
    )]
    fn test_drain_minify_skip(content: &str, expected: &str) {
        let mut item = parse_str::<Item>(content).unwrap();
        let attrs = item.get_attributes_mut().unwrap();
        assert!(drain_minify_skip(attrs));
        assert!(!drain_minify_skip(attrs));
        assert_eq!(item, parse_str::<Item>(expected).unwrap());
    }
}
