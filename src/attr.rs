use syn::{parse_str, Attribute, Item, Meta, NestedMeta, Path};

thread_local! {
    static RUST_MINIFY_SKIP: Path = parse_str::<Path>("rust_minify::skip").unwrap();
}

pub fn is_minify_skip_meta(meta: &Meta) -> bool {
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

pub trait ItemExt {
    fn get_attributes(&self) -> Option<&[Attribute]>;
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
}
