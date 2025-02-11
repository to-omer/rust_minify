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
    any_drain_filter(attrs, |attr| is_minify_skip_meta(&attr.meta))
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

fn any_drain_filter<T, F>(v: &mut Vec<T>, mut filter: F) -> bool
where
    F: FnMut(&T) -> bool,
{
    let n = v.len();
    let mut del = 0usize;
    for i in 0..n {
        unsafe {
            if filter(v.get_unchecked(i)) {
                del += 1;
            } else if del > 0 {
                let src = v.as_ptr().add(i);
                let dst = v.as_mut_ptr().add(i - del);
                std::ptr::copy_nonoverlapping(src, dst, 1);
            }
        }
    }
    v.truncate(n - del);
    del > 0
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

    #[test]
    fn test_any_drain_filter() {
        let mut v = vec![1, 2, 3, 4, 5];
        let result = any_drain_filter(&mut v, |x| *x % 2 == 0);
        assert!(result);
        assert_eq!(v, vec![1, 3, 5]);
    }
}
