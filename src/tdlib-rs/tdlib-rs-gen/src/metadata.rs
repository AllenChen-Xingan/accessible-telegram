// Copyright 2020 - developers of the `grammers` project.
// Copyright 2022 - developers of the `tdlib-rs` project.
// Copyright 2024 - developers of the `tgt` and `tdlib-rs` projects.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::rustifier;
use std::collections::{HashMap, HashSet};
use tdlib_rs_parser::tl::{Category, Definition, Type};

/// Additional metadata required by several parts of the generation.
pub(crate) struct Metadata<'a> {
    recursing_defs: HashSet<&'a String>,
    default_impl_defs: HashSet<&'a String>,
    defs_with_type: HashMap<&'a String, Vec<&'a Definition>>,
}

impl<'a> Metadata<'a> {
    pub fn new(definitions: &'a [Definition]) -> Self {
        let mut metadata = Self {
            recursing_defs: HashSet::new(),
            default_impl_defs: HashSet::new(),
            defs_with_type: HashMap::new(),
        };

        let type_definitions = definitions
            .iter()
            .filter(|d| d.category == Category::Types)
            .collect::<Vec<_>>();

        let type_definition_map = type_definitions
            .iter()
            .map(|d| (&d.name, d))
            .collect::<HashMap<_, _>>();

        type_definitions.iter().for_each(|d| {
            metadata
                .defs_with_type
                .entry(&d.ty.name)
                .or_default()
                .push(d);
        });

        type_definitions.iter().for_each(|d| {
            if def_self_references(
                d,
                d,
                &metadata.defs_with_type,
                &type_definition_map,
                &mut HashSet::new(),
            ) {
                metadata.recursing_defs.insert(&d.name);
            }
        });

        type_definitions.iter().for_each(|d| {
            if def_contains_only_bare_types(d, &type_definition_map) {
                metadata.default_impl_defs.insert(&d.name);
            }
        });

        metadata
    }

    /// Returns `true` if any of the parameters of `Definition` eventually
    /// contains the same type as the `Definition` itself (meaning it recurses).
    pub fn is_recursive_def(&self, def: &Definition) -> bool {
        self.recursing_defs.contains(&def.name)
    }

    /// Returns `true` if the `Definition` can implement the trait `Default`
    pub fn can_def_implement_default(&self, def: &Definition) -> bool {
        self.default_impl_defs.contains(&def.name)
    }

    pub fn defs_with_type(&self, ty: &'a Type) -> &Vec<&Definition> {
        &self.defs_with_type[&ty.name]
    }
}

fn def_self_references<'a>(
    root: &Definition,
    check: &'a Definition,
    defs_with_type: &'a HashMap<&String, Vec<&Definition>>,
    defs_by_name: &'a HashMap<&String, &&Definition>,
    visited: &mut HashSet<&'a String>,
) -> bool {
    visited.insert(&check.name);
    for param in check.params.iter() {
        if param.ty.name == root.ty.name {
            return true;
        }

        // A bare type (`button:inlineButton`) names a constructor, not a
        // type, so follow that one definition instead of all of a type's.
        let next: Vec<&'a Definition> = if param.ty.bare {
            defs_by_name
                .get(&param.ty.name)
                .map(|d| vec![**d])
                .unwrap_or_default()
        } else {
            defs_with_type
                .get(&param.ty.name)
                .cloned()
                .unwrap_or_default()
        };

        for def in next {
            if def.ty.name == root.ty.name {
                return true;
            }
            if visited.contains(&def.name) {
                continue;
            }
            if def_self_references(root, def, defs_with_type, defs_by_name, visited) {
                return true;
            }
        }
    }

    false
}

fn def_contains_only_bare_types<'a>(
    check: &'a Definition,
    definition_map: &'a HashMap<&String, &&Definition>,
) -> bool {
    for param in check.params.iter() {
        if !rustifier::parameters::is_builtin_type(param) && !param.ty.bare {
            return false;
        }

        if let Some(def) = definition_map.get(&param.ty.name)
            && !def_contains_only_bare_types(def, definition_map)
        {
            return false;
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use tdlib_rs_parser::parse_tl_file;

    fn defs(tl: &str) -> Vec<Definition> {
        parse_tl_file(tl.to_owned()).map(Result::unwrap).collect()
    }

    fn recursive(defs: &[Definition], name: &str) -> bool {
        let metadata = Metadata::new(defs);
        let def = defs.iter().find(|d| d.name == name).unwrap();
        metadata.is_recursive_def(def)
    }

    #[test]
    fn cycle_through_bare_type_is_recursive() {
        // TDLib 1.8.69: `richTextButton button:inlineButton` (bare) closes
        // the cycle InlineButton -> RichText -> RichTextButton -> InlineButton.
        let defs = defs(
            "inlineButton text:RichText = InlineButton;\n\
             richTextPlain text:string = RichText;\n\
             richTextButton button:inlineButton = RichText;\n",
        );
        assert!(recursive(&defs, "richTextButton"));
        assert!(!recursive(&defs, "richTextPlain"));
    }

    #[test]
    fn bare_type_without_cycle_is_not_recursive() {
        let defs = defs(
            "formattedText text:string = FormattedText;\n\
             messageText text:formattedText = MessageContent;\n",
        );
        assert!(!recursive(&defs, "messageText"));
    }

    #[test]
    fn self_reference_through_vector_is_not_recursive() {
        // `Vec` is already on the heap, so no `Box` is needed.
        let defs = defs("pageBlockList items:vector<PageBlock> = PageBlock;\n");
        assert!(!recursive(&defs, "pageBlockList"));
    }
}
