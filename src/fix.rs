use syn::{
    punctuated::Punctuated, visit_mut, visit_mut::VisitMut, AngleBracketedGenericArguments,
    BoundLifetimes, Constraint, DataEnum, ExprArray, ExprCall, ExprClosure, ExprMethodCall,
    ExprStruct, ExprTuple, FieldsNamed, FieldsUnnamed, Generics, Item, ItemEnum, ItemTrait,
    ItemTraitAlias, LifetimeParam, ParenthesizedGenericArguments, PatOr, PatSlice, PatStruct,
    PatTuple, PredicateLifetime, PredicateType, Signature, TraitItemType, TypeBareFn,
    TypeImplTrait, TypeParam, TypeTraitObject, TypeTuple, UseGroup, WhereClause,
};

pub fn remove_trailing_punct<T, P>(punctuated: &mut Punctuated<T, P>) {
    if punctuated.trailing_punct() {
        let value = punctuated.pop().unwrap().into_value();
        punctuated.push_value(value);
    }
}

pub struct Visitor;

impl Visitor {
    pub fn fix_item(node: &mut Item) {
        let mut visitor = Self;
        visitor.visit_item_mut(node);
    }
}

impl VisitMut for Visitor {
    fn visit_angle_bracketed_generic_arguments_mut(
        &mut self,
        node: &mut AngleBracketedGenericArguments,
    ) {
        remove_trailing_punct(&mut node.args);
        visit_mut::visit_angle_bracketed_generic_arguments_mut(self, node);
    }

    fn visit_bound_lifetimes_mut(&mut self, node: &mut BoundLifetimes) {
        remove_trailing_punct(&mut node.lifetimes);
        visit_mut::visit_bound_lifetimes_mut(self, node);
    }

    fn visit_constraint_mut(&mut self, node: &mut Constraint) {
        remove_trailing_punct(&mut node.bounds);
        visit_mut::visit_constraint_mut(self, node);
    }

    fn visit_data_enum_mut(&mut self, node: &mut DataEnum) {
        remove_trailing_punct(&mut node.variants);
        visit_mut::visit_data_enum_mut(self, node);
    }

    fn visit_expr_array_mut(&mut self, node: &mut ExprArray) {
        remove_trailing_punct(&mut node.elems);
        visit_mut::visit_expr_array_mut(self, node);
    }

    fn visit_expr_call_mut(&mut self, node: &mut ExprCall) {
        remove_trailing_punct(&mut node.args);
        visit_mut::visit_expr_call_mut(self, node);
    }

    fn visit_expr_closure_mut(&mut self, node: &mut ExprClosure) {
        remove_trailing_punct(&mut node.inputs);
        visit_mut::visit_expr_closure_mut(self, node);
    }

    fn visit_expr_method_call_mut(&mut self, node: &mut ExprMethodCall) {
        remove_trailing_punct(&mut node.args);
        visit_mut::visit_expr_method_call_mut(self, node);
    }

    fn visit_expr_struct_mut(&mut self, node: &mut ExprStruct) {
        if node.dot2_token.is_none() {
            remove_trailing_punct(&mut node.fields);
        }
        visit_mut::visit_expr_struct_mut(self, node);
    }

    fn visit_expr_tuple_mut(&mut self, node: &mut ExprTuple) {
        if node.elems.len() > 1 {
            remove_trailing_punct(&mut node.elems);
        }
        visit_mut::visit_expr_tuple_mut(self, node);
    }

    fn visit_fields_named_mut(&mut self, node: &mut FieldsNamed) {
        remove_trailing_punct(&mut node.named);
        visit_mut::visit_fields_named_mut(self, node);
    }

    fn visit_fields_unnamed_mut(&mut self, node: &mut FieldsUnnamed) {
        remove_trailing_punct(&mut node.unnamed);
        visit_mut::visit_fields_unnamed_mut(self, node);
    }

    fn visit_generics_mut(&mut self, node: &mut Generics) {
        remove_trailing_punct(&mut node.params);
        visit_mut::visit_generics_mut(self, node);
    }

    fn visit_item_enum_mut(&mut self, node: &mut ItemEnum) {
        remove_trailing_punct(&mut node.variants);
        visit_mut::visit_item_enum_mut(self, node);
    }

