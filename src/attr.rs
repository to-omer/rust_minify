use syn::{parse_str, Attribute, Item, Meta, NestedMeta, Path};

thread_local! {
    static RUST_MINIFY_SKIP: Path = parse_str::<Path>("rust_minify::skip").unwrap();
}

fn is_minify_skip_meta(meta: &Meta) -> bool {
    match meta {
        Meta::Path(path) => RUST_MINIFY_SKIP.with(|p| p == path),
        Meta::List(list) => {
            list.path.is_ident("cfg_attr")
                && list.nested.iter().skip(1).any(|nested| match nested {
                    NestedMeta::Meta(meta) => is_minify_skip_meta(meta),
                    NestedMeta::Lit(_) => false,
                })
        }
        Meta::NameValue(_) => false,
    }
}

pub fn is_minify_skip(attrs: &[Attribute]) -> bool {
    attrs
        .iter()
        .any(|attr| attr.parse_meta().iter().any(is_minify_skip_meta))
}

pub fn drain_minify_skip(attrs: &mut Vec<Attribute>) -> bool {
    any_drain_filter(attrs, |attr| {
        attr.parse_meta().iter().any(is_minify_skip_meta)
    })
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
            Item::Macro2(it) => &it.attrs,
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
            Item::Macro2(it) => &mut it.attrs,
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
