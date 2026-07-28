use std::collections::{BTreeMap, BTreeSet};

use quote::ToTokens;
use syn::visit::Visit;

pub(super) fn unconditional_rust_const_definition(source: &str, name: &str) -> Option<String> {
    let syntax = syn::parse_file(source).ok()?;
    if conditional(&syntax.attrs) {
        return None;
    }
    let mut definitions = syntax.items.iter().filter_map(|item| {
        let syn::Item::Const(item) = item else {
            return None;
        };
        (item.ident == name && !conditional(&item.attrs))
            .then(|| item.to_token_stream().to_string())
    });
    let definition = definitions.next()?;
    definitions.next().is_none().then_some(definition)
}

#[derive(Default)]
struct LocalFunctionCalls(BTreeSet<String>);

impl<'ast> Visit<'ast> for LocalFunctionCalls {
    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        if let syn::Expr::Path(path) = call.func.as_ref() {
            if let Some(segment) = path.path.segments.last() {
                self.0.insert(segment.ident.to_string());
            }
        }
        syn::visit::visit_expr_call(self, call);
    }
}

pub(super) fn reachable_enabled_functions(source: &str, root: &str) -> Option<String> {
    let syntax = syn::parse_file(source).ok()?;
    if conditional(&syntax.attrs) {
        return None;
    }
    let mut functions = BTreeMap::new();
    for item in &syntax.items {
        let syn::Item::Fn(function) = item else {
            continue;
        };
        if conditional(&function.attrs) {
            continue;
        }
        if functions
            .insert(function.sig.ident.to_string(), function)
            .is_some()
        {
            return None;
        }
    }
    let mut pending = vec![root.to_string()];
    let mut visited = BTreeSet::new();
    let mut contract = String::new();
    while let Some(name) = pending.pop() {
        if !visited.insert(name.clone()) {
            continue;
        }
        let function = functions.get(&name)?;
        contract.push_str(&function.to_token_stream().to_string());
        let mut calls = LocalFunctionCalls::default();
        calls.visit_block(&function.block);
        pending.extend(
            calls
                .0
                .into_iter()
                .filter(|call| functions.contains_key(call)),
        );
    }
    Some(
        contract
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect(),
    )
}

pub(super) fn noop_enabled_function(source: &str, name: &str) -> Option<String> {
    let mut syntax = syn::parse_file(source).ok()?;
    if conditional(&syntax.attrs) {
        return None;
    }
    let mut matches = 0;
    for item in &mut syntax.items {
        let syn::Item::Fn(function) = item else {
            continue;
        };
        if function.sig.ident == name && !conditional(&function.attrs) {
            function.block = Box::new(syn::parse_quote!({}));
            matches += 1;
        }
    }
    (matches == 1).then(|| syntax.to_token_stream().to_string())
}

pub(super) fn enabled_anchor_has_markers<'a>(
    source: &str,
    anchor: &str,
    markers: impl IntoIterator<Item = &'a str>,
) -> bool {
    reachable_enabled_functions(source, anchor)
        .is_some_and(|contract| markers.into_iter().all(|marker| contract.contains(marker)))
}

fn conditional(attributes: &[syn::Attribute]) -> bool {
    attributes
        .iter()
        .any(|attribute| attribute.path().is_ident("cfg") || attribute.path().is_ident("cfg_attr"))
}