    fn visit_item_trait_mut(&mut self, node: &mut ItemTrait) {
        remove_trailing_punct(&mut node.supertraits);
        visit_mut::visit_item_trait_mut(self, node);
    }

    fn visit_item_trait_alias_mut(&mut self, node: &mut ItemTraitAlias) {
        remove_trailing_punct(&mut node.bounds);
        visit_mut::visit_item_trait_alias_mut(self, node);
    }

    fn visit_lifetime_param_mut(&mut self, node: &mut LifetimeParam) {
        remove_trailing_punct(&mut node.bounds);
        visit_mut::visit_lifetime_param_mut(self, node);
    }

    fn visit_parenthesized_generic_arguments_mut(
        &mut self,
        node: &mut ParenthesizedGenericArguments,
    ) {
        remove_trailing_punct(&mut node.inputs);
        visit_mut::visit_parenthesized_generic_arguments_mut(self, node);
    }

    fn visit_pat_or_mut(&mut self, node: &mut PatOr) {
        node.leading_vert.take();
        visit_mut::visit_pat_or_mut(self, node);
    }

    fn visit_pat_slice_mut(&mut self, node: &mut PatSlice) {
        remove_trailing_punct(&mut node.elems);
        visit_mut::visit_pat_slice_mut(self, node);
    }

    fn visit_pat_struct_mut(&mut self, node: &mut PatStruct) {
        if node.rest.is_none() {
            remove_trailing_punct(&mut node.fields);
        }
        visit_mut::visit_pat_struct_mut(self, node);
    }

    fn visit_pat_tuple_mut(&mut self, node: &mut PatTuple) {
        if node.elems.len() > 1 {
            remove_trailing_punct(&mut node.elems);
        }
        visit_mut::visit_pat_tuple_mut(self, node);
    }

    fn visit_predicate_lifetime_mut(&mut self, node: &mut PredicateLifetime) {
        remove_trailing_punct(&mut node.bounds);
        visit_mut::visit_predicate_lifetime_mut(self, node);
    }

    fn visit_predicate_type_mut(&mut self, node: &mut PredicateType) {
        remove_trailing_punct(&mut node.bounds);
        visit_mut::visit_predicate_type_mut(self, node);
    }

    fn visit_signature_mut(&mut self, node: &mut Signature) {
        if node.variadic.is_none() {
            remove_trailing_punct(&mut node.inputs);
        }
        visit_mut::visit_signature_mut(self, node);
    }

    fn visit_trait_item_type_mut(&mut self, node: &mut TraitItemType) {
        remove_trailing_punct(&mut node.bounds);
        visit_mut::visit_trait_item_type_mut(self, node);
    }

    fn visit_type_bare_fn_mut(&mut self, node: &mut TypeBareFn) {
        if node.variadic.is_none() {
            remove_trailing_punct(&mut node.inputs);
        }
        visit_mut::visit_type_bare_fn_mut(self, node);
    }

    fn visit_type_impl_trait_mut(&mut self, node: &mut TypeImplTrait) {
        remove_trailing_punct(&mut node.bounds);
        visit_mut::visit_type_impl_trait_mut(self, node);
    }

    fn visit_type_param_mut(&mut self, node: &mut TypeParam) {
        remove_trailing_punct(&mut node.bounds);
        visit_mut::visit_type_param_mut(self, node);
    }

    fn visit_type_trait_object_mut(&mut self, node: &mut TypeTraitObject) {
        remove_trailing_punct(&mut node.bounds);
        visit_mut::visit_type_trait_object_mut(self, node);
    }

    fn visit_type_tuple_mut(&mut self, node: &mut TypeTuple) {
        if node.elems.len() > 1 {
            remove_trailing_punct(&mut node.elems);
        }
        visit_mut::visit_type_tuple_mut(self, node);
    }

    fn visit_use_group_mut(&mut self, node: &mut UseGroup) {
        remove_trailing_punct(&mut node.items);
        visit_mut::visit_use_group_mut(self, node);
    }

    fn visit_where_clause_mut(&mut self, node: &mut WhereClause) {
        remove_trailing_punct(&mut node.predicates);
        visit_mut::visit_where_clause_mut(self, node);
    }
}
