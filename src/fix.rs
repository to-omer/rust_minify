use syn::{
    AngleBracketedGenericArguments, BoundLifetimes, Constraint, DataEnum, ExprArray, ExprCall,
    ExprClosure, ExprMethodCall, ExprStruct, ExprTuple, FieldsNamed, FieldsUnnamed, Generics, Item,
    ItemEnum, ItemTrait, ItemTraitAlias, LifetimeParam, ParenthesizedGenericArguments, PatOr,
    PatSlice, PatStruct, PatTuple, PredicateLifetime, PredicateType, Signature, TraitItemType,
    TypeFnPtr, TypeImplTrait, TypeParam, TypeTraitObject, TypeTuple, UseGroup, WhereClause,
    visit_mut, visit_mut::VisitMut,
};

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
        node.args.pop_punct();
        visit_mut::visit_angle_bracketed_generic_arguments_mut(self, node);
    }

    fn visit_bound_lifetimes_mut(&mut self, node: &mut BoundLifetimes) {
        node.lifetimes.pop_punct();
        visit_mut::visit_bound_lifetimes_mut(self, node);
    }

    fn visit_constraint_mut(&mut self, node: &mut Constraint) {
        node.bounds.pop_punct();
        visit_mut::visit_constraint_mut(self, node);
    }

    fn visit_data_enum_mut(&mut self, node: &mut DataEnum) {
        node.variants.pop_punct();
        visit_mut::visit_data_enum_mut(self, node);
    }

    fn visit_expr_array_mut(&mut self, node: &mut ExprArray) {
        node.elems.pop_punct();
        visit_mut::visit_expr_array_mut(self, node);
    }

    fn visit_expr_call_mut(&mut self, node: &mut ExprCall) {
        node.args.pop_punct();
        visit_mut::visit_expr_call_mut(self, node);
    }

    fn visit_expr_closure_mut(&mut self, node: &mut ExprClosure) {
        node.inputs.pop_punct();
        visit_mut::visit_expr_closure_mut(self, node);
    }

    fn visit_expr_method_call_mut(&mut self, node: &mut ExprMethodCall) {
        node.args.pop_punct();
        visit_mut::visit_expr_method_call_mut(self, node);
    }

    fn visit_expr_struct_mut(&mut self, node: &mut ExprStruct) {
        if node.dot2_token.is_none() {
            node.fields.pop_punct();
        }
        visit_mut::visit_expr_struct_mut(self, node);
    }

    fn visit_expr_tuple_mut(&mut self, node: &mut ExprTuple) {
        if node.elems.len() > 1 {
            node.elems.pop_punct();
        }
        visit_mut::visit_expr_tuple_mut(self, node);
    }

    fn visit_fields_named_mut(&mut self, node: &mut FieldsNamed) {
        node.named.pop_punct();
        visit_mut::visit_fields_named_mut(self, node);
    }

    fn visit_fields_unnamed_mut(&mut self, node: &mut FieldsUnnamed) {
        node.unnamed.pop_punct();
        visit_mut::visit_fields_unnamed_mut(self, node);
    }

    fn visit_generics_mut(&mut self, node: &mut Generics) {
        node.params.pop_punct();
        visit_mut::visit_generics_mut(self, node);
    }

    fn visit_item_enum_mut(&mut self, node: &mut ItemEnum) {
        node.variants.pop_punct();
        visit_mut::visit_item_enum_mut(self, node);
    }

    fn visit_item_trait_mut(&mut self, node: &mut ItemTrait) {
        node.supertraits.pop_punct();
        visit_mut::visit_item_trait_mut(self, node);
    }

    fn visit_item_trait_alias_mut(&mut self, node: &mut ItemTraitAlias) {
        node.bounds.pop_punct();
        visit_mut::visit_item_trait_alias_mut(self, node);
    }

    fn visit_lifetime_param_mut(&mut self, node: &mut LifetimeParam) {
        node.bounds.pop_punct();
        visit_mut::visit_lifetime_param_mut(self, node);
    }

    fn visit_parenthesized_generic_arguments_mut(
        &mut self,
        node: &mut ParenthesizedGenericArguments,
    ) {
        node.inputs.pop_punct();
        visit_mut::visit_parenthesized_generic_arguments_mut(self, node);
    }

    fn visit_pat_or_mut(&mut self, node: &mut PatOr) {
        node.leading_vert.take();
        visit_mut::visit_pat_or_mut(self, node);
    }

    fn visit_pat_slice_mut(&mut self, node: &mut PatSlice) {
        node.elems.pop_punct();
        visit_mut::visit_pat_slice_mut(self, node);
    }

    fn visit_pat_struct_mut(&mut self, node: &mut PatStruct) {
        if node.rest.is_none() {
            node.fields.pop_punct();
        }
        visit_mut::visit_pat_struct_mut(self, node);
    }

    fn visit_pat_tuple_mut(&mut self, node: &mut PatTuple) {
        if node.elems.len() > 1 {
            node.elems.pop_punct();
        }
        visit_mut::visit_pat_tuple_mut(self, node);
    }

    fn visit_predicate_lifetime_mut(&mut self, node: &mut PredicateLifetime) {
        node.bounds.pop_punct();
        visit_mut::visit_predicate_lifetime_mut(self, node);
    }

    fn visit_predicate_type_mut(&mut self, node: &mut PredicateType) {
        node.bounds.pop_punct();
        visit_mut::visit_predicate_type_mut(self, node);
    }

    fn visit_signature_mut(&mut self, node: &mut Signature) {
        if node.variadic.is_none() {
            node.inputs.pop_punct();
        }
        visit_mut::visit_signature_mut(self, node);
    }

    fn visit_trait_item_type_mut(&mut self, node: &mut TraitItemType) {
        node.bounds.pop_punct();
        visit_mut::visit_trait_item_type_mut(self, node);
    }

    fn visit_type_fn_ptr_mut(&mut self, node: &mut TypeFnPtr) {
        if node.variadic.is_none() {
            node.inputs.pop_punct();
        }
        visit_mut::visit_type_fn_ptr_mut(self, node);
    }

    fn visit_type_impl_trait_mut(&mut self, node: &mut TypeImplTrait) {
        node.bounds.pop_punct();
        visit_mut::visit_type_impl_trait_mut(self, node);
    }

    fn visit_type_param_mut(&mut self, node: &mut TypeParam) {
        node.bounds.pop_punct();
        visit_mut::visit_type_param_mut(self, node);
    }

    fn visit_type_trait_object_mut(&mut self, node: &mut TypeTraitObject) {
        node.bounds.pop_punct();
        visit_mut::visit_type_trait_object_mut(self, node);
    }

    fn visit_type_tuple_mut(&mut self, node: &mut TypeTuple) {
        if node.elems.len() > 1 {
            node.elems.pop_punct();
        }
        visit_mut::visit_type_tuple_mut(self, node);
    }

    fn visit_use_group_mut(&mut self, node: &mut UseGroup) {
        node.items.pop_punct();
        visit_mut::visit_use_group_mut(self, node);
    }

    fn visit_where_clause_mut(&mut self, node: &mut WhereClause) {
        node.predicates.pop_punct();
        visit_mut::visit_where_clause_mut(self, node);
    }
}
